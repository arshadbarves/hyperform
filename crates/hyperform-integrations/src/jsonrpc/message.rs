//! JSON-RPC 2.0 messages. Parsing never fails outright: anything that is not
//! a well-formed request, notification or response becomes
//! [`RpcMessage::Invalid`], keeping the id when there is one so the waiting
//! request fails with a protocol error instead of hanging until its timeout.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "{s:?}"),
        }
    }
}

impl From<i64> for RequestId {
    fn from(n: i64) -> Self {
        Self::Number(n)
    }
}

impl From<&str> for RequestId {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcErrorObject {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcErrorObject {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcRequest {
    pub id: RequestId,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcNotification {
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcResponse {
    /// `None` only for error responses to unparseable requests.
    pub id: Option<RequestId>,
    pub result: Result<Value, RpcErrorObject>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RpcMessage {
    Request(RpcRequest),
    Notification(RpcNotification),
    Response(RpcResponse),
    /// Not a valid JSON-RPC message; `id` is kept when it could be read.
    Invalid {
        id: Option<RequestId>,
        reason: String,
    },
}

impl RpcMessage {
    /// Classifies one decoded JSON value.
    pub fn from_value(value: Value) -> Self {
        let Value::Object(mut object) = value else {
            return Self::invalid(None, "a JSON-RPC message must be an object");
        };
        let id = match object.remove("id") {
            None | Some(Value::Null) => None,
            Some(raw) => match serde_json::from_value::<RequestId>(raw) {
                Ok(id) => Some(id),
                Err(_) => return Self::invalid(None, "the id is neither a number nor a string"),
            },
        };
        if let Some(method) = object.remove("method") {
            let Value::String(method) = method else {
                return Self::invalid(id, "the method is not a string");
            };
            let params = object.remove("params");
            return match id {
                Some(id) => Self::Request(RpcRequest { id, method, params }),
                None => Self::Notification(RpcNotification { method, params }),
            };
        }
        response(id, object)
    }

    /// Decodes one frame: a single message or a batch array.
    pub fn parse_frame(bytes: &[u8]) -> Vec<Self> {
        match serde_json::from_slice::<Value>(bytes) {
            Ok(Value::Array(items)) if !items.is_empty() => {
                items.into_iter().map(Self::from_value).collect()
            }
            Ok(value) => vec![Self::from_value(value)],
            Err(e) => vec![Self::invalid(None, format!("invalid JSON: {e}"))],
        }
    }

    fn invalid(id: Option<RequestId>, reason: impl Into<String>) -> Self {
        Self::Invalid {
            id,
            reason: reason.into(),
        }
    }
}

fn response(id: Option<RequestId>, mut object: Map<String, Value>) -> RpcMessage {
    let result = object.remove("result");
    match object.remove("error") {
        Some(error) if !error.is_null() => match serde_json::from_value::<RpcErrorObject>(error) {
            Ok(error) => RpcMessage::Response(RpcResponse {
                id,
                result: Err(error),
            }),
            Err(e) => RpcMessage::invalid(id, format!("malformed error object: {e}")),
        },
        _ => match result {
            Some(result) => RpcMessage::Response(RpcResponse {
                id,
                result: Ok(result),
            }),
            None => RpcMessage::invalid(id, "the message has no method, result or error"),
        },
    }
}

fn envelope() -> Map<String, Value> {
    let mut object = Map::new();
    object.insert("jsonrpc".into(), Value::String("2.0".into()));
    object
}

impl RpcRequest {
    pub fn to_value(&self) -> Value {
        let mut object = envelope();
        object.insert("id".into(), id_value(&self.id));
        object.insert("method".into(), Value::String(self.method.clone()));
        if let Some(params) = &self.params {
            object.insert("params".into(), params.clone());
        }
        Value::Object(object)
    }
}

impl RpcNotification {
    pub fn to_value(&self) -> Value {
        let mut object = envelope();
        object.insert("method".into(), Value::String(self.method.clone()));
        if let Some(params) = &self.params {
            object.insert("params".into(), params.clone());
        }
        Value::Object(object)
    }
}

impl RpcResponse {
    pub fn to_value(&self) -> Value {
        let mut object = envelope();
        object.insert("id".into(), self.id.as_ref().map_or(Value::Null, id_value));
        match &self.result {
            Ok(result) => object.insert("result".into(), result.clone()),
            Err(error) => object.insert("error".into(), error_value(error)),
        };
        Value::Object(object)
    }
}

fn id_value(id: &RequestId) -> Value {
    match id {
        RequestId::Number(n) => Value::from(*n),
        RequestId::String(s) => Value::String(s.clone()),
    }
}

fn error_value(error: &RpcErrorObject) -> Value {
    let mut object = Map::new();
    object.insert("code".into(), Value::from(error.code));
    object.insert("message".into(), Value::String(error.message.clone()));
    if let Some(data) = &error.data {
        object.insert("data".into(), data.clone());
    }
    Value::Object(object)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse(value: Value) -> RpcMessage {
        RpcMessage::from_value(value)
    }

    #[test]
    fn classifies_requests_notifications_and_responses() {
        let request = parse(json!({"jsonrpc": "2.0", "id": "a", "method": "ping"}));
        assert_eq!(
            request,
            RpcMessage::Request(RpcRequest {
                id: "a".into(),
                method: "ping".into(),
                params: None
            })
        );
        let note = parse(json!({"jsonrpc": "2.0", "method": "x", "params": [1]}));
        assert!(matches!(note, RpcMessage::Notification(n) if n.params == Some(json!([1]))));
        let ok = parse(json!({"jsonrpc": "2.0", "id": 7, "result": null}));
        assert_eq!(
            ok,
            RpcMessage::Response(RpcResponse {
                id: Some(7.into()),
                result: Ok(Value::Null)
            })
        );
        let err = parse(json!({"id": 8, "error": {"code": -32601, "message": "nope"}}));
        let RpcMessage::Response(RpcResponse { result: Err(e), .. }) = err else {
            panic!("expected an error response");
        };
        assert_eq!((e.code, e.message.as_str()), (METHOD_NOT_FOUND, "nope"));
    }

    #[test]
    fn invalid_messages_keep_their_id() {
        assert_eq!(
            parse(json!({"id": 3})),
            RpcMessage::Invalid {
                id: Some(3.into()),
                reason: "the message has no method, result or error".into()
            }
        );
        let bad_error = parse(json!({"id": 4, "error": "boom"}));
        assert!(matches!(
            bad_error,
            RpcMessage::Invalid {
                id: Some(RequestId::Number(4)),
                ..
            }
        ));
        assert!(matches!(
            parse(json!([1])),
            RpcMessage::Invalid { id: None, .. }
        ));
        assert!(matches!(
            parse(json!({"id": 1.5, "result": 1})),
            RpcMessage::Invalid { id: None, .. }
        ));
    }

    #[test]
    fn frames_decode_batches_and_reject_bad_json() {
        let batch = RpcMessage::parse_frame(br#"[{"method":"a"},{"id":1,"result":2}]"#);
        assert_eq!(batch.len(), 2);
        let bad = RpcMessage::parse_frame(b"not json");
        assert!(
            matches!(&bad[..], [RpcMessage::Invalid { id: None, reason }] if reason.starts_with("invalid JSON"))
        );
    }

    #[test]
    fn serializes_with_the_jsonrpc_envelope() {
        let request = RpcRequest {
            id: 1.into(),
            method: "initialize".into(),
            params: Some(json!({})),
        };
        assert_eq!(
            request.to_value(),
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})
        );
        let note = RpcNotification {
            method: "exit".into(),
            params: None,
        };
        assert_eq!(note.to_value(), json!({"jsonrpc": "2.0", "method": "exit"}));
        let response = RpcResponse {
            id: Some("s".into()),
            result: Err(RpcErrorObject::new(METHOD_NOT_FOUND, "no")),
        };
        assert_eq!(
            response.to_value(),
            json!({"jsonrpc": "2.0", "id": "s", "error": {"code": -32601, "message": "no"}})
        );
        assert_eq!(parse(response.to_value()), RpcMessage::Response(response));
    }
}
