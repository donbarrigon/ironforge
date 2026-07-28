use crate::config::env;
use backtrace::Backtrace;
use hyper::StatusCode;
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::fmt;

// ─── ForgeError ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ForgeError {
    #[serde(serialize_with = "serialize_status")]
    pub status: StatusCode,

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

impl fmt::Display for ForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.status.as_u16(), self.name, self.message)
    }
}

impl std::error::Error for ForgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl ForgeError {
    // ─── Status code constants ──────────────────────────────────────────────
    // Alias directo a los StatusCode que este framework usa activamente
    // (uno por cada método de conveniencia de más abajo). Permiten referirse
    // al código sin importar hyper::StatusCode ni recordar el número.

    pub const BAD_REQUEST: StatusCode = StatusCode::BAD_REQUEST;
    pub const UNAUTHORIZED: StatusCode = StatusCode::UNAUTHORIZED;
    pub const PAYMENT_REQUIRED: StatusCode = StatusCode::PAYMENT_REQUIRED;
    pub const FORBIDDEN: StatusCode = StatusCode::FORBIDDEN;
    pub const NOT_FOUND: StatusCode = StatusCode::NOT_FOUND;
    pub const METHOD_NOT_ALLOWED: StatusCode = StatusCode::METHOD_NOT_ALLOWED;
    pub const NOT_ACCEPTABLE: StatusCode = StatusCode::NOT_ACCEPTABLE;
    pub const PROXY_AUTHENTICATION_REQUIRED: StatusCode = StatusCode::PROXY_AUTHENTICATION_REQUIRED;
    pub const REQUEST_TIMEOUT: StatusCode = StatusCode::REQUEST_TIMEOUT;
    pub const CONFLICT: StatusCode = StatusCode::CONFLICT;
    pub const GONE: StatusCode = StatusCode::GONE;
    pub const LENGTH_REQUIRED: StatusCode = StatusCode::LENGTH_REQUIRED;
    pub const PRECONDITION_FAILED: StatusCode = StatusCode::PRECONDITION_FAILED;
    pub const PAYLOAD_TOO_LARGE: StatusCode = StatusCode::PAYLOAD_TOO_LARGE;
    pub const URI_TOO_LONG: StatusCode = StatusCode::URI_TOO_LONG;
    pub const UNSUPPORTED_MEDIA_TYPE: StatusCode = StatusCode::UNSUPPORTED_MEDIA_TYPE;
    pub const RANGE_NOT_SATISFIABLE: StatusCode = StatusCode::RANGE_NOT_SATISFIABLE;
    pub const EXPECTATION_FAILED: StatusCode = StatusCode::EXPECTATION_FAILED;
    pub const IM_A_TEAPOT: StatusCode = StatusCode::IM_A_TEAPOT;
    pub const MISDIRECTED_REQUEST: StatusCode = StatusCode::MISDIRECTED_REQUEST;
    pub const UNPROCESSABLE_ENTITY: StatusCode = StatusCode::UNPROCESSABLE_ENTITY;
    pub const LOCKED: StatusCode = StatusCode::LOCKED;
    pub const FAILED_DEPENDENCY: StatusCode = StatusCode::FAILED_DEPENDENCY;
    pub const UPGRADE_REQUIRED: StatusCode = StatusCode::UPGRADE_REQUIRED;
    pub const PRECONDITION_REQUIRED: StatusCode = StatusCode::PRECONDITION_REQUIRED;
    pub const TOO_MANY_REQUESTS: StatusCode = StatusCode::TOO_MANY_REQUESTS;
    pub const REQUEST_HEADER_FIELDS_TOO_LARGE: StatusCode = StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE;
    pub const UNAVAILABLE_FOR_LEGAL_REASONS: StatusCode = StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS;
    pub const INTERNAL_SERVER_ERROR: StatusCode = StatusCode::INTERNAL_SERVER_ERROR;
    pub const NOT_IMPLEMENTED: StatusCode = StatusCode::NOT_IMPLEMENTED;
    pub const BAD_GATEWAY: StatusCode = StatusCode::BAD_GATEWAY;
    pub const SERVICE_UNAVAILABLE: StatusCode = StatusCode::SERVICE_UNAVAILABLE;
    pub const GATEWAY_TIMEOUT: StatusCode = StatusCode::GATEWAY_TIMEOUT;
    pub const HTTP_VERSION_NOT_SUPPORTED: StatusCode = StatusCode::HTTP_VERSION_NOT_SUPPORTED;
    pub const VARIANT_ALSO_NEGOTIATES: StatusCode = StatusCode::VARIANT_ALSO_NEGOTIATES;
    pub const INSUFFICIENT_STORAGE: StatusCode = StatusCode::INSUFFICIENT_STORAGE;
    pub const LOOP_DETECTED: StatusCode = StatusCode::LOOP_DETECTED;
    pub const NOT_EXTENDED: StatusCode = StatusCode::NOT_EXTENDED;
    pub const NETWORK_AUTHENTICATION_REQUIRED: StatusCode = StatusCode::NETWORK_AUTHENTICATION_REQUIRED;

    // ─── Status message constants ───────────────────────────────────────────
    // Texto canónico en inglés (idéntico a StatusCode::canonical_reason(),
    // verificado contra el crate `http`), pero como const propia: accesible
    // en compile-time, sin necesitar una instancia de StatusCode en la mano.

    pub const BAD_REQUEST_MSG: &'static str = "Bad Request";
    pub const UNAUTHORIZED_MSG: &'static str = "Unauthorized";
    pub const PAYMENT_REQUIRED_MSG: &'static str = "Payment Required";
    pub const FORBIDDEN_MSG: &'static str = "Forbidden";
    pub const NOT_FOUND_MSG: &'static str = "Not Found";
    pub const METHOD_NOT_ALLOWED_MSG: &'static str = "Method Not Allowed";
    pub const NOT_ACCEPTABLE_MSG: &'static str = "Not Acceptable";
    pub const PROXY_AUTHENTICATION_REQUIRED_MSG: &'static str = "Proxy Authentication Required";
    pub const REQUEST_TIMEOUT_MSG: &'static str = "Request Timeout";
    pub const CONFLICT_MSG: &'static str = "Conflict";
    pub const GONE_MSG: &'static str = "Gone";
    pub const LENGTH_REQUIRED_MSG: &'static str = "Length Required";
    pub const PRECONDITION_FAILED_MSG: &'static str = "Precondition Failed";
    pub const PAYLOAD_TOO_LARGE_MSG: &'static str = "Payload Too Large";
    pub const URI_TOO_LONG_MSG: &'static str = "URI Too Long";
    pub const UNSUPPORTED_MEDIA_TYPE_MSG: &'static str = "Unsupported Media Type";
    pub const RANGE_NOT_SATISFIABLE_MSG: &'static str = "Range Not Satisfiable";
    pub const EXPECTATION_FAILED_MSG: &'static str = "Expectation Failed";
    pub const IM_A_TEAPOT_MSG: &'static str = "I'm a teapot";
    pub const MISDIRECTED_REQUEST_MSG: &'static str = "Misdirected Request";
    pub const UNPROCESSABLE_ENTITY_MSG: &'static str = "Unprocessable Entity";
    pub const LOCKED_MSG: &'static str = "Locked";
    pub const FAILED_DEPENDENCY_MSG: &'static str = "Failed Dependency";
    pub const UPGRADE_REQUIRED_MSG: &'static str = "Upgrade Required";
    pub const PRECONDITION_REQUIRED_MSG: &'static str = "Precondition Required";
    pub const TOO_MANY_REQUESTS_MSG: &'static str = "Too Many Requests";
    pub const REQUEST_HEADER_FIELDS_TOO_LARGE_MSG: &'static str = "Request Header Fields Too Large";
    pub const UNAVAILABLE_FOR_LEGAL_REASONS_MSG: &'static str = "Unavailable For Legal Reasons";
    pub const INTERNAL_SERVER_ERROR_MSG: &'static str = "Internal Server Error";
    pub const NOT_IMPLEMENTED_MSG: &'static str = "Not Implemented";
    pub const BAD_GATEWAY_MSG: &'static str = "Bad Gateway";
    pub const SERVICE_UNAVAILABLE_MSG: &'static str = "Service Unavailable";
    pub const GATEWAY_TIMEOUT_MSG: &'static str = "Gateway Timeout";
    pub const HTTP_VERSION_NOT_SUPPORTED_MSG: &'static str = "HTTP Version Not Supported";
    pub const VARIANT_ALSO_NEGOTIATES_MSG: &'static str = "Variant Also Negotiates";
    pub const INSUFFICIENT_STORAGE_MSG: &'static str = "Insufficient Storage";
    pub const LOOP_DETECTED_MSG: &'static str = "Loop Detected";
    pub const NOT_EXTENDED_MSG: &'static str = "Not Extended";
    pub const NETWORK_AUTHENTICATION_REQUIRED_MSG: &'static str = "Network Authentication Required";

    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        let debug = env().app.debug;
        Self {
            status,
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
