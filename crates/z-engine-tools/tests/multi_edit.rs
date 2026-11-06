//! `MultiEdit`: ordered in-memory application, atomicity, and creation.

mod support;

use serde_json::json;
use support::{ctx, err_text, ok_text, project, read, read_file};
use z_engine_tools::Tool;
use z_engine_tools::builtin::MultiEditTool;

#[tokio::test]
async fn edits_apply_in_order_to_the_previous_result() {
    let dir = project(&[("a.rs", "fn old() {}\nold();\n")]);
    let ctx = ctx(dir.path());
    read(&ctx, "a.rs").await;
    let output = MultiEditTool
        .call(
            json!({"file_path": "a.rs", "edits": [
                {"old_string": "fn old() {}", "new_string": "fn new() {}"},
                {"old_string": "old();", "new_string": "new();"},
                {"old_string": "new", "new_string": "renamed", "replace_all": true}
            ]}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(
        read_file(dir.path(), "a.rs"),
        "fn renamed() {}\nrenamed();\n"
    );
    let text = output.text_content();
    assert!(text.contains("Edit 3: Replaced 2 occurrences."), "{text}");
    assert_eq!(output.effects.files_written, vec![dir.path().join("a.rs")]);
}

#[tokio::test]
async fn one_failing_edit_applies_none() {
    let dir = project(&[("b.txt", "alpha\nbeta\n")]);
    let ctx = ctx(dir.path());
    read(&ctx, "b.txt").await;
    let err = err_text(
        &MultiEditTool,
        &ctx,
        json!({"file_path": "b.txt", "edits": [
            {"old_string": "alpha", "new_string": "ALPHA"},
            {"old_string": "gamma", "new_string": "GAMMA"}
        ]}),
    )
    .await;
    assert!(
        err.starts_with("Edit 2 of 2 failed, so no edits were applied to b.txt"),
        "{err}"
    );
    assert_eq!(read_file(dir.path(), "b.txt"), "alpha\nbeta\n");
}

#[tokio::test]
async fn a_missing_file_is_created_by_an_empty_first_old_string() {
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    let output = MultiEditTool
        .call(
            json!({"file_path": "new.txt", "edits": [
                {"old_string": "", "new_string": "hello NAME\n"},
                {"old_string": "NAME", "new_string": "world"}
            ]}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(output.summary, "Created new.txt");
    assert_eq!(read_file(dir.path(), "new.txt"), "hello world\n");
    let err = err_text(
        &MultiEditTool,
        &ctx,
        json!({"file_path": "other.txt", "edits": [{"old_string": "a", "new_string": "b"}]}),
    )
    .await;
    assert!(
        err.contains("Edit 1 of 1 failed") && err.contains("does not exist"),
        "{err}"
    );
}

#[tokio::test]
async fn inputs_are_validated_per_edit() {
    let dir = project(&[("c.txt", "c\n")]);
    let ctx = ctx(dir.path());
    let err = err_text(
        &MultiEditTool,
        &ctx,
        json!({"file_path": "c.txt", "edits": []}),
    )
    .await;
    assert!(err.contains("at least one edit"), "{err}");
    let err = err_text(
        &MultiEditTool,
        &ctx,
        json!({"file_path": "c.txt", "edits": [{"old_string": "c"}]}),
    )
    .await;
    assert!(
        err.contains("edit 1:") && err.contains("new_string"),
        "{err}"
    );
    read(&ctx, "c.txt").await;
    let text = ok_text(
        &MultiEditTool,
        &ctx,
        json!({"file_path": "c.txt", "edits": [{"old_string": "c", "new_string": "d", "replace_all": "false"}]}),
    )
    .await;
    assert!(text.contains("     1\td\n"), "{text}");
    assert_eq!(
        MultiEditTool.title(&json!({"file_path": "c.txt", "edits": [{}, {}]}), &ctx),
        "Edit c.txt (2 edits)"
    );
}
