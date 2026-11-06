//! `Edit`: read-before-edit and staleness, uniqueness and `replace_all`,
//! the fallback ladder, creation, snippets, and previews.

mod support;

use serde_json::json;
use support::{ctx, err_text, ok_text, project, read, read_file};
use z_engine_protocol::Preview;
use z_engine_tools::Tool;
use z_engine_tools::builtin::EditTool;

#[tokio::test]
async fn refuses_unread_and_stale_files() {
    let dir = project(&[("e.txt", "hello world\n")]);
    let ctx = ctx(dir.path());
    let input = json!({"file_path": "e.txt", "old_string": "world", "new_string": "there"});
    let err = err_text(&EditTool, &ctx, input.clone()).await;
    assert!(err.contains("has not been read yet"), "{err}");

    read(&ctx, "e.txt").await;
    std::fs::write(dir.path().join("e.txt"), "hello changed world\n").unwrap();
    let err = err_text(&EditTool, &ctx, input.clone()).await;
    assert!(err.contains("modified since it was last read"), "{err}");
    assert_eq!(read_file(dir.path(), "e.txt"), "hello changed world\n");

    read(&ctx, "e.txt").await;
    let output = EditTool.call(input, &ctx).await.unwrap();
    assert_eq!(read_file(dir.path(), "e.txt"), "hello changed there\n");
    assert_eq!(output.effects.files_written, vec![dir.path().join("e.txt")]);
    assert_eq!(output.summary, "Updated e.txt (+1 -1)");
}

#[tokio::test]
async fn result_shows_a_numbered_snippet_and_allows_chained_edits() {
    let content: String = (1..=12).map(|i| format!("line {i}\n")).collect();
    let dir = project(&[("m.txt", &content)]);
    let ctx = ctx(dir.path());
    read(&ctx, "m.txt").await;
    let text = ok_text(
        &EditTool,
        &ctx,
        json!({"file_path": "m.txt", "old_string": "line 6\nline 7", "new_string": "SIX\nSEVEN"}),
    )
    .await;
    assert!(
        text.starts_with("The file m.txt has been updated."),
        "{text}"
    );
    assert!(text.contains("     6\tSIX\n     7\tSEVEN\n"), "{text}");
    assert!(
        text.contains("     3\tline 3\n") && !text.contains("\tline 1\n"),
        "{text}"
    );
    // Our own write keeps the file fresh for the next edit.
    ok_text(
        &EditTool,
        &ctx,
        json!({"file_path": "m.txt", "old_string": "SIX", "new_string": "6"}),
    )
    .await;
    assert!(read_file(dir.path(), "m.txt").contains("line 5\n6\nSEVEN\n"));
}

#[tokio::test]
async fn multiple_matches_need_context_or_replace_all() {
    let dir = project(&[("r.rs", "let a = 1;\nlet b = a + a;\n")]);
    let ctx = ctx(dir.path());
    read(&ctx, "r.rs").await;
    let err = err_text(
        &EditTool,
        &ctx,
        json!({"file_path": "r.rs", "old_string": "a", "new_string": "x"}),
    )
    .await;
    assert!(
        err.contains("occurs 3 times") && err.contains("replace_all"),
        "{err}"
    );
    let text = ok_text(
        &EditTool,
        &ctx,
        json!({"file_path": "r.rs", "old_string": "a", "new_string": "x", "replace_all": true}),
    )
    .await;
    assert!(text.contains("Replaced 3 occurrences."), "{text}");
    assert_eq!(
        read_file(dir.path(), "r.rs"),
        "let x = 1;\nlet b = x + x;\n"
    );
}

#[tokio::test]
async fn whitespace_rung_matches_and_says_so() {
    let dir = project(&[("w.py", "def f():\n    if x:\n        return 1\n")]);
    let ctx = ctx(dir.path());
    read(&ctx, "w.py").await;
    let text = ok_text(
        &EditTool,
        &ctx,
        json!({"file_path": "w.py", "old_string": "if x:\n    return 1", "new_string": "if x:\n    return 2"}),
    )
    .await;
    assert!(
        text.contains("matched lines 2-3 after normalizing whitespace"),
        "{text}"
    );
    assert_eq!(
        read_file(dir.path(), "w.py"),
        "def f():\n    if x:\n        return 2\n"
    );
}

#[tokio::test]
async fn fuzzy_rung_matches_one_near_identical_block() {
    let content = "fn total(items: &[u32]) -> u32 {\n    items.iter().sum()\n}\n";
    let dir = project(&[("t.rs", content)]);
    let ctx = ctx(dir.path());
    read(&ctx, "t.rs").await;
    let text = ok_text(
        &EditTool,
        &ctx,
        json!({
            "file_path": "t.rs",
            "old_string": "fn total(items: &[u32]) -> u32 {\n    items.itre().sum()\n}",
            "new_string": "fn total(items: &[u32]) -> u64 {\n    items.iter().map(|&i| u64::from(i)).sum()\n}"
        }),
    )
    .await;
    assert!(text.contains("approximately (similarity 0.9"), "{text}");
    assert!(read_file(dir.path(), "t.rs").contains("-> u64"));
}

#[tokio::test]
async fn misses_point_at_the_most_similar_text() {
    let dir = project(&[("s.rs", "let total = compute_total(items, tax_rate);\n")]);
    let ctx = ctx(dir.path());
    read(&ctx, "s.rs").await;
    let err = err_text(
        &EditTool,
        &ctx,
        json!({"file_path": "s.rs", "old_string": "let total = compute_sum(items, rate);", "new_string": "x"}),
    )
    .await;
    assert!(err.contains("old_string was not found"), "{err}");
    assert!(
        err.contains("The most similar text is at lines 1-1"),
        "{err}"
    );
    assert!(err.contains("     1\tlet total = compute_total"), "{err}");
}

#[tokio::test]
async fn empty_old_string_creates_and_identical_strings_are_refused() {
    let dir = project(&[("full.txt", "x\n")]);
    let ctx = ctx(dir.path());
    let output = EditTool
        .call(
            json!({"file_path": "new/file.txt", "old_string": "", "new_string": "fresh\n"}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(output.summary, "Created new/file.txt");
    assert_eq!(read_file(dir.path(), "new/file.txt"), "fresh\n");
    read(&ctx, "full.txt").await;
    let err = err_text(
        &EditTool,
        &ctx,
        json!({"file_path": "full.txt", "old_string": "", "new_string": "y"}),
    )
    .await;
    assert!(err.contains("only creates a new file"), "{err}");
    let err = err_text(
        &EditTool,
        &ctx,
        json!({"file_path": "full.txt", "old_string": "x", "new_string": "x"}),
    )
    .await;
    assert!(err.contains("identical"), "{err}");
    let err = err_text(
        &EditTool,
        &ctx,
        json!({"file_path": "gone.txt", "old_string": "a", "new_string": "b"}),
    )
    .await;
    assert!(err.contains("does not exist"), "{err}");
    let err = err_text(
        &EditTool,
        &ctx,
        json!({"file_path": "nb.ipynb", "old_string": "a", "new_string": "b"}),
    )
    .await;
    assert!(err.contains("edit its cells with NotebookEdit"), "{err}");
}

#[tokio::test]
async fn preview_is_the_diff_the_edit_would_make() {
    let dir = project(&[("p.txt", "one\ntwo\nthree\n")]);
    let ctx = ctx(dir.path());
    let preview = EditTool
        .preview(
            &json!({"file_path": "p.txt", "old_string": "two", "new_string": "2"}),
            &ctx,
        )
        .await;
    let Some(Preview::Diff { path, diff }) = preview else {
        panic!("{preview:?}");
    };
    assert_eq!(path, "p.txt");
    assert!(diff.contains("-two\n+2\n"), "{diff}");
    let none = EditTool
        .preview(
            &json!({"file_path": "p.txt", "old_string": "absent text here", "new_string": "2"}),
            &ctx,
        )
        .await;
    assert!(none.is_none());
    assert_eq!(read_file(dir.path(), "p.txt"), "one\ntwo\nthree\n");
}
