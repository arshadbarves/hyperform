//! Instruction and code-intelligence ports: `Skill` and `LSP`.

mod support;

use std::sync::Arc;

use serde_json::json;
use support::fakes::{FakeLsp, FakeSkills};
use support::{ctx, ctx_with, err_text, ok_text, project};
use z_engine_policy::Action;
use z_engine_tools::builtin::{LspTool, SkillTool};
use z_engine_tools::{Ports, Tool, ToolError};

#[tokio::test]
async fn skills_load_with_their_base_directory() {
    let dir = project(&[]);
    let ctx = ctx_with(
        dir.path(),
        Ports {
            skills: Some(Arc::new(FakeSkills)),
            ..Ports::default()
        },
    );
    let text = ok_text(&SkillTool, &ctx, json!({"skill": "/pdf"})).await;
    assert_eq!(
        text,
        "Skill: pdf\nBase directory for this skill: /skills/pdf\n\nUse scripts/extract.py."
    );
    let err = err_text(&SkillTool, &ctx, json!({"skill": "docx"})).await;
    assert!(
        err.contains("no skill named docx") && err.contains("Available skills: pdf."),
        "{err}"
    );
    assert_eq!(
        SkillTool.action(&json!({"skill": "pdf"}), &ctx),
        Action::Skill { name: "pdf".into() }
    );
    let missing = SkillTool
        .call(json!({"skill": "pdf"}), &ctx_plain(&dir))
        .await;
    assert!(matches!(missing, Err(ToolError::Unavailable(_))));
}

fn ctx_plain(dir: &tempfile::TempDir) -> z_engine_tools::ToolCtx {
    ctx(dir.path())
}

#[tokio::test]
async fn lsp_queries_are_validated_and_resolved() {
    let dir = project(&[]);
    let port = Arc::new(FakeLsp::default());
    let ctx = ctx_with(
        dir.path(),
        Ports {
            lsp: Some(port.clone()),
            ..Ports::default()
        },
    );
    let text = ok_text(
        &LspTool,
        &ctx,
        json!({"operation": "definition", "file_path": "src/lib.rs", "line": 3, "character": "7"}),
    )
    .await;
    assert_eq!(text, "definition -> src/lib.rs:10:5");
    let requests = port.requests.lock().unwrap().clone();
    assert_eq!(
        requests[0].file_path,
        Some(dir.path().join("src/lib.rs").display().to_string())
    );
    assert_eq!(
        (requests[0].line, requests[0].character),
        (Some(3), Some(7))
    );

    let err = err_text(
        &LspTool,
        &ctx,
        json!({"operation": "definition", "file_path": "a.rs"}),
    )
    .await;
    assert!(
        err.contains("`line (1-based)` is required for definition"),
        "{err}"
    );
    let err = err_text(&LspTool, &ctx, json!({"operation": "workspaceSymbols"})).await;
    assert!(err.contains("`query` is required"), "{err}");
    let err = err_text(
        &LspTool,
        &ctx,
        json!({"operation": "renamePreview", "file_path": "a.rs", "line": 1, "character": 1}),
    )
    .await;
    assert!(err.contains("`new_name` is required"), "{err}");
    let err = err_text(&LspTool, &ctx, json!({"operation": "typeDefinition"})).await;
    assert!(err.contains("`operation` must be one of"), "{err}");
    ok_text(&LspTool, &ctx, json!({"operation": "diagnostics"})).await;
}

#[tokio::test]
async fn lsp_is_a_read_of_the_file() {
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    let input = json!({"operation": "hover", "file_path": "src/a.rs", "line": 2, "character": 4});
    assert_eq!(
        LspTool.action(&input, &ctx),
        Action::Read {
            paths: vec![dir.path().join("src/a.rs")]
        }
    );
    assert_eq!(LspTool.title(&input, &ctx), "LSP hover src/a.rs:2:4");
    assert!(LspTool.is_read_only(&input));
    let err = LspTool.call(input, &ctx).await.unwrap_err();
    assert!(matches!(err, ToolError::Unavailable(_)));
}
