//! JSON-RPC error objects and the stable mapping from domain error classes.

use salah_domain::error::Error;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC reserved codes for protocol faults.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

/// First application error code. Each stable `data.kind` owns one number.
pub const APPLICATION_ERROR_BASE: i64 = -32000;

/// Structured error payload. `kind` is the compatibility contract; the other
/// fields are additive metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorData {
    pub kind: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<Value>,
}

impl ErrorData {
    fn new(kind: impl Into<String>, retryable: bool) -> Self {
        Self {
            kind: kind.into(),
            retryable,
            field: None,
            current: None,
        }
    }
}

/// The JSON-RPC `error` member.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ErrorData>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>, data: Option<ErrorData>) -> Self {
        Self {
            code,
            message: message.into(),
            data,
        }
    }

    /// A framing or request-envelope fault with no application `kind`.
    pub fn protocol(code: i64, message: impl Into<String>) -> Self {
        Self::new(code, message, None)
    }

    pub fn parse(message: impl Into<String>) -> Self {
        Self::protocol(PARSE_ERROR, message)
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::protocol(INVALID_REQUEST, message)
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::protocol(METHOD_NOT_FOUND, format!("method not found: {method}"))
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        let mut data = ErrorData::new("validation", false);
        data.field = Some("params".into());
        Self::new(INVALID_PARAMS, message, Some(data))
    }

    pub fn internal(message: impl Into<String>) -> Self {
        let mut data = ErrorData::new("internal", false);
        data.field = None;
        Self::new(INTERNAL_ERROR, message, Some(data))
    }
}

/// The wire code for a stable `Error::code()` class.
pub fn code_for_kind(kind: &str) -> i64 {
    match kind {
        "validation" => APPLICATION_ERROR_BASE - 1,
        "not_found" => APPLICATION_ERROR_BASE - 2,
        "ambiguous" => APPLICATION_ERROR_BASE - 3,
        "conflict" => APPLICATION_ERROR_BASE - 4,
        "busy" => APPLICATION_ERROR_BASE - 5,
        "storage" => APPLICATION_ERROR_BASE - 6,
        "source_unavailable" => APPLICATION_ERROR_BASE - 7,
        "config" => APPLICATION_ERROR_BASE - 8,
        "internal" => APPLICATION_ERROR_BASE - 9,
        _ => APPLICATION_ERROR_BASE - 9,
    }
}

fn retryable_kind(kind: &str) -> bool {
    matches!(kind, "busy" | "source_unavailable")
}

impl From<Error> for RpcError {
    fn from(error: Error) -> Self {
        let kind = error.code();
        let mut data = ErrorData::new(kind, retryable_kind(kind));
        if let Error::Validation { field, .. } = &error {
            data.field = Some((*field).to_string());
        }
        Self::new(code_for_kind(kind), error.message(), Some(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_domain_kind_has_a_fixed_application_code() {
        let kinds = [
            "validation",
            "not_found",
            "ambiguous",
            "conflict",
            "busy",
            "storage",
            "source_unavailable",
            "config",
            "internal",
        ];
        let mut codes = std::collections::HashSet::new();
        for kind in kinds {
            let code = code_for_kind(kind);
            assert!(
                (APPLICATION_ERROR_BASE - 100..APPLICATION_ERROR_BASE).contains(&code),
                "{kind} has a non-application code"
            );
            assert!(codes.insert(code), "duplicate code for {kind}");
        }
    }

    #[test]
    fn validation_errors_keep_their_field_and_busy_is_retryable() {
        let error: RpcError = Error::validation("sleep_time", "bad time").into();
        let data = error.data.unwrap();
        assert_eq!(data.kind, "validation");
        assert_eq!(data.field.as_deref(), Some("sleep_time"));
        assert!(!data.retryable);

        let error: RpcError = Error::Busy("try again".into()).into();
        assert!(error.data.unwrap().retryable);
    }
}
