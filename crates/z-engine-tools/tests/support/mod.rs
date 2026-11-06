//! Fixtures shared by the integration tests: temporary projects, contexts,
//! and fake ports that record every call.
#![allow(dead_code)]

pub mod fakes;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use z_engine_tools::builtin::ReadTool;
use z_engine_tools::{Ports, SpillFn, Tool, ToolCtx, ToolError, ToolOutput};

/// A temporary project containing `files` (relative path, content).
pub fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, content) in files {
        write(dir.path(), path, content);
    }
    dir
}

pub fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

pub fn read_file(root: &Path, path: &str) -> String {
    std::fs::read_to_string(root.join(path)).unwrap()
}

pub fn ctx(root: &Path) -> ToolCtx {
    ToolCtx::for_tests(root)
}

pub fn ctx_with(root: &Path, ports: Ports) -> ToolCtx {
    let mut ctx = ToolCtx::for_tests(root);
    ctx.ports = Arc::new(ports);
    ctx
}

pub async fn call(tool: &dyn Tool, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
    tool.call(input, ctx).await
}

/// The text of a successful call.
pub async fn ok_text(tool: &dyn Tool, ctx: &ToolCtx, input: Value) -> String {
    match tool.call(input, ctx).await {
        Ok(output) => output.text_content(),
        Err(e) => panic!("{} failed: {e}", tool.name()),
    }
}

/// The message of a failed call.
pub async fn err_text(tool: &dyn Tool, ctx: &ToolCtx, input: Value) -> String {
    match tool.call(input, ctx).await {
        Ok(output) => panic!(
            "{} unexpectedly succeeded: {}",
            tool.name(),
            output.text_content()
        ),
        Err(e) => e.to_string(),
    }
}

/// Reads `path` with the Read tool so edits are allowed.
pub async fn read(ctx: &ToolCtx, path: &str) {
    ReadTool
        .call(json!({"file_path": path}), ctx)
        .await
        .unwrap();
}

/// `(hint, content)` pairs a recording spill hook stored.
pub type Spilled = Arc<Mutex<Vec<(String, String)>>>;

/// A spill hook that records what it stored and returns a fixed path.
pub fn recording_spill() -> (SpillFn, Spilled) {
    let stored = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&stored);
    let spill: SpillFn = Arc::new(move |hint: &str, content: &str| {
        sink.lock()
            .unwrap()
            .push((hint.to_string(), content.to_string()));
        Some(PathBuf::from("/artifacts/full-output.txt"))
    });
    (spill, stored)
}
