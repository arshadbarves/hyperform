//! Shared integration-test helpers: the fake server binaries, fixture
//! projects, and position helpers.
// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

pub mod mcp_http;

use std::path::PathBuf;
use std::time::Duration;

use z_engine_integrations::{CallToolResult, LspServerSpec, McpContent, McpServerSpec};

pub const FAKE_MCP: &str = env!("CARGO_BIN_EXE_zengine-fake-mcp");
pub const FAKE_LSP: &str = env!("CARGO_BIN_EXE_zengine-fake-lsp");

pub fn fake_mcp(name: &str, args: &[&str]) -> McpServerSpec {
    let args = args.iter().map(|a| (*a).to_string()).collect();
    McpServerSpec::stdio(name, FAKE_MCP, args).with_timeout(Duration::from_secs(10))
}

/// Concatenated text content of a tool result.
pub fn text_of(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|item| match item {
            McpContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Polls `check` for up to two seconds.
pub async fn eventually(mut check: impl FnMut() -> bool) -> bool {
    for _ in 0..200 {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    check()
}

/// The Rust fixture served by the fake language server. Line 12 puts a
/// crab (two UTF-16 units) before the `helper` call; line 21 puts one
/// before the `ERROR` marker.
pub const LIB_RS: &str = r#"struct Greeter {
    name: String,
}

impl Greeter {
    fn new(name: &str) -> Greeter {
        Greeter { name: name.to_string() }
    }
}

fn greet(g: &Greeter) -> String {
    let crab = "🦀"; helper(g)
}

fn helper(g: &Greeter) -> String {
    format!("héllo {}", g.name)
}

fn main() {
    let g = Greeter::new("wörld");
    let _ = greet(&g); // 🦀 ERROR marker
}
"#;

pub fn fake_lsp(args: &[&str]) -> LspServerSpec {
    LspServerSpec::new("fake-lsp", FAKE_LSP, args, &["rs"], &["Cargo.toml"])
}

/// A project with `Cargo.toml` and `src/lib.rs`; returns the canonical root.
pub fn rust_project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
    std::fs::write(root.join("src/lib.rs"), LIB_RS).unwrap();
    (dir, root)
}

/// 1-based line of the first line containing `needle`, and that line.
pub fn line_of(text: &str, needle: &str) -> (u32, String) {
    let (index, line) = text
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(needle))
        .unwrap_or_else(|| panic!("no line contains {needle:?}"));
    (u32::try_from(index).unwrap() + 1, line.to_string())
}

/// 1-based character column of `needle` in `line`.
pub fn col(line: &str, needle: &str) -> u32 {
    let byte = line
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in {line:?}"));
    u32::try_from(line[..byte].chars().count()).unwrap() + 1
}

/// `(line, column)` of `needle` in the first line containing `anchor`.
pub fn at(anchor: &str, needle: &str) -> (u32, u32) {
    let (line, text) = line_of(LIB_RS, anchor);
    (line, col(&text, needle))
}
