use hyper::body::Bytes;
use hyper::{Request, Response, body::Incoming, header};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{
    config::env,
    error::ForgeError,
    handler::{
        BoxStream, ResBody,
        headers::{self, ContentType},
        validator::Validator,
    },
};

pub struct Context {
    pub r: Request<Incoming>,
    pub w: Option<ResBody>,
    pub status: u16,
    pub locale: String,
    pub headers: header::HeaderMap,
    body_cache: Option<Bytes>,
}

impl Context {
    pub fn new(req: Request<Incoming>) -> Self {
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
            locale,
            headers: header::HeaderMap::new(),
            body_cache: None,
        }
    }

    /// Content-Type de lo que llegó en el request. Similar que accept()
    pub fn content_type(&self) -> ContentType {
        self.r
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(ContentType::from_header_value)
            .unwrap_or(ContentType::MsgPack)
    }

    /// Formato que el cliente espera de vuelta. Similar que content_type()
    pub fn accept(&self) -> ContentType {
        self.r
            .headers()
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .map(ContentType::from_accept_header)
            .unwrap_or(ContentType::MsgPack)
    }

    // ─── Headers de salida ───────────────────────────────────────────────────

    pub fn set_header(&mut self, name: header::HeaderName, value: &str) -> Result<(), ForgeError> {
        headers::set(&mut self.headers, name, value)
    }

    pub fn get_header(&self, name: &header::HeaderName) -> Option<&str> {
        headers::get(&self.headers, name)
    }

    pub fn remove_header(&mut self, name: &header::HeaderName) -> Option<header::HeaderValue> {
        headers::remove(&mut self.headers, name)
    }

    // ─── Lectura del body ────────────────────────────────────────────────────

    /// Lee y bufferea el body crudo. Cachea el resultado -- Incoming es un
    /// stream que solo se puede consumir una vez, así que si algo ya lo
    /// leyó (este mismo método, llamado antes), se devuelve la copia
    /// cacheada en vez de volver a leer del socket.
    pub async fn raw_body(&mut self) -> Result<Bytes, ForgeError> {
        if let Some(cached) = &self.body_cache {
            return Ok(cached.clone()); // Bytes::clone es barato (Arc por dentro)
        }
        let collected = http_body_util::BodyExt::collect(self.r.body_mut())
            .await
            .map_err(|e| ForgeError::bad_request("failed to read request body").caused_by(e))?;
        let bytes = collected.to_bytes();
        self.body_cache = Some(bytes.clone());
        Ok(bytes)
    }

    fn decode<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, ForgeError> {
        match self.content_type() {
            ContentType::MsgPack => {
                rmp_serde::from_slice(bytes).map_err(|e| ForgeError::bad_request("invalid msgpack body").caused_by(e))
            }
            ContentType::Json => {
                serde_json::from_slice(bytes).map_err(|e| ForgeError::bad_request("invalid json body").caused_by(e))
            }
            ContentType::Form => serde_urlencoded::from_bytes(bytes)
                .map_err(|e| ForgeError::bad_request("invalid form body").caused_by(e)),
            ContentType::Yaml => {
                serde_yaml::from_slice(bytes).map_err(|e| ForgeError::bad_request("invalid yaml body").caused_by(e))
            }
            other => Err(ForgeError::unsupported_media_type(format!(
                "cannot decode body as struct from content-type '{}'",
                other.mime()
            ))),
        }
    }

    /// Lee el body, lo decodifica según el Content-Type (json/msgpack/form/yaml)
    /// a T, y corre sus reglas de validación (T::rules(), que a su vez llama
    /// su propio prepare_for_validation()). Funciona también con
    /// `get_body::<Vec<Item>>()` gracias al impl de Validator para Vec<T>.
    pub async fn get_body<T>(&mut self) -> Result<T, ForgeError>
    where
        T: DeserializeOwned + Validator,
    {
        let bytes = self.raw_body().await?;
        let mut data: T = self.decode(&bytes)?;
        data.rules()?;
        Ok(data)
    }

    // TODO: get_body_multipart<T>() -- multipart/form-data (json + archivos).
    // Se implementará sin usar el crate `multer`, parseando el boundary y el
    // stream a mano para tener control total (streaming de archivos a disco
    // sin pasar por RAM, límites de tamaño propios, etc).

    // ─── Escritura de respuesta ─────────────────────────────────────────────

    pub fn response_into(&mut self, status: u16, body: Bytes) -> Result<(), ForgeError> {
        self.status = status;
        self.w = Some(ResBody::full(body));
        Ok(())
    }

    pub fn response<T: Serialize>(&mut self, status: u16, data: &T) -> Result<(), ForgeError> {
        let (mime, bytes) = match self.accept() {
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

    fn response_fallback(&mut self) -> Result<(), ForgeError> {
        let (mime, bytes) = match self.accept() {
            ContentType::Json => ("application/json", headers::fallback_json_bytes().clone()),
            _ => ("application/msgpack", headers::fallback_msgpack_bytes().clone()),
        };
        self.headers
            .insert(header::CONTENT_TYPE, header::HeaderValue::from_static(mime));
        self.response_into(500, bytes)
    }

    pub fn response_error(&mut self, e: ForgeError) -> Result<(), ForgeError> {
        let status = e.status.as_u16();
        self.response(status, &e)
    }

    // ─── Atajos 2xx (CRUD) ───────────────────────────────────────────────────

    pub fn response_ok<T: Serialize>(&mut self, data: &T) -> Result<(), ForgeError> {
        self.response(200, data)
    }

    pub fn response_created<T: Serialize>(&mut self, data: &T) -> Result<(), ForgeError> {
        self.response(201, data)
    }

    pub fn response_accepted<T: Serialize>(&mut self, data: &T) -> Result<(), ForgeError> {
        self.response(202, data)
    }

    pub fn response_no_content(&mut self) -> Result<(), ForgeError> {
        self.response_into(204, Bytes::new())
    }

    // ─── Streaming ───────────────────────────────────────────────────────────
    // Nota: a diferencia de response()/response_ok()/etc, acá NO se setea
    // ningún header automáticamente (ni Content-Type). El controlador es
    // responsable de llamar set_header() antes si lo necesita.

    pub fn stream(&mut self, status: u16, stream: BoxStream) -> Result<(), ForgeError> {
        self.status = status;
        self.w = Some(ResBody::stream(stream));
        Ok(())
    }

    pub fn done(&mut self) -> Result<(), ForgeError> {
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
            .unwrap_or_else(|_| Response::new(ResBody::full(headers::fallback_json_bytes().clone())))
    }
}
