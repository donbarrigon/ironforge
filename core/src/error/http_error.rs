use crate::config::env;
use backtrace::Backtrace;
use http_body_util::Full;
use hyper::{Response, StatusCode, body::Bytes, header};
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::fmt;

// ─── IntoCause ───────────────────────────────────────────────────────────────

pub struct Empty;

pub trait IntoCause {
    fn into_cause(self) -> Option<Box<dyn std::error::Error>>;
}

impl IntoCause for Empty {
    fn into_cause(self) -> Option<Box<dyn std::error::Error>> {
        None
    }
}

impl<E: std::error::Error + 'static> IntoCause for E {
    fn into_cause(self) -> Option<Box<dyn std::error::Error>> {
        Some(Box::new(self))
    }
}

// ─── HttpError ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct HttpError {
    #[serde(serialize_with = "serialize_status")]
    pub status: StatusCode,

    #[serde(rename = "statusMessage", skip_serializing_if = "String::is_empty")]
    pub status_message: String,

    pub message: String,

    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,

    #[serde(skip_serializing_if = "String::is_empty")]
    pub stack: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.status.as_u16(), self.name, self.message)
    }
}

impl std::error::Error for HttpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl HttpError {
    pub fn new(status: StatusCode, message: impl Into<String>, cause: impl IntoCause) -> Self {
        let debug = env().app.debug;
        Self {
            status,
            status_message: status.canonical_reason().unwrap_or("").into(),
            message: message.into(),
            name: "ForgeError".into(),
            stack: if debug { get_stack() } else { String::new() },
            cause: if debug { create_cause(cause.into_cause()) } else { None },
            data: None,
        }
    }

    pub fn with_data(mut self, data: impl Serialize) -> Self {
        self.data = Some(serde_json::json!(data));
        self
    }

    // ─── Response ────────────────────────────────────────────────────────────

    pub fn response(&self) -> Response<Full<Bytes>> {
        let bytes = rmp_serde::to_vec_named(self)
            .unwrap_or_else(|e| format!(r#"{{"error":"Msgpack serialization error: {}"}}"#, e).into_bytes());

        Response::builder()
            .status(self.status)
            .header(header::CONTENT_TYPE, "application/msgpack")
            .body(Full::new(Bytes::from(bytes)))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
    }

    pub fn response_json(&self) -> Response<Full<Bytes>> {
        let bytes = serde_json::to_vec(self)
            .unwrap_or_else(|e| format!(r#"{{"error":"Json serialization error: {}"}}"#, e).into_bytes());

        Response::builder()
            .status(self.status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::new(Bytes::from(bytes)))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
    }

    // ─── 4xx Client Errors ───────────────────────────────────────────────────

    pub fn bad_request(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message, cause)
    }

    pub fn unauthorized(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message, cause)
    }

    pub fn payment_required(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::PAYMENT_REQUIRED, message, cause)
    }

    pub fn forbidden(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::FORBIDDEN, message, cause)
    }

    pub fn not_found(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::NOT_FOUND, message, cause)
    }

    pub fn method_not_allowed(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::METHOD_NOT_ALLOWED, message, cause)
    }

    pub fn not_acceptable(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::NOT_ACCEPTABLE, message, cause)
    }

    pub fn proxy_authentication_required(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::PROXY_AUTHENTICATION_REQUIRED, message, cause)
    }

    pub fn request_timeout(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::REQUEST_TIMEOUT, message, cause)
    }

    pub fn conflict(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::CONFLICT, message, cause)
    }

    pub fn gone(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::GONE, message, cause)
    }

    pub fn length_required(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::LENGTH_REQUIRED, message, cause)
    }

    pub fn precondition_failed(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::PRECONDITION_FAILED, message, cause)
    }

    pub fn payload_too_large(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::PAYLOAD_TOO_LARGE, message, cause)
    }

    pub fn uri_too_long(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::URI_TOO_LONG, message, cause)
    }

    pub fn unsupported_media_type(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::UNSUPPORTED_MEDIA_TYPE, message, cause)
    }

    pub fn range_not_satisfiable(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::RANGE_NOT_SATISFIABLE, message, cause)
    }

    pub fn expectation_failed(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::EXPECTATION_FAILED, message, cause)
    }

    pub fn im_a_teapot(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::IM_A_TEAPOT, message, cause)
    }

    pub fn misdirected_request(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::MISDIRECTED_REQUEST, message, cause)
    }

    pub fn unprocessable_entity(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, message, cause)
    }

    pub fn locked(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::LOCKED, message, cause)
    }

    pub fn failed_dependency(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::FAILED_DEPENDENCY, message, cause)
    }

    pub fn upgrade_required(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::UPGRADE_REQUIRED, message, cause)
    }

    pub fn precondition_required(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::PRECONDITION_REQUIRED, message, cause)
    }

    pub fn too_many_requests(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, message, cause)
    }

    pub fn request_header_fields_too_large(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE, message, cause)
    }

    pub fn unavailable_for_legal_reasons(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS, message, cause)
    }

    // ─── 5xx Server Errors ───────────────────────────────────────────────────

    pub fn internal_server_error(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message, cause)
    }

    pub fn not_implemented(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::NOT_IMPLEMENTED, message, cause)
    }

    pub fn bad_gateway(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, message, cause)
    }

    pub fn service_unavailable(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, message, cause)
    }

    pub fn gateway_timeout(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::GATEWAY_TIMEOUT, message, cause)
    }

    pub fn http_version_not_supported(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::HTTP_VERSION_NOT_SUPPORTED, message, cause)
    }

    pub fn variant_also_negotiates(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::VARIANT_ALSO_NEGOTIATES, message, cause)
    }

    pub fn insufficient_storage(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::INSUFFICIENT_STORAGE, message, cause)
    }

    pub fn loop_detected(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::LOOP_DETECTED, message, cause)
    }

    pub fn not_extended(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::NOT_EXTENDED, message, cause)
    }

    pub fn network_authentication_required(message: impl Into<String>, cause: impl IntoCause) -> Self {
        Self::new(StatusCode::NETWORK_AUTHENTICATION_REQUIRED, message, cause)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn get_stack() -> String {
    if cfg!(debug_assertions) {
        format!("{:?}", Backtrace::new())
    } else {
        String::new()
    }
}

fn create_cause(cause: Option<Box<dyn std::error::Error>>) -> Option<Value> {
    cause.map(|e| {
        serde_json::json!({
            "name":    std::any::type_name_of_val(&*e),
            "message": e.to_string(),
            "source":  e.source().map(|s| s.to_string()),
        })
    })
}

fn serialize_status<S>(status: &StatusCode, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    status.as_u16().serialize(serializer)
}
