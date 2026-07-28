use hyper::body::Bytes;
use hyper::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Serialize;
use std::sync::OnceLock;

use crate::error::ForgeError;

// ─── ContentType ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContentType {
    MsgPack,
    Json,
    Yaml,
    Form,
    Html,
    Csv,
    PlainText,
    Xml,
}

impl ContentType {
    /// Parsea un solo valor de mime type, ignorando parámetros
    /// como ";charset=utf-8". Cualquier valor no reconocido cae en
    /// MsgPack (el default del framework), nunca en None.
    pub fn from_header_value(v: &str) -> Self {
        let mime = v.split(';').next().unwrap_or("").trim();
        match mime {
            "application/msgpack" => ContentType::MsgPack,
            "application/json" => ContentType::Json,
            "application/yaml" | "text/yaml" => ContentType::Yaml,
            "application/x-www-form-urlencoded" => ContentType::Form,
            "text/html" => ContentType::Html,
            "text/csv" => ContentType::Csv,
            "text/plain" => ContentType::PlainText,
            "application/xml" | "text/xml" => ContentType::Xml,
            _ => ContentType::MsgPack,
        }
    }

    /// Un Accept puede traer varios valores separados por coma;
    /// se toma el primero (sin ponderar q-values todavía).
    pub fn from_accept_header(v: &str) -> Self {
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
            ContentType::Form => "application/x-www-form-urlencoded",
            ContentType::Html => "text/html",
            ContentType::Csv => "text/csv",
            ContentType::PlainText => "text/plain",
            ContentType::Xml => "application/xml",
        }
    }
}

// ─── Manejo de headers de salida (add/update, get, remove) ─────────────────
// Viven como funciones libres sobre &mut HeaderMap (en vez de métodos de
// Context) para poder testearlas sin necesitar un Request/Incoming real.

/// Agrega o actualiza un header. Si ya tiene exactamente ese valor,
/// no hace nada (evita un insert innecesario).
pub fn set(map: &mut HeaderMap, name: HeaderName, value: &str) -> Result<(), ForgeError> {
    if let Some(current) = map.get(&name) {
        if current.as_bytes() == value.as_bytes() {
            return Ok(());
        }
    }
    let hv = HeaderValue::from_str(value)
        .map_err(|e| ForgeError::bad_request(format!("invalid header value for '{}'", name)).caused_by(e))?;
    map.insert(name, hv);
    Ok(())
}

/// Busca un header.
pub fn get<'a>(map: &'a HeaderMap, name: &HeaderName) -> Option<&'a str> {
    map.get(name).and_then(|v| v.to_str().ok())
}

/// Elimina un header. Devuelve el valor anterior si existía.
pub fn remove(map: &mut HeaderMap, name: &HeaderName) -> Option<HeaderValue> {
    map.remove(name)
}

// ─── Fallback bytes (500 por fallo de serialización) ───────────────────────
// Todos se computan una única vez (OnceLock) y quedan cacheados el resto
// del proceso -- se pueden "precalentar" con init() al arrancar el server.

#[derive(Serialize)]
struct FallbackPayload<'a> {
    status: u16,
    #[serde(rename = "statusMessage")]
    status_message: &'a str,
    message: &'a str,
    name: &'a str,
}

const FALLBACK_PAYLOAD: FallbackPayload<'static> = FallbackPayload {
    status: 500,
    status_message: "Internal Server Error",
    message: "failed to serialize response",
    name: "ForgeError",
};

pub fn fallback_json_bytes() -> &'static Bytes {
    static CELL: OnceLock<Bytes> = OnceLock::new();
    CELL.get_or_init(|| {
        let bytes = serde_json::to_vec(&FALLBACK_PAYLOAD)
            .unwrap_or_else(|_| br#"{"status":500,"message":"failed to serialize response"}"#.to_vec());
        Bytes::from(bytes)
    })
}

pub fn fallback_msgpack_bytes() -> &'static Bytes {
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
