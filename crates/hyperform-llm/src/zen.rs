//! OpenCode Zen request identity and free-tier fingerprint.
//!
//! Zen's gateway expects the session headers the official OpenCode client
//! sends (`x-opencode-session`, `x-opencode-request`, `x-opencode-client`).
//! Since Sep 2026 the free tier also requires an OpenCode user-agent
//! (`opencode/1.18+`), a session id shaped like
//! `ses_` + 12 hex + 14 alnum, streaming, and tools named `shell`/`bash`
//! and `read`. Without that fingerprint free models answer 403
//! `FreeTierError`.

use serde_json::{Value, json};

use crate::opencode::USER_AGENT;

/// Client label OpenCode's gateway accepts for free-tier requests.
const CLIENT: &str = "cli";
const HEX_LEN: usize = 12;
const TAIL_LEN: usize = 14;
const SESSION_BODY: usize = HEX_LEN + TAIL_LEN;
const ALNUM: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Dummy tools the free-tier gatekeeper looks for. Real agent tools
/// (`Bash`, `Read`, …) stay on the request; these are only for the gate.
const GATE_SHELL: &str = "shell";
const GATE_READ: &str = "read";

pub(crate) fn is_zen_url(base_url: &str) -> bool {
    base_url.to_ascii_lowercase().contains("opencode.ai")
}

pub(crate) fn new_session_id() -> String {
    let bytes = ulid::Ulid::new().to_bytes();
    format!("ses_{}{}", hex_from(&bytes[..6]), alnum_from(&bytes[6..]))
}

/// A Zen-shaped session id derived from a stable conversation key, so one
/// conversation keeps one Zen session. `None` when the key has no usable
/// characters.
pub(crate) fn session_id_for(key: &str) -> Option<String> {
    if is_valid_session(key) {
        return Some(key.to_string());
    }
    let alnum: String = key.chars().filter(char::is_ascii_alphanumeric).collect();
    if alnum.is_empty() {
        return None;
    }
    let mut material = alnum.as_bytes().to_vec();
    while material.len() < SESSION_BODY {
        material.extend_from_slice(alnum.as_bytes());
    }
    let hex: String = material[..HEX_LEN]
        .iter()
        .map(|b| format!("{:x}", b % 16))
        .collect();
    let tail = alnum_from(&material[HEX_LEN..HEX_LEN + TAIL_LEN]);
    Some(format!("ses_{hex}{tail}"))
}

pub(crate) fn user_agent() -> String {
    USER_AGENT.to_string()
}

pub(crate) fn apply_headers(
    request: reqwest::RequestBuilder,
    session_id: &str,
) -> reqwest::RequestBuilder {
    request
        .header("x-opencode-session", session_id)
        .header("x-opencode-request", request_id())
        .header("x-opencode-client", CLIENT)
        .header(reqwest::header::USER_AGENT, user_agent())
}

/// Ensure a chat-completions body satisfies Zen's free-tier gate: stream
/// on, and tools that declare `shell` (or `bash`) and `read`.
pub(crate) fn apply_free_tier_body(body: &mut Value) {
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".into(), json!(true));
    }
    let tools = match body.get_mut("tools") {
        Some(Value::Array(tools)) => tools,
        _ => {
            body["tools"] = json!([]);
            body["tools"].as_array_mut().expect("just set")
        }
    };
    if !has_named(tools, &["shell", "bash"]) {
        tools.push(gate_tool(
            GATE_SHELL,
            "Execute a shell command and return its output.",
            "command",
            "The shell command to run",
        ));
    }
    if !has_named(tools, &["read"]) {
        tools.push(gate_tool(
            GATE_READ,
            "Read a file and return its contents.",
            "path",
            "Path of the file to read",
        ));
    }
}

fn request_id() -> String {
    let bytes = ulid::Ulid::new().to_bytes();
    format!("req_{}{}", hex_from(&bytes[..6]), alnum_from(&bytes[6..]))
}

fn is_valid_session(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("ses_") else {
        return false;
    };
    let bytes = rest.as_bytes();
    if bytes.len() != SESSION_BODY {
        return false;
    }
    bytes[..HEX_LEN]
        .iter()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && bytes[HEX_LEN..].iter().all(u8::is_ascii_alphanumeric)
}

fn has_named(tools: &[Value], names: &[&str]) -> bool {
    // The gateway matches exact lowercase names; Claude Code's `Bash` /
    // `Read` do not satisfy the free-tier check.
    tools.iter().any(|tool| {
        let name = tool
            .pointer("/function/name")
            .and_then(Value::as_str)
            .unwrap_or("");
        names.contains(&name)
    })
}

fn gate_tool(name: &str, description: &str, param: &str, param_desc: &str) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": {
                "type": "object",
                "properties": { param: { "type": "string", "description": param_desc } },
                "required": [param],
            },
        },
    })
}

fn hex_from(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn alnum_from(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(TAIL_LEN);
    for &b in bytes.iter().cycle() {
        if out.len() >= TAIL_LEN {
            break;
        }
        out.push(ALNUM[(b as usize) % ALNUM.len()] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_zen_gateway() {
        assert!(is_zen_url("https://opencode.ai/zen/v1"));
        assert!(is_zen_url("https://OPENCODE.AI/zen/v1/"));
        assert!(!is_zen_url("https://openrouter.ai/api/v1"));
    }

    #[test]
    fn ids_match_free_tier_shape() {
        let session = new_session_id();
        let request = request_id();
        assert!(is_valid_session(&session), "{session}");
        assert!(request.starts_with("req_"));
        assert_eq!(session.len(), 4 + SESSION_BODY);
        assert_eq!(request.len(), 4 + SESSION_BODY);
        assert_ne!(session[4..], request[4..]);
    }

    #[test]
    fn conversation_keys_map_to_stable_session_ids() {
        let key = "01K5ZQ8X1B2C3D4E5F6G7H8J9K";
        let first = session_id_for(key).unwrap();
        assert_eq!(session_id_for(key).as_deref(), Some(first.as_str()));
        assert!(is_valid_session(&first), "{first}");
        let already = new_session_id();
        assert_eq!(session_id_for(&already).as_deref(), Some(already.as_str()));
        assert_eq!(session_id_for("--"), None);
    }

    #[test]
    fn user_agent_is_opencode() {
        assert_eq!(user_agent(), USER_AGENT);
        assert!(user_agent().starts_with("opencode/"));
    }

    #[test]
    fn free_tier_body_injects_stream_and_gate_tools() {
        let mut body = json!({
            "model": "big-pickle",
            "messages": [],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "Bash",
                    "parameters": {"type": "object", "properties": {}}
                }
            }]
        });
        apply_free_tier_body(&mut body);
        assert_eq!(body["stream"], true);
        let names: Vec<&str> = body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t.pointer("/function/name")?.as_str())
            .collect();
        assert!(names.contains(&"Bash"));
        assert!(names.contains(&"shell"));
        assert!(names.contains(&"read"));
    }

    #[test]
    fn free_tier_body_keeps_existing_bash_and_read() {
        let mut body = json!({
            "tools": [
                {"type": "function", "function": {"name": "bash", "parameters": {}}},
                {"type": "function", "function": {"name": "read", "parameters": {}}},
            ]
        });
        apply_free_tier_body(&mut body);
        assert_eq!(body["tools"].as_array().unwrap().len(), 2);
    }
}
