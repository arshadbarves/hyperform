//! Fake MCP and LSP servers for integration tests: the integrations
//! crate's `zengine-fake-mcp` / `zengine-fake-lsp` binaries (built into
//! this test run's target directory on first use) and a small Python MCP
//! server with a configurable, growable tool list. Every helper returns
//! `None` when its server cannot be provided, so tests skip.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// `target/<profile>` of the running test binary.
fn profile_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.parent()?.to_path_buf())
}

fn fake_bins() -> Option<&'static PathBuf> {
    static BINS: OnceLock<Option<PathBuf>> = OnceLock::new();
    BINS.get_or_init(|| {
        let dir = profile_dir()?;
        let present = |name: &str| dir.join(name).is_file();
        if present("zengine-fake-mcp") && present("zengine-fake-lsp") {
            return Some(dir);
        }
        let profile = match dir.file_name()?.to_str()? {
            "debug" => "dev",
            other => other,
        }
        .to_string();
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let built = Command::new(cargo)
            .args([
                "build",
                "-p",
                "z-engine-integrations",
                "--bins",
                "--profile",
            ])
            .arg(&profile)
            .env("CARGO_TARGET_DIR", dir.parent()?)
            .current_dir(workspace)
            .status()
            .ok()?;
        (built.success() && present("zengine-fake-mcp") && present("zengine-fake-lsp"))
            .then_some(dir)
    })
    .as_ref()
}

/// The fake MCP stdio server (tools `echo`, `fail`, `notify`, `stats`, ...).
pub fn fake_mcp_bin() -> Option<PathBuf> {
    fake_bins().map(|dir| dir.join("zengine-fake-mcp"))
}

/// The fake language server (errors on lines containing `ERROR`).
pub fn fake_lsp_bin() -> Option<PathBuf> {
    fake_bins().map(|dir| dir.join("zengine-fake-lsp"))
}

pub fn skip(what: &str) {
    eprintln!("skipped: {what} is not available");
}

const PY_MCP: &str = r#"
import json, os, sys

count = int(sys.argv[1])
grown = sys.argv[2]

def send(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()

def tool(name, description, read_only=True):
    return {"name": name, "description": description,
            "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}},
            "annotations": {"readOnlyHint": read_only}}

def tools():
    listed = [tool("t%d" % i, "Tool %d.\nMore detail." % i) for i in range(count)]
    listed.append(tool("grow", "Adds a tool.", False))
    if os.path.exists(grown):
        listed.append(tool("extra", "Appears after grow."))
    return listed

for line in sys.stdin:
    try:
        message = json.loads(line)
    except ValueError:
        continue
    if "id" not in message or "method" not in message:
        continue
    method, rid, params = message["method"], message["id"], message.get("params") or {}
    if method == "initialize":
        result = {"protocolVersion": params.get("protocolVersion", "2025-06-18"),
                  "capabilities": {"tools": {"listChanged": True}},
                  "serverInfo": {"name": "py-fake", "version": "1"}}
    elif method == "tools/list":
        result = {"tools": tools()}
    elif method == "tools/call":
        name = params.get("name")
        if name == "grow":
            open(grown, "w").close()
            send({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"})
        text = "%s: %s" % (name, (params.get("arguments") or {}).get("text", ""))
        result = {"content": [{"type": "text", "text": text}], "isError": False}
    elif method == "ping":
        result = {}
    else:
        send({"jsonrpc": "2.0", "id": rid, "error": {"code": -32601, "message": "no " + method}})
        continue
    send({"jsonrpc": "2.0", "id": rid, "result": result})
"#;

/// `[mcp.servers.<name>]` running the Python fake with `count` tools
/// `t0..` plus `grow` (which adds `extra`); `None` without `python3`.
pub fn py_mcp_toml(dir: &Path, name: &str, count: usize) -> Option<String> {
    let python = Command::new("python3").arg("--version").output().ok()?;
    if !python.status.success() {
        return None;
    }
    let script = dir.join("fake_mcp.py");
    std::fs::write(&script, PY_MCP).ok()?;
    let grown = dir.join("grown.flag");
    Some(format!(
        "\n[mcp.servers.{name}]\ncommand = \"python3\"\nargs = [{:?}, \"{count}\", {:?}]\ntimeout_secs = 10\n",
        script.display().to_string(),
        grown.display().to_string()
    ))
}

/// `[mcp.servers.<name>]` running the Rust fake.
pub fn fake_mcp_toml(name: &str, extra: &str) -> Option<String> {
    let bin = fake_mcp_bin()?;
    Some(format!(
        "\n[mcp.servers.{name}]\ncommand = {:?}\ntimeout_secs = 10\n{extra}\n",
        bin.display().to_string()
    ))
}
