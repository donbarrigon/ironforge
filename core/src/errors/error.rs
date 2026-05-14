use backtrace::Backtrace;
use hyper::StatusCode;
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, Serialize)]
pub struct Error {
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.status.as_u16(), self.name, self.message)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl Error {
    pub fn new(status: StatusCode, message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self {
            status,
            status_message: status.canonical_reason().unwrap_or("").into(),
            message: message.into(),
            name: "ForgeError".into(),
            stack: get_stack(),
            cause: create_cause(cause),
            data: None,
        };
    }

    // ─── 4xx Client Errors ───────────────────────────────────────────────────

    pub fn bad_request(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::BAD_REQUEST, message, cause);
    }

    pub fn unauthorized(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::UNAUTHORIZED, message, cause);
    }

    pub fn payment_required(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::PAYMENT_REQUIRED, message, cause);
    }

    pub fn forbidden(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::FORBIDDEN, message, cause);
    }

    pub fn not_found(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::NOT_FOUND, message, cause);
    }

    pub fn method_not_allowed(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::METHOD_NOT_ALLOWED, message, cause);
    }

    pub fn not_acceptable(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::NOT_ACCEPTABLE, message, cause);
    }

    pub fn proxy_authentication_required(
        message: impl Into<String>,
        cause: Option<Box<dyn std::error::Error>>,
    ) -> Self {
        return Self::new(StatusCode::PROXY_AUTHENTICATION_REQUIRED, message, cause);
    }

    pub fn request_timeout(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::REQUEST_TIMEOUT, message, cause);
    }

    pub fn conflict(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::CONFLICT, message, cause);
    }

    pub fn gone(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::GONE, message, cause);
    }

    pub fn length_required(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::LENGTH_REQUIRED, message, cause);
    }

    pub fn precondition_failed(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::PRECONDITION_FAILED, message, cause);
    }

    pub fn payload_too_large(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::PAYLOAD_TOO_LARGE, message, cause);
    }

    pub fn uri_too_long(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::URI_TOO_LONG, message, cause);
    }

    pub fn unsupported_media_type(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::UNSUPPORTED_MEDIA_TYPE, message, cause);
    }

    pub fn range_not_satisfiable(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::RANGE_NOT_SATISFIABLE, message, cause);
    }

    pub fn expectation_failed(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::EXPECTATION_FAILED, message, cause);
    }

    pub fn im_a_teapot(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::IM_A_TEAPOT, message, cause);
    }

    pub fn misdirected_request(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::MISDIRECTED_REQUEST, message, cause);
    }

    pub fn unprocessable_entity(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::UNPROCESSABLE_ENTITY, message, cause);
    }

    pub fn locked(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::LOCKED, message, cause);
    }

    pub fn failed_dependency(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::FAILED_DEPENDENCY, message, cause);
    }

    pub fn upgrade_required(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::UPGRADE_REQUIRED, message, cause);
    }

    pub fn precondition_required(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::PRECONDITION_REQUIRED, message, cause);
    }

    pub fn too_many_requests(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::TOO_MANY_REQUESTS, message, cause);
    }

    pub fn request_header_fields_too_large(
        message: impl Into<String>,
        cause: Option<Box<dyn std::error::Error>>,
    ) -> Self {
        return Self::new(StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE, message, cause);
    }

    pub fn unavailable_for_legal_reasons(
        message: impl Into<String>,
        cause: Option<Box<dyn std::error::Error>>,
    ) -> Self {
        return Self::new(StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS, message, cause);
    }

    // ─── 5xx Server Errors ───────────────────────────────────────────────────

    pub fn internal_server_error(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::INTERNAL_SERVER_ERROR, message, cause);
    }

    pub fn not_implemented(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::NOT_IMPLEMENTED, message, cause);
    }

    pub fn bad_gateway(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::BAD_GATEWAY, message, cause);
    }

    pub fn service_unavailable(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::SERVICE_UNAVAILABLE, message, cause);
    }

    pub fn gateway_timeout(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::GATEWAY_TIMEOUT, message, cause);
    }

    pub fn http_version_not_supported(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::HTTP_VERSION_NOT_SUPPORTED, message, cause);
    }

    pub fn variant_also_negotiates(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::VARIANT_ALSO_NEGOTIATES, message, cause);
    }

    pub fn insufficient_storage(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::INSUFFICIENT_STORAGE, message, cause);
    }

    pub fn loop_detected(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::LOOP_DETECTED, message, cause);
    }

    pub fn not_extended(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
        return Self::new(StatusCode::NOT_EXTENDED, message, cause);
    }

    pub fn network_authentication_required(
        message: impl Into<String>,
        cause: Option<Box<dyn std::error::Error>>,
    ) -> Self {
        return Self::new(StatusCode::NETWORK_AUTHENTICATION_REQUIRED, message, cause);
    }

    // // ─── 1xx Informational ───────────────────────────────────────────────────

    // pub fn continue_request(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::CONTINUE, message, cause);
    // }

    // pub fn switching_protocols(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::SWITCHING_PROTOCOLS, message, cause);
    // }

    // pub fn processing(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::PROCESSING, message, cause);
    // }

    // // ─── 2xx Success ─────────────────────────────────────────────────────────

    // pub fn ok(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
    //     return Self::new(StatusCode::OK, message, cause);
    // }

    // pub fn created(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
    //     return Self::new(StatusCode::CREATED, message, cause);
    // }

    // pub fn accepted(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
    //     return Self::new(StatusCode::ACCEPTED, message, cause);
    // }

    // pub fn non_authoritative_information(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::NON_AUTHORITATIVE_INFORMATION, message, cause);
    // }

    // pub fn no_content(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::NO_CONTENT, message, cause);
    // }

    // pub fn reset_content(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::RESET_CONTENT, message, cause);
    // }

    // pub fn partial_content(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::PARTIAL_CONTENT, message, cause);
    // }

    // pub fn multi_status(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::MULTI_STATUS, message, cause);
    // }

    // pub fn already_reported(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::ALREADY_REPORTED, message, cause);
    // }

    // pub fn im_used(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
    //     return Self::new(StatusCode::IM_USED, message, cause);
    // }

    // // ─── 3xx Redirection ─────────────────────────────────────────────────────

    // pub fn multiple_choices(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::MULTIPLE_CHOICES, message, cause);
    // }

    // pub fn moved_permanently(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::MOVED_PERMANENTLY, message, cause);
    // }

    // pub fn found(message: impl Into<String>, cause: Option<Box<dyn std::error::Error>>) -> Self {
    //     return Self::new(StatusCode::FOUND, message, cause);
    // }

    // pub fn see_other(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::SEE_OTHER, message, cause);
    // }

    // pub fn not_modified(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::NOT_MODIFIED, message, cause);
    // }

    // pub fn use_proxy(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::USE_PROXY, message, cause);
    // }

    // pub fn temporary_redirect(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::TEMPORARY_REDIRECT, message, cause);
    // }

    // pub fn permanent_redirect(
    //     message: impl Into<String>,
    //     cause: Option<Box<dyn std::error::Error>>,
    // ) -> Self {
    //     return Self::new(StatusCode::PERMANENT_REDIRECT, message, cause);
    // }
}

fn get_stack() -> String {
    if cfg!(debug_assertions) {
        return format!("{:?}", Backtrace::new());
    } else {
        return String::new();
    }
}

fn create_cause(cause: Option<Box<dyn std::error::Error>>) -> Option<Value> {
    return cause.map(|e| {
        serde_json::json!({
            "name":    std::any::type_name_of_val(&*e),
            "message": e.to_string(),
            "source":  e.source().map(|s| s.to_string()),
        })
    });
}

fn serialize_status<S>(status: &StatusCode, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    return status.as_u16().serialize(serializer);
}
