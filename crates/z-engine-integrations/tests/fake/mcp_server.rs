//! Test-only MCP stdio server (newline-delimited JSON-RPC).
//!
//! Tools (two `tools/list` pages): `echo`, `fail` (isError), `slow` (replies
//! after `ms`, default 5000, on its own thread), `crash` (exits without a
//! reply), `notify` (announces a tools change first), `malformed` (breaks the
//! result schema), `noisy` (prints a non-JSON line first), `media` (every
//! content kind), `stats` (methods and cancellations seen so far).
//! Flags: `--tools-only` declares only tools; `--protocol=V` answers with V.

use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

type Out = Arc<Mutex<std::io::Stdout>>;

#[derive(Default)]
struct Stats {
    methods: Vec<String>,
    cancelled: Vec<Value>,
}

fn send(out: &Out, message: &Value) {
    let mut out = out.lock().unwrap();
    writeln!(out, "{message}").unwrap();
    out.flush().unwrap();
}

fn reply(out: &Out, id: Value, result: Value) {
    send(out, &json!({"jsonrpc": "2.0", "id": id, "result": result}));
}

fn text(text: &str, is_error: bool) -> Value {
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

fn tool(name: &str, description: &str) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}, "ms": {"type": "integer"}}},
        "annotations": {"readOnlyHint": name == "echo"}
    })
}

fn tools_page(cursor: Option<&str>) -> Value {
    match cursor {
        None => json!({
            "tools": [tool("echo", "Echoes text"), tool("fail", "Always fails"), tool("slow", "Sleeps")],
            "nextCursor": "page-2"
        }),
        Some(_) => json!({"tools": [
            tool("crash", "Exits"), tool("notify", "Announces a change"), tool("malformed", "Bad result"),
            tool("noisy", "Prints garbage"), tool("media", "Every content kind"), tool("stats", "What was seen")
        ]}),
    }
}

fn initialize(message: &Value, tools_only: bool, forced: Option<&str>) -> Value {
    let requested = message["params"]["protocolVersion"]
        .as_str()
        .unwrap_or("2025-06-18");
    let version = forced.unwrap_or(requested);
    let mut capabilities = json!({"tools": {"listChanged": true}, "logging": {}});
    if !tools_only {
        capabilities["resources"] = json!({});
        capabilities["prompts"] = json!({});
    }
    json!({
        "protocolVersion": version,
        "capabilities": capabilities,
        "serverInfo": {"name": "zengine-fake-mcp", "version": "0.1.0"},
        "instructions": "Fake server for tests."
    })
}

fn call_tool(out: &Out, stats: &Arc<Mutex<Stats>>, id: Value, params: &Value) {
    let args = &params["arguments"];
    match params["name"].as_str().unwrap_or_default() {
        "echo" => reply(
            out,
            id,
            text(
                &format!("echo: {}", args["text"].as_str().unwrap_or("")),
                false,
            ),
        ),
        "fail" => reply(out, id, text("the tool failed on purpose", true)),
        "slow" => {
            let ms = args["ms"].as_u64().unwrap_or(5_000);
            let out = Arc::clone(out);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(ms));
                reply(&out, id, text("slow done", false));
            });
        }
        "crash" => {
            eprintln!("fake-mcp: crashing on purpose");
            std::process::exit(3);
        }
        "notify" => {
            send(
                out,
                &json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"}),
            );
            reply(out, id, text("notified", false));
        }
        "malformed" => reply(out, id, json!({"content": "not an array"})),
        "noisy" => {
            {
                let mut raw = out.lock().unwrap();
                writeln!(raw, "this line is not JSON").unwrap();
                raw.flush().unwrap();
            }
            reply(out, id, text("after the noise", false));
        }
        "media" => reply(
            out,
            id,
            json!({
                "content": [
                    {"type": "text", "text": "caption"},
                    {"type": "image", "data": "aGVsbG8=", "mimeType": "image/png"},
                    {"type": "resource_link", "uri": "fake://readme", "name": "readme"},
                    {"type": "resource", "resource": {"uri": "fake://notes", "mimeType": "text/plain", "text": "notes"}}
                ],
                "structuredContent": {"ok": true}
            }),
        ),
        "stats" => {
            let stats = stats.lock().unwrap();
            let seen = json!({"methods": stats.methods, "cancelled": stats.cancelled});
            reply(out, id, json!({"content": [], "structuredContent": seen}));
        }
        other => send(
            out,
            &json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32602, "message": format!("unknown tool {other}")}}),
        ),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let tools_only = args.iter().any(|a| a == "--tools-only");
    let forced = args
        .iter()
        .find_map(|a| a.strip_prefix("--protocol="))
        .map(str::to_string);
    let out: Out = Arc::new(Mutex::new(std::io::stdout()));
    let stats = Arc::new(Mutex::new(Stats::default()));
    eprintln!("fake-mcp: started");
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(method) = message["method"].as_str().map(str::to_string) else {
            continue; // a reply to one of our requests
        };
        stats.lock().unwrap().methods.push(method.clone());
        let Some(id) = message.get("id").cloned() else {
            if method == "notifications/cancelled" {
                stats
                    .lock()
                    .unwrap()
                    .cancelled
                    .push(message["params"]["requestId"].clone());
            }
            continue;
        };
        let params = &message["params"];
        match method.as_str() {
            "initialize" => reply(
                &out,
                id,
                initialize(&message, tools_only, forced.as_deref()),
            ),
            "ping" => reply(&out, id, json!({})),
            "tools/list" => reply(&out, id, tools_page(params["cursor"].as_str())),
            "tools/call" => call_tool(&out, &stats, id, params),
            "resources/list" => reply(
                &out,
                id,
                json!({"resources": [{"uri": "fake://readme", "name": "readme", "description": "The readme", "mimeType": "text/markdown"}]}),
            ),
            "resources/read" => reply(
                &out,
                id,
                json!({"contents": [{"uri": params["uri"], "mimeType": "text/markdown", "text": "# Fake readme"}]}),
            ),
            "prompts/list" => reply(
                &out,
                id,
                json!({"prompts": [{"name": "review", "description": "Review a file",
                                    "arguments": [{"name": "file", "description": "Path", "required": true}]}]}),
            ),
            "prompts/get" => {
                let file = params["arguments"]["file"].as_str().unwrap_or("?");
                let message = json!({"role": "user", "content": {"type": "text", "text": format!("Please review {file}")}});
                reply(&out, id, json!({"messages": [message]}));
            }
            _ => send(
                &out,
                &json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "method not found"}}),
            ),
        }
    }
}
