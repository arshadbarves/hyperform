//! `NotebookEdit`: replace, insert, and delete cells in Jupyter's format.

mod support;

use serde_json::{Value, json};
use support::{ctx, err_text, ok_text, read, read_file};
use z_engine_protocol::Preview;
use z_engine_tools::Tool;
use z_engine_tools::builtin::NotebookEditTool;

fn notebook() -> Value {
    json!({
        "cells": [
            {"cell_type": "markdown", "id": "intro", "metadata": {"collapsed": true}, "source": ["# Title"]},
            {"cell_type": "code", "id": "calc", "metadata": {}, "execution_count": 4,
             "outputs": [{"output_type": "stream", "name": "stdout", "text": ["2\n"]}],
             "source": ["1 + 1"]}
        ],
        "metadata": {"kernelspec": {"language": "python", "name": "python3"}},
        "nbformat": 4,
        "nbformat_minor": 5
    })
}

fn setup() -> (tempfile::TempDir, z_engine_tools::ToolCtx) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("nb.ipynb"), notebook().to_string()).unwrap();
    let ctx = ctx(dir.path());
    (dir, ctx)
}

fn saved(dir: &tempfile::TempDir) -> Value {
    serde_json::from_str(&read_file(dir.path(), "nb.ipynb")).unwrap()
}

#[tokio::test]
async fn edits_require_a_fresh_read() {
    let (_dir, ctx) = setup();
    let err = err_text(
        &NotebookEditTool,
        &ctx,
        json!({"notebook_path": "nb.ipynb", "cell_id": "calc", "new_source": "2 + 2"}),
    )
    .await;
    assert!(err.contains("has not been read yet"), "{err}");
}

#[tokio::test]
async fn replace_keeps_other_fields_and_clears_stale_outputs() {
    let (dir, ctx) = setup();
    read(&ctx, "nb.ipynb").await;
    let output = NotebookEditTool
        .call(
            json!({"notebook_path": "nb.ipynb", "cell_id": "calc", "new_source": "x = 2\nx * 2"}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(output.summary, "Updated cell calc in nb.ipynb.");
    assert_eq!(
        output.effects.files_written,
        vec![dir.path().join("nb.ipynb")]
    );
    let nb = saved(&dir);
    assert_eq!(nb["cells"][1]["source"], json!(["x = 2\n", "x * 2"]));
    assert_eq!(nb["cells"][1]["outputs"], json!([]));
    assert_eq!(nb["cells"][1]["execution_count"], Value::Null);
    assert_eq!(nb["cells"][0], notebook()["cells"][0]);
    assert_eq!(nb["metadata"], notebook()["metadata"]);
    let text = read_file(dir.path(), "nb.ipynb");
    assert!(
        text.contains("\n \"cells\": [\n  {\n   \""),
        "Jupyter indent: {text}"
    );
    assert!(text.ends_with("}\n"));
}

#[tokio::test]
async fn insert_and_delete_cells() {
    let (dir, ctx) = setup();
    read(&ctx, "nb.ipynb").await;
    let text = ok_text(
        &NotebookEditTool,
        &ctx,
        json!({"notebook_path": "nb.ipynb", "cell_id": "intro", "new_source": "import os", "cell_type": "code", "edit_mode": "insert"}),
    )
    .await;
    assert!(
        text.starts_with("Inserted code cell ") && text.contains("after cell intro"),
        "{text}"
    );
    let nb = saved(&dir);
    assert_eq!(nb["cells"].as_array().unwrap().len(), 3);
    assert_eq!(nb["cells"][1]["source"], json!(["import os"]));
    assert_eq!(nb["cells"][1]["outputs"], json!([]));
    assert!(
        nb["cells"][1]["id"]
            .as_str()
            .is_some_and(|id| id.len() == 8)
    );

    ok_text(
        &NotebookEditTool,
        &ctx,
        json!({"notebook_path": "nb.ipynb", "cell_id": "cell-0", "new_source": "", "edit_mode": "delete"}),
    )
    .await;
    let nb = saved(&dir);
    assert_eq!(nb["cells"].as_array().unwrap().len(), 2);
    assert_eq!(nb["cells"][1]["id"], "calc");

    let err = err_text(
        &NotebookEditTool,
        &ctx,
        json!({"notebook_path": "nb.ipynb", "new_source": "x", "edit_mode": "insert"}),
    )
    .await;
    assert!(err.contains("`cell_type`"), "{err}");
    let err = err_text(
        &NotebookEditTool,
        &ctx,
        json!({"notebook_path": "nb.ipynb", "cell_id": "nope", "new_source": "x"}),
    )
    .await;
    assert!(err.contains("\"nope\" was not found"), "{err}");
}

#[tokio::test]
async fn preview_diffs_the_cell_source() {
    let (_dir, ctx) = setup();
    let preview = NotebookEditTool
        .preview(
            &json!({"notebook_path": "nb.ipynb", "cell_id": "calc", "new_source": "1 + 2"}),
            &ctx,
        )
        .await;
    let Some(Preview::Diff { diff, .. }) = preview else {
        panic!("{preview:?}");
    };
    assert!(diff.contains("-1 + 1") && diff.contains("+1 + 2"), "{diff}");
    let err = err_text(
        &NotebookEditTool,
        &ctx,
        json!({"notebook_path": "a.py", "new_source": "x"}),
    )
    .await;
    assert!(err.contains("use Edit"), "{err}");
}
