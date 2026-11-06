//! `Write`: creation, read-before-overwrite, staleness, previews, effects.

mod support;

use serde_json::json;
use support::{ctx, err_text, project, read, read_file};
use z_engine_policy::Action;
use z_engine_protocol::Preview;
use z_engine_tools::Tool;
use z_engine_tools::builtin::WriteTool;

#[tokio::test]
async fn creates_new_files_and_parent_directories() {
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    let output = WriteTool
        .call(
            json!({"file_path": "deep/sub/new.txt", "content": "a\nb\n"}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(read_file(dir.path(), "deep/sub/new.txt"), "a\nb\n");
    assert_eq!(output.summary, "Created deep/sub/new.txt");
    assert_eq!(
        output.effects.files_written,
        vec![dir.path().join("deep/sub/new.txt")]
    );
    // The model's own write counts as seen: a second write needs no read.
    let output = WriteTool
        .call(
            json!({"file_path": "deep/sub/new.txt", "content": "c\n"}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(output.summary, "Wrote 1 lines to deep/sub/new.txt");
}

#[tokio::test]
async fn overwriting_requires_a_fresh_read() {
    let dir = project(&[("a.txt", "old\n")]);
    let ctx = ctx(dir.path());
    let input = json!({"file_path": "a.txt", "content": "new\n"});
    let err = err_text(&WriteTool, &ctx, input.clone()).await;
    assert!(
        err.contains("a.txt has not been read yet. Read it first"),
        "{err}"
    );
    assert_eq!(read_file(dir.path(), "a.txt"), "old\n");

    read(&ctx, "a.txt").await;
    std::fs::write(dir.path().join("a.txt"), "changed by the user\n").unwrap();
    let err = err_text(&WriteTool, &ctx, input.clone()).await;
    assert!(
        err.contains("has been modified since it was last read"),
        "{err}"
    );

    read(&ctx, "a.txt").await;
    let output = WriteTool.call(input, &ctx).await.unwrap();
    assert!(
        output
            .text_content()
            .contains("replacing its previous content (+1 -1"),
        "{}",
        output.text_content()
    );
    assert_eq!(read_file(dir.path(), "a.txt"), "new\n");
}

#[tokio::test]
async fn previews_are_diffs_for_new_and_existing_files() {
    let dir = project(&[("b.txt", "line1\nline2\n")]);
    let ctx = ctx(dir.path());
    let preview = WriteTool
        .preview(
            &json!({"file_path": "b.txt", "content": "line1\nTWO\n"}),
            &ctx,
        )
        .await;
    let Some(Preview::Diff { path, diff }) = preview else {
        panic!("{preview:?}");
    };
    assert_eq!(path, "b.txt");
    assert!(diff.contains("-line2\n+TWO\n"), "{diff}");
    let preview = WriteTool
        .preview(&json!({"file_path": "c.txt", "content": "hello\n"}), &ctx)
        .await;
    let Some(Preview::Diff { diff, .. }) = preview else {
        panic!("{preview:?}");
    };
    assert!(diff.starts_with("--- /dev/null\n+++ b/c.txt\n"), "{diff}");
    assert!(diff.contains("+hello"));
}

#[tokio::test]
async fn gates_as_a_write_of_the_resolved_path() {
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    let input = json!({"file_path": "x/y.rs", "content": ""});
    assert_eq!(
        WriteTool.action(&input, &ctx),
        Action::Write {
            paths: vec![dir.path().join("x/y.rs")]
        }
    );
    assert!(!WriteTool.is_read_only(&input) && !WriteTool.is_concurrency_safe(&input));
    assert_eq!(WriteTool.title(&input, &ctx), "Write x/y.rs");
    let err = err_text(&WriteTool, &ctx, json!({"file_path": "z.txt"})).await;
    assert!(err.contains("missing required field `content`"), "{err}");
}
