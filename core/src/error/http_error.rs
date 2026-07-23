use crate::{config::env, handler::ResBody};
use backtrace::Backtrace;
use http_body_util::Full;
use hyper::{Response, StatusCode, body::Bytes, header};
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::fmt;

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
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        let debug = env().app.debug;
        Self {
            status,
            status_message: status.canonical_reason().unwrap_or("").into(),
            message: message.into(),
            name: "ForgeError".into(),
            stack: if debug { get_stack() } else { String::new() },
            cause: None,
            data: None,
        }
    }

    // ─── Builder ──────────────────────────────────────────────────────────────

    /// Adjunta la causa del error (solo visible en modo debug)
    pub fn caused_by(mut self, cause: impl std::error::Error + 'static) -> Self {
        if env().app.debug {
            self.cause = create_cause(Some(Box::new(cause)));
        }
        self
    }

    /// Adjunta data adicional al error
    pub fn with_data(mut self, data: impl Serialize) -> Self {
        self.data = Some(serde_json::json!(data));
        self
    }

    // ─── Response ────────────────────────────────────────────────────────────

    pub fn response(&self) -> Response<ResBody> {
        let bytes = rmp_serde::to_vec_named(self)
            .unwrap_or_else(|e| format!(r#"{{"error":"Msgpack serialization error: {}"}}"#, e).into_bytes());

        Response::builder()
            .status(self.status)
            .header(header::CONTENT_TYPE, "application/msgpack")
            .body(ResBody::full(Bytes::from(bytes)))
            .unwrap_or_else(|_| Response::new(ResBody::full(Bytes::new())))
    }

    pub fn response_json(&self) -> Response<ResBody> {
        let bytes = serde_json::to_vec(self)
            .unwrap_or_else(|e| format!(r#"{{"error":"Json serialization error: {}"}}"#, e).into_bytes());

        Response::builder()
            .status(self.status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(ResBody::full(Bytes::from(bytes)))
            .unwrap_or_else(|_| Response::new(ResBody::full(Bytes::new())))
    }

    // ─── 4xx Client Errors ───────────────────────────────────────────────────

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }

    pub fn payment_required(message: impl Into<String>) -> Self {
        Self::new(StatusCode::PAYMENT_REQUIRED, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub fn method_not_allowed(message: impl Into<String>) -> Self {
        Self::new(StatusCode::METHOD_NOT_ALLOWED, message)
    }

    pub fn not_acceptable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_ACCEPTABLE, message)
    }

    pub fn proxy_authentication_required(message: impl Into<String>) -> Self {
        Self::new(StatusCode::PROXY_AUTHENTICATION_REQUIRED, message)
    }

    pub fn request_timeout(message: impl Into<String>) -> Self {
        Self::new(StatusCode::REQUEST_TIMEOUT, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    pub fn gone(message: impl Into<String>) -> Self {
        Self::new(StatusCode::GONE, message)
    }

    pub fn length_required(message: impl Into<String>) -> Self {
        Self::new(StatusCode::LENGTH_REQUIRED, message)
    }

    pub fn precondition_failed(message: impl Into<String>) -> Self {
        Self::new(StatusCode::PRECONDITION_FAILED, message)
    }

    pub fn payload_too_large(message: impl Into<String>) -> Self {
        Self::new(StatusCode::PAYLOAD_TOO_LARGE, message)
    }

    pub fn uri_too_long(message: impl Into<String>) -> Self {
        Self::new(StatusCode::URI_TOO_LONG, message)
    }

    pub fn unsupported_media_type(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNSUPPORTED_MEDIA_TYPE, message)
    }

    pub fn range_not_satisfiable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::RANGE_NOT_SATISFIABLE, message)
    }

    pub fn expectation_failed(message: impl Into<String>) -> Self {
        Self::new(StatusCode::EXPECTATION_FAILED, message)
    }

    pub fn im_a_teapot(message: impl Into<String>) -> Self {
        Self::new(StatusCode::IM_A_TEAPOT, message)
    }

    pub fn misdirected_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::MISDIRECTED_REQUEST, message)
    }

    pub fn unprocessable_entity(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, message)
    }

    pub fn locked(message: impl Into<String>) -> Self {
        Self::new(StatusCode::LOCKED, message)
    }

    pub fn failed_dependency(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FAILED_DEPENDENCY, message)
    }

    pub fn upgrade_required(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UPGRADE_REQUIRED, message)
    }

    pub fn precondition_required(message: impl Into<String>) -> Self {
        Self::new(StatusCode::PRECONDITION_REQUIRED, message)
    }

    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, message)
    }

    pub fn request_header_fields_too_large(message: impl Into<String>) -> Self {
        Self::new(StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE, message)
    }

    pub fn unavailable_for_legal_reasons(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS, message)
    }

    // ─── 5xx Server Errors ───────────────────────────────────────────────────

    pub fn internal_server_error(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_IMPLEMENTED, message)
    }

    pub fn bad_gateway(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, message)
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, message)
    }

    pub fn gateway_timeout(message: impl Into<String>) -> Self {
        Self::new(StatusCode::GATEWAY_TIMEOUT, message)
    }

    pub fn http_version_not_supported(message: impl Into<String>) -> Self {
        Self::new(StatusCode::HTTP_VERSION_NOT_SUPPORTED, message)
    }

    pub fn variant_also_negotiates(message: impl Into<String>) -> Self {
        Self::new(StatusCode::VARIANT_ALSO_NEGOTIATES, message)
    }

    pub fn insufficient_storage(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INSUFFICIENT_STORAGE, message)
    }

    pub fn loop_detected(message: impl Into<String>) -> Self {
        Self::new(StatusCode::LOOP_DETECTED, message)
    }

    pub fn not_extended(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_EXTENDED, message)
    }

    pub fn network_authentication_required(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NETWORK_AUTHENTICATION_REQUIRED, message)
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
