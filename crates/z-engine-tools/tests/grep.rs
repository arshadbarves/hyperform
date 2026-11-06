//! `Grep`: output modes, context, flags, filters, paging, and errors.

mod support;

use serde_json::json;
use support::{ctx, err_text, ok_text, project};
use z_engine_policy::Action;
use z_engine_tools::Tool;
use z_engine_tools::builtin::GrepTool;

fn fixture() -> tempfile::TempDir {
    project(&[
        (
            "src/lib.rs",
            "fn alpha() {}\nfn beta() {}\n// TODO: gamma\n",
        ),
        ("src/main.rs", "fn main() {\n    alpha();\n}\n"),
        ("docs/notes.md", "Alpha notes\nTODO later\n"),
    ])
}

#[tokio::test]
async fn files_with_matches_is_the_default() {
    let dir = fixture();
    let output = GrepTool
        .call(json!({"pattern": "alpha"}), &ctx(dir.path()))
        .await
        .unwrap();
    assert_eq!(
        output.text_content(),
        "Found 2 files\nsrc/lib.rs\nsrc/main.rs\n"
    );
    assert_eq!(output.summary, "Found 2 files");
}

#[tokio::test]
async fn content_mode_shows_lines_with_context_and_numbers() {
    let dir = fixture();
    let ctx = ctx(dir.path());
    let text = ok_text(
        &GrepTool,
        &ctx,
        json!({"pattern": "beta", "output_mode": "content"}),
    )
    .await;
    assert_eq!(text, "src/lib.rs:2:fn beta() {}\n");
    let text = ok_text(
        &GrepTool,
        &ctx,
        json!({"pattern": "beta", "output_mode": "content", "-C": 1}),
    )
    .await;
    assert_eq!(
        text,
        "src/lib.rs-1-fn alpha() {}\nsrc/lib.rs:2:fn beta() {}\nsrc/lib.rs-3-// TODO: gamma\n"
    );
    let text = ok_text(
        &GrepTool,
        &ctx,
        json!({"pattern": "beta", "output_mode": "content", "-n": false}),
    )
    .await;
    assert_eq!(text, "src/lib.rs:fn beta() {}\n");
}

#[tokio::test]
async fn count_mode_case_folding_and_filters() {
    let dir = fixture();
    let ctx = ctx(dir.path());
    let text = ok_text(
        &GrepTool,
        &ctx,
        json!({"pattern": "todo", "-i": true, "output_mode": "count"}),
    )
    .await;
    assert!(
        text.starts_with("docs/notes.md:1\nsrc/lib.rs:1\n"),
        "{text}"
    );
    assert!(text.contains("Found 2 matches across 2 files."), "{text}");
    let text = ok_text(
        &GrepTool,
        &ctx,
        json!({"pattern": "(?i)alpha", "glob": "*.md"}),
    )
    .await;
    assert_eq!(text, "Found 1 file\ndocs/notes.md\n");
    let text = ok_text(
        &GrepTool,
        &ctx,
        json!({"pattern": "alpha", "type": "rust", "path": "src/main.rs"}),
    )
    .await;
    assert_eq!(text, "Found 1 file\nsrc/main.rs\n");
}

#[tokio::test]
async fn multiline_patterns_span_lines() {
    let dir = fixture();
    let text = ok_text(
        &GrepTool,
        &ctx(dir.path()),
        json!({"pattern": "fn main\\(\\) \\{\\s+alpha", "multiline": true}),
    )
    .await;
    assert_eq!(text, "Found 1 file\nsrc/main.rs\n");
}

#[tokio::test]
async fn head_limit_and_offset_page_through_results() {
    let dir = fixture();
    let ctx = ctx(dir.path());
    let text = ok_text(
        &GrepTool,
        &ctx,
        json!({"pattern": "fn", "output_mode": "content", "head_limit": 2}),
    )
    .await;
    assert!(
        text.starts_with("src/lib.rs:1:fn alpha() {}\nsrc/lib.rs:2:fn beta() {}\n"),
        "{text}"
    );
    assert!(text.contains("continue with offset=2"), "{text}");
    let text = ok_text(
        &GrepTool,
        &ctx,
        json!({"pattern": "fn", "output_mode": "content", "head_limit": 2, "offset": 2}),
    )
    .await;
    assert!(text.starts_with("src/main.rs:1:fn main() {\n"), "{text}");
    assert!(!text.contains("continue with offset"), "{text}");
}

#[tokio::test]
async fn misses_and_bad_input() {
    let dir = fixture();
    let ctx = ctx(dir.path());
    let output = GrepTool
        .call(json!({"pattern": "zzz_nothing"}), &ctx)
        .await
        .unwrap();
    assert_eq!(output.text_content(), "No matches found");
    let err = err_text(
        &GrepTool,
        &ctx,
        json!({"pattern": "fn(", "output_mode": "content"}),
    )
    .await;
    assert!(err.starts_with("invalid input: bad pattern"), "{err}");
    let err = err_text(
        &GrepTool,
        &ctx,
        json!({"pattern": "x", "output_mode": "lines"}),
    )
    .await;
    assert!(err.contains("`output_mode` must be"), "{err}");
    let err = err_text(&GrepTool, &ctx, json!({"pattern": "x", "path": "nowhere"})).await;
    assert!(err.contains("Path does not exist: nowhere"), "{err}");
}

#[tokio::test]
async fn gates_as_a_read_of_the_search_path() {
    let dir = fixture();
    let ctx = ctx(dir.path());
    assert_eq!(
        GrepTool.action(&json!({"pattern": "x", "path": "src"}), &ctx),
        Action::Read {
            paths: vec![dir.path().join("src")]
        }
    );
    assert_eq!(
        GrepTool.title(&json!({"pattern": "x", "path": "src"}), &ctx),
        "Grep \"x\" in src"
    );
    assert!(GrepTool.is_concurrency_safe(&json!({"pattern": "x"})));
}
