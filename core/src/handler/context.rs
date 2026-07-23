use hyper::body::Bytes;
use hyper::header::{HeaderName, HeaderValue};
use hyper::{Request, Response, body::Incoming, header};
use serde::Serialize;
use std::sync::OnceLock;

use crate::{
    config::env,
    error::HttpError,
    handler::{BoxStream, ResBody},
};

// ─── ContentType ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContentType {
    MsgPack,
    Json,
    Yaml,
    Html,
    Csv,
    PlainText,
    Xml,
}

impl ContentType {
    /// Parsea un solo valor de mime type, ignorando parámetros
    /// como ";charset=utf-8". Cualquier valor no reconocido cae en
    /// MsgPack (el default del framework), nunca en None.
    pub(crate) fn from_header_value(v: &str) -> Self {
        let mime = v.split(';').next().unwrap_or("").trim();
        match mime {
            "application/json" => ContentType::Json,
            "application/yaml" | "text/yaml" => ContentType::Yaml,
            "text/html" => ContentType::Html,
            "text/csv" => ContentType::Csv,
            "text/plain" => ContentType::PlainText,
            "application/xml" | "text/xml" => ContentType::Xml,
            _ => ContentType::MsgPack,
        }
    }

    /// Un Accept puede traer varios valores separados por coma;
    /// se toma el primero (sin ponderar q-values todavía).
    pub(crate) fn from_accept_header(v: &str) -> Self {
        v.split(',')
            .next()
            .map(|part| Self::from_header_value(part.trim()))
            .unwrap_or(ContentType::MsgPack)
    }

    pub fn mime(&self) -> &'static str {
        match self {
            ContentType::MsgPack => "application/msgpack",
            ContentType::Json => "application/json",
            ContentType::Yaml => "application/yaml",
            ContentType::Html => "text/html",
            ContentType::Csv => "text/csv",
            ContentType::PlainText => "text/plain",
            ContentType::Xml => "application/xml",
        }
    }
}

// ─── Fallback bytes (500 por fallo de serialización) ───────────────────────
// Todos se computan una única vez (OnceLock) y quedan cacheados el resto
// del proceso -- se pueden "precalentar" con init() al arrancar el server.

#[derive(Serialize)]
pub(crate) struct FallbackPayload<'a> {
    pub status: u16,
    #[serde(rename = "statusMessage")]
    pub status_message: &'a str,
    pub message: &'a str,
    pub name: &'a str,
}

pub(crate) const FALLBACK_PAYLOAD: FallbackPayload<'static> = FallbackPayload {
    status: 500,
    status_message: "Internal Server Error",
    message: "failed to serialize response",
    name: "ForgeError",
};

pub(crate) fn fallback_json_bytes() -> &'static Bytes {
    static CELL: OnceLock<Bytes> = OnceLock::new();
    CELL.get_or_init(|| {
        let bytes = serde_json::to_vec(&FALLBACK_PAYLOAD)
            .unwrap_or_else(|_| br#"{"status":500,"message":"failed to serialize response"}"#.to_vec());
        Bytes::from(bytes)
    })
}

pub(crate) fn fallback_msgpack_bytes() -> &'static Bytes {
    static CELL: OnceLock<Bytes> = OnceLock::new();
    CELL.get_or_init(|| {
        let bytes = rmp_serde::to_vec_named(&FALLBACK_PAYLOAD).unwrap_or_else(|_| fallback_json_bytes().to_vec());
        Bytes::from(bytes)
    })
}

/// Fuerza el cómputo de todos los bytes de fallback al arrancar el server,
/// para que el primer error real de un request no pague ese costo (mínimo,
/// pero evitable) y para detectar temprano cualquier problema de
/// serialización de los fallbacks mismos.
///
/// Llamar una sola vez, al cargar el env (ej. en main, después de env::init()).
pub fn init() {
    let _ = fallback_json_bytes();
    let _ = fallback_msgpack_bytes();
}

// ─── Headers: lógica testeable sin necesitar un Request/Context real ───────
// (Incoming no se puede construir a mano en tests, así que esta lógica
// vive separada para poder testearla sobre un HeaderMap suelto.)

pub(crate) mod headers {
    use super::*;

    pub fn set(map: &mut header::HeaderMap, name: HeaderName, value: &str) -> Result<(), HttpError> {
        if let Some(current) = map.get(&name) {
            if current.as_bytes() == value.as_bytes() {
                return Ok(());
            }
        }
        let hv = HeaderValue::from_str(value)
            .map_err(|e| HttpError::bad_request(format!("invalid header value for '{}'", name)).caused_by(e))?;
        map.insert(name, hv);
        Ok(())
    }

    pub fn get<'a>(map: &'a header::HeaderMap, name: &HeaderName) -> Option<&'a str> {
        map.get(name).and_then(|v| v.to_str().ok())
    }

    pub fn remove(map: &mut header::HeaderMap, name: &HeaderName) -> Option<HeaderValue> {
        map.remove(name)
    }
}

// ─── Context ────────────────────────────────────────────────────────────────

pub struct Context {
    pub r: Request<Incoming>,
    pub w: Option<ResBody>,
    pub status: u16,
    pub locale: String,
    pub headers: header::HeaderMap,
    content_type: ContentType,
    accept: ContentType,
}

impl Context {
    pub fn new(req: Request<Incoming>) -> Self {
        let content_type = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(ContentType::from_header_value)
            .unwrap_or(ContentType::MsgPack);

        let accept = req
            .headers()
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .map(ContentType::from_accept_header)
            .unwrap_or(ContentType::MsgPack);

        let locale = req
            .headers()
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|v| v.trim().to_string())
            .unwrap_or_else(|| env().app.locale.clone());

        Self {
            r: req,
            w: None,
            status: 200,
            content_type,
            accept,
            locale,
            headers: header::HeaderMap::new(),
        }
    }

    pub fn content_type(&self) -> ContentType {
        self.content_type
    }

    pub fn accept(&self) -> ContentType {
        self.accept
    }

    // ─── Headers de salida ───────────────────────────────────────────────────

    /// Agrega o actualiza un header. Si ya tiene exactamente ese valor,
    /// no hace nada (evita un insert innecesario).
    pub fn set_header(&mut self, name: HeaderName, value: &str) -> Result<(), HttpError> {
        headers::set(&mut self.headers, name, value)
    }

    /// Busca un header de salida ya seteado.
    pub fn get_header(&self, name: &HeaderName) -> Option<&str> {
        headers::get(&self.headers, name)
    }

    /// Elimina un header de salida. Devuelve el valor anterior si existía.
    pub fn remove_header(&mut self, name: &HeaderName) -> Option<HeaderValue> {
        headers::remove(&mut self.headers, name)
    }

    // ─── Escritura de respuesta ─────────────────────────────────────────────

    /// Respuesta "cruda": bytes ya armados, sin tocar headers.
    /// Es el paso final que usan todas las demás funciones response_*.
    pub fn response_into(&mut self, status: u16, body: Bytes) -> Result<(), HttpError> {
        self.status = status;
        self.w = Some(ResBody::full(body));
        Ok(())
    }

    /// Serializa `data` según self.accept (msgpack/json por ahora), setea
    /// el Content-Type y arma la respuesta. Nunca propaga el error de
    /// serialización -- si falla, cae a los bytes de fallback 500.
    pub fn response<T: Serialize>(&mut self, status: u16, data: &T) -> Result<(), HttpError> {
        let (mime, bytes) = match self.accept {
            ContentType::Json => match serde_json::to_vec(data) {
                Ok(b) => ("application/json", Bytes::from(b)),
                Err(_) => return self.response_fallback(),
            },
            _ => match rmp_serde::to_vec_named(data) {
                Ok(b) => ("application/msgpack", Bytes::from(b)),
                Err(_) => return self.response_fallback(),
            },
        };

        self.headers
            .insert(header::CONTENT_TYPE, header::HeaderValue::from_static(mime));
        self.response_into(status, bytes)
    }

    fn response_fallback(&mut self) -> Result<(), HttpError> {
        let (mime, bytes) = match self.accept {
            ContentType::Json => ("application/json", fallback_json_bytes().clone()),
            _ => ("application/msgpack", fallback_msgpack_bytes().clone()),
        };
        self.headers
            .insert(header::CONTENT_TYPE, header::HeaderValue::from_static(mime));
        self.response_into(500, bytes)
    }

    /// HttpError ya implementa Serialize -- solo se delega a response(),
    /// el status sale del propio error.
    pub fn response_error(&mut self, e: HttpError) -> Result<(), HttpError> {
        let status = e.status.as_u16();
        self.response(status, &e)
    }

    // ─── Atajos 2xx (CRUD) ───────────────────────────────────────────────────

    pub fn response_ok<T: Serialize>(&mut self, data: &T) -> Result<(), HttpError> {
        self.response(200, data)
    }

    pub fn response_created<T: Serialize>(&mut self, data: &T) -> Result<(), HttpError> {
        self.response(201, data)
    }

    pub fn response_accepted<T: Serialize>(&mut self, data: &T) -> Result<(), HttpError> {
        self.response(202, data)
    }

    pub fn response_no_content(&mut self) -> Result<(), HttpError> {
        self.response_into(204, Bytes::new())
    }

    // ─── Streaming ───────────────────────────────────────────────────────────

    /// A diferencia de las response_*, sí puede fallar de verdad: una vez
    /// que se entrega el control al stream no hay "bytes de fallback" a
    /// los que caer, así que el error se propaga con normalidad.
    pub fn stream(&mut self, status: u16, stream: BoxStream) -> Result<(), HttpError> {
        self.status = status;
        self.w = Some(ResBody::stream(stream));
        Ok(())
    }

    /// Marca el final del stream. No hace nada por sí misma -- existe
    /// para que el controller pueda cerrar con `c.done()?; Ok(())` igual
    /// que con cualquier otra función response_*.
    pub fn done(&mut self) -> Result<(), HttpError> {
        Ok(())
    }

    // ─── Finalización ────────────────────────────────────────────────────────

    pub fn into_response(self) -> Response<ResBody> {
        let body = self.w.unwrap_or_else(|| ResBody::full(Bytes::new()));
        let mut builder = Response::builder().status(self.status);
        if let Some(headers) = builder.headers_mut() {
            *headers = self.headers;
        }
        builder
            .body(body)
            .unwrap_or_else(|_| Response::new(ResBody::full(fallback_json_bytes().clone())))
    }
}
