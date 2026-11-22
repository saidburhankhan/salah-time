//! JSON-RPC 2.0 envelopes, NDJSON framing values, and notification types.

use crate::error::RpcError;
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::day::BoundaryId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const JSONRPC_VERSION: &str = "2.0";
pub const PROTOCOL_VERSION: u32 = 1;
/// One NDJSON frame may not exceed 1 MiB.
pub const MAX_LINE_BYTES: usize = 1024 * 1024;
pub const SERVER_NAME: &str = concat!("salah ", env!("CARGO_PKG_VERSION"));

/// A parsed JSON-RPC request. Notifications from clients are not accepted:
/// every request must carry an id so the caller can correlate the response.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub id: Value,
    pub method: String,
    pub params: Value,
}

impl Request {
    pub fn new(id: Value, method: impl Into<String>, params: Value) -> Self {
        Self {
            id,
            method: method.into(),
            params,
        }
    }

    /// Validates the envelope. An invalid request is a protocol fault, not an
    /// application error, so it has no `data.kind`.
    pub fn parse(value: Value) -> Result<Self, RpcError> {
        let object = value
            .as_object()
            .ok_or_else(|| RpcError::invalid_request("request must be a JSON object"))?;
        let id = object
            .get("id")
            .cloned()
            .ok_or_else(|| RpcError::invalid_request("request is missing id"))?;
        if !(id.is_string() || id.is_number() || id.is_null()) {
            return Err(RpcError::invalid_request(
                "id must be a string, number, or null",
            ));
        }
        match object.get("jsonrpc").and_then(Value::as_str) {
            Some(JSONRPC_VERSION) => {}
            Some(_) => {
                return Err(RpcError::invalid_request("jsonrpc must be \"2.0\""));
            }
            None => return Err(RpcError::invalid_request("request is missing jsonrpc")),
        }
        let method = object
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError::invalid_request("method must be a string"))?;
        Ok(Self {
            id,
            method: method.to_string(),
            params: object.get("params").cloned().unwrap_or(Value::Null),
        })
    }
}

/// One JSON-RPC response line. Exactly one of `result` and `error` is set.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Response {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Value, error: RpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            result: None,
            error: Some(error),
        }
    }

    pub fn into_line(self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self)
    }
}

/// One subscription scope carried by a `changed` notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeScope {
    Items,
    Sessions,
    Projects,
    Tags,
    Metrics,
    Marks,
    Config,
}

impl ChangeScope {
    /// The scope of a store change-log entity name.
    pub fn of_entity(entity: &str) -> Option<Self> {
        Some(match entity {
            "item" => Self::Items,
            "session" => Self::Sessions,
            "project" => Self::Projects,
            "tag" => Self::Tags,
            "metric" => Self::Metrics,
            "prayer_mark" => Self::Marks,
            _ => return None,
        })
    }
}

/// Invalidation notification. It deliberately carries no entity patches: a
/// client refetches `status.get` or `day.get` after receiving it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedNotification {
    pub revision: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<NaiveDate>,
    pub scopes: Vec<ChangeScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_id: Option<String>,
    pub actor: String,
}

pub const CHANGED_METHOD: &str = "changed";
pub const CLOCK_BOUNDARY_METHOD: &str = "clock.boundary";

/// The next boundary and the instant it occurs at, so clients never need to
/// reconstruct boundary maths locally.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockBoundaryNext {
    pub id: BoundaryId,
    pub name: String,
    pub time: String,
    pub at: DateTime<Utc>,
    pub remaining_seconds: i64,
}

/// Pushed when the current block or next boundary changes. `now` is the
/// authoritative daemon instant the rest of the payload was derived from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockBoundaryNotification {
    pub day: NaiveDate,
    pub block: Option<BoundaryId>,
    pub now: DateTime<Utc>,
    pub next: ClockBoundaryNext,
}

impl ClockBoundaryNotification {
    pub fn envelope(self) -> Notification<Self> {
        Notification {
            jsonrpc: JSONRPC_VERSION,
            method: CLOCK_BOUNDARY_METHOD,
            params: self,
        }
    }
}

/// A JSON-RPC notification envelope (no `id`). S4 emits these; the types are
/// defined here so both the daemon and clients share the shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Notification<T> {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: T,
}

impl ChangedNotification {
    pub fn envelope(self) -> Notification<Self> {
        Notification {
            jsonrpc: JSONRPC_VERSION,
            method: CHANGED_METHOD,
            params: self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[test]
    fn request_parsing_requires_the_jsonrpc_envelope() {
        assert!(Request::parse(json!({"jsonrpc":"2.0","id":1,"method":"hello"})).is_ok());
        assert!(Request::parse(json!({"id":1,"method":"hello"})).is_err());
        assert!(Request::parse(json!({"jsonrpc":"1.0","id":1,"method":"hello"})).is_err());
        assert!(Request::parse(json!({"jsonrpc":"2.0","method":"hello"})).is_err());
        assert!(Request::parse(json!(["not", "an", "object"])).is_err());
    }

    #[test]
    fn clock_boundary_notification_has_no_id_and_carries_an_instant() {
        let notification = ClockBoundaryNotification {
            day: NaiveDate::from_ymd_opt(2031, 5, 14).unwrap(),
            block: Some(BoundaryId::Fajr),
            now: Utc.with_ymd_and_hms(2031, 5, 14, 4, 30, 0).unwrap(),
            next: ClockBoundaryNext {
                id: BoundaryId::Dhuhr,
                name: "Dhuhr".into(),
                time: "12:19".into(),
                at: Utc.with_ymd_and_hms(2031, 5, 14, 7, 19, 0).unwrap(),
                remaining_seconds: 10140,
            },
        }
        .envelope();
        let value = serde_json::to_value(notification).unwrap();
        assert_eq!(value["method"], "clock.boundary");
        assert!(value.get("id").is_none());
        assert_eq!(value["params"]["next"]["id"], "dhuhr");
        assert!(value["params"]["next"]["at"].is_string());
    }

    #[test]
    fn changed_notification_has_no_id_and_carries_a_revision() {
        let notification = ChangedNotification {
            revision: 7,
            days: vec![NaiveDate::from_ymd_opt(2031, 5, 14).unwrap()],
            scopes: vec![ChangeScope::Items, ChangeScope::Sessions],
            op_id: Some("op-1".into()),
            actor: "widget".into(),
        }
        .envelope();
        let value = serde_json::to_value(notification).unwrap();
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["method"], "changed");
        assert!(value.get("id").is_none());
        assert_eq!(value["params"]["revision"], 7);
        assert_eq!(value["params"]["scopes"][0], "items");
    }
}
