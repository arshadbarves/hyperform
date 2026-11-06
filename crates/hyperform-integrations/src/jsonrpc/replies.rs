//! Answers to requests a server sends to the client. The client declares no
//! optional capabilities, so only liveness and the LSP bookkeeping requests
//! servers send regardless get a result; everything else is refused with
//! `MethodNotFound` so the server never waits on us.

use serde_json::{Value, json};

use super::message::{METHOD_NOT_FOUND, RpcErrorObject};

/// The reply to a server->client request.
pub fn default_reply(method: &str, params: Option<&Value>) -> Result<Value, RpcErrorObject> {
    match method {
        "ping" => Ok(json!({})),
        // One entry per requested item; `null` means "use your defaults".
        "workspace/configuration" => {
            let items = params
                .and_then(|p| p.get("items"))
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            Ok(Value::Array(vec![Value::Null; items]))
        }
        "client/registerCapability"
        | "client/unregisterCapability"
        | "window/workDoneProgress/create"
        | "window/showMessageRequest" => Ok(Value::Null),
        _ => Err(RpcErrorObject::new(
            METHOD_NOT_FOUND,
            format!("method not found: {method}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_liveness_and_lsp_bookkeeping() {
        assert_eq!(default_reply("ping", None), Ok(json!({})));
        let params = json!({"items": [{"section": "rust-analyzer"}, {"section": "files"}]});
        assert_eq!(
            default_reply("workspace/configuration", Some(&params)),
            Ok(json!([null, null]))
        );
        assert_eq!(
            default_reply("workspace/configuration", None),
            Ok(json!([]))
        );
        for method in [
            "client/registerCapability",
            "window/workDoneProgress/create",
        ] {
            assert_eq!(default_reply(method, Some(&json!({}))), Ok(Value::Null));
        }
    }

    #[test]
    fn refuses_everything_else() {
        let error = default_reply("sampling/createMessage", None).unwrap_err();
        assert_eq!(error.code, METHOD_NOT_FOUND);
        assert!(error.message.contains("sampling/createMessage"));
    }
}
