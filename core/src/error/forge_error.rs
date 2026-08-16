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
    // en compile-time, sin necesitar una instancia de StatusCode en la mano,
    // y desacoplada de que hyper cambie el texto en el futuro.

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

    /// Sobreescribe el mensaje por defecto con uno propio.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

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

    pub fn bad_request() -> Self {
        Self::new(Self::BAD_REQUEST, Self::BAD_REQUEST_MSG)
    }

    pub fn unauthorized() -> Self {
        Self::new(Self::UNAUTHORIZED, Self::UNAUTHORIZED_MSG)
    }

    pub fn payment_required() -> Self {
        Self::new(Self::PAYMENT_REQUIRED, Self::PAYMENT_REQUIRED_MSG)
    }

    pub fn forbidden() -> Self {
        Self::new(Self::FORBIDDEN, Self::FORBIDDEN_MSG)
    }

    pub fn not_found() -> Self {
        Self::new(Self::NOT_FOUND, Self::NOT_FOUND_MSG)
    }

    pub fn method_not_allowed() -> Self {
        Self::new(Self::METHOD_NOT_ALLOWED, Self::METHOD_NOT_ALLOWED_MSG)
    }

    pub fn not_acceptable() -> Self {
        Self::new(Self::NOT_ACCEPTABLE, Self::NOT_ACCEPTABLE_MSG)
    }

    pub fn proxy_authentication_required() -> Self {
        Self::new(
            Self::PROXY_AUTHENTICATION_REQUIRED,
            Self::PROXY_AUTHENTICATION_REQUIRED_MSG,
        )
    }

    pub fn request_timeout() -> Self {
        Self::new(Self::REQUEST_TIMEOUT, Self::REQUEST_TIMEOUT_MSG)
    }

    pub fn conflict() -> Self {
        Self::new(Self::CONFLICT, Self::CONFLICT_MSG)
    }

    pub fn gone() -> Self {
        Self::new(Self::GONE, Self::GONE_MSG)
    }

    pub fn length_required() -> Self {
        Self::new(Self::LENGTH_REQUIRED, Self::LENGTH_REQUIRED_MSG)
    }

    pub fn precondition_failed() -> Self {
        Self::new(Self::PRECONDITION_FAILED, Self::PRECONDITION_FAILED_MSG)
    }

    pub fn payload_too_large() -> Self {
        Self::new(Self::PAYLOAD_TOO_LARGE, Self::PAYLOAD_TOO_LARGE_MSG)
    }

    pub fn uri_too_long() -> Self {
        Self::new(Self::URI_TOO_LONG, Self::URI_TOO_LONG_MSG)
    }

    pub fn unsupported_media_type() -> Self {
        Self::new(Self::UNSUPPORTED_MEDIA_TYPE, Self::UNSUPPORTED_MEDIA_TYPE_MSG)
    }

    pub fn range_not_satisfiable() -> Self {
        Self::new(Self::RANGE_NOT_SATISFIABLE, Self::RANGE_NOT_SATISFIABLE_MSG)
    }

    pub fn expectation_failed() -> Self {
        Self::new(Self::EXPECTATION_FAILED, Self::EXPECTATION_FAILED_MSG)
    }

    pub fn im_a_teapot() -> Self {
        Self::new(Self::IM_A_TEAPOT, Self::IM_A_TEAPOT_MSG)
    }

    pub fn misdirected_request() -> Self {
        Self::new(Self::MISDIRECTED_REQUEST, Self::MISDIRECTED_REQUEST_MSG)
    }

    pub fn unprocessable_entity() -> Self {
        Self::new(Self::UNPROCESSABLE_ENTITY, Self::UNPROCESSABLE_ENTITY_MSG)
    }

    pub fn locked() -> Self {
        Self::new(Self::LOCKED, Self::LOCKED_MSG)
    }

    pub fn failed_dependency() -> Self {
        Self::new(Self::FAILED_DEPENDENCY, Self::FAILED_DEPENDENCY_MSG)
    }

    pub fn upgrade_required() -> Self {
        Self::new(Self::UPGRADE_REQUIRED, Self::UPGRADE_REQUIRED_MSG)
    }

    pub fn precondition_required() -> Self {
        Self::new(Self::PRECONDITION_REQUIRED, Self::PRECONDITION_REQUIRED_MSG)
    }

    pub fn too_many_requests() -> Self {
        Self::new(Self::TOO_MANY_REQUESTS, Self::TOO_MANY_REQUESTS_MSG)
    }

    pub fn request_header_fields_too_large() -> Self {
        Self::new(
            Self::REQUEST_HEADER_FIELDS_TOO_LARGE,
            Self::REQUEST_HEADER_FIELDS_TOO_LARGE_MSG,
        )
    }

    pub fn unavailable_for_legal_reasons() -> Self {
        Self::new(
            Self::UNAVAILABLE_FOR_LEGAL_REASONS,
            Self::UNAVAILABLE_FOR_LEGAL_REASONS_MSG,
        )
    }

    // ─── 5xx Server Errors ───────────────────────────────────────────────────

    pub fn internal() -> Self {
        Self::new(Self::INTERNAL_SERVER_ERROR, Self::INTERNAL_SERVER_ERROR_MSG)
    }

    pub fn not_implemented() -> Self {
        Self::new(Self::NOT_IMPLEMENTED, Self::NOT_IMPLEMENTED_MSG)
    }

    pub fn bad_gateway() -> Self {
        Self::new(Self::BAD_GATEWAY, Self::BAD_GATEWAY_MSG)
    }

    pub fn service_unavailable() -> Self {
        Self::new(Self::SERVICE_UNAVAILABLE, Self::SERVICE_UNAVAILABLE_MSG)
    }

    pub fn gateway_timeout() -> Self {
        Self::new(Self::GATEWAY_TIMEOUT, Self::GATEWAY_TIMEOUT_MSG)
    }

    pub fn http_version_not_supported() -> Self {
        Self::new(Self::HTTP_VERSION_NOT_SUPPORTED, Self::HTTP_VERSION_NOT_SUPPORTED_MSG)
    }

    pub fn variant_also_negotiates() -> Self {
        Self::new(Self::VARIANT_ALSO_NEGOTIATES, Self::VARIANT_ALSO_NEGOTIATES_MSG)
    }

    pub fn insufficient_storage() -> Self {
        Self::new(Self::INSUFFICIENT_STORAGE, Self::INSUFFICIENT_STORAGE_MSG)
    }

    pub fn loop_detected() -> Self {
        Self::new(Self::LOOP_DETECTED, Self::LOOP_DETECTED_MSG)
    }

    pub fn not_extended() -> Self {
        Self::new(Self::NOT_EXTENDED, Self::NOT_EXTENDED_MSG)
    }

    pub fn network_authentication_required() -> Self {
        Self::new(
            Self::NETWORK_AUTHENTICATION_REQUIRED,
            Self::NETWORK_AUTHENTICATION_REQUIRED_MSG,
        )
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
