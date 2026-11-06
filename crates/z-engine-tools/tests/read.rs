//! `Read` on text: numbering, windows, long lines, empty files,
//! directories, missing and binary files, and read tracking.

mod support;

use serde_json::json;
use support::{ctx, err_text, ok_text, project};
use z_engine_policy::Action;
use z_engine_tools::builtin::ReadTool;
use z_engine_tools::{Tool, ToolLimits};

#[tokio::test]
async fn text_is_numbered_like_cat_n_and_recorded() {
    let dir = project(&[("src/a.txt", "alpha\nbeta\ngamma\n")]);
    let ctx = ctx(dir.path());
    let output = ReadTool
        .call(json!({"file_path": "src/a.txt"}), &ctx)
        .await
        .unwrap();
    assert_eq!(
        output.text_content(),
        "     1\talpha\n     2\tbeta\n     3\tgamma\n"
    );
    assert_eq!(output.summary, "Read 3 lines");
    assert!(!output.is_error);
    let path = dir.path().join("src/a.txt");
    assert_eq!(output.effects.files_read, vec![path.clone()]);
    assert!(ctx.files.check_fresh(&path).is_ok());
}

#[tokio::test]
async fn offset_and_limit_select_a_window() {
    let content: String = (1..=10).map(|i| format!("line{i}\n")).collect();
    let dir = project(&[("b.txt", &content)]);
    let text = ok_text(
        &ReadTool,
        &ctx(dir.path()),
        json!({"file_path": "b.txt", "offset": 4, "limit": "3"}),
    )
    .await;
    assert!(
        text.starts_with("     4\tline4\n     5\tline5\n     6\tline6\n"),
        "{text}"
    );
    assert!(!text.contains("line7"));
    assert!(text.contains("Continue with offset=7"));
}

#[tokio::test]
async fn default_line_limit_and_long_lines_follow_the_limits() {
    let long = "x".repeat(50);
    let content: String = (1..=30).map(|_| format!("{long}\n")).collect();
    let dir = project(&[("c.txt", &content)]);
    let mut ctx = ctx(dir.path());
    ctx.limits = ToolLimits {
        read_default_lines: 5,
        read_max_line_chars: 10,
        ..ToolLimits::default()
    };
    let text = ok_text(&ReadTool, &ctx, json!({"file_path": "c.txt"})).await;
    assert!(
        text.contains("     1\txxxxxxxxxx... [line truncated: 40 more characters]\n"),
        "{text}"
    );
    assert!(text.contains("     5\t"));
    assert!(!text.contains("     6\t"));
    assert!(text.contains("of 30"));
}

#[tokio::test]
async fn offset_past_the_end_is_an_error() {
    let dir = project(&[("d.txt", "one\n")]);
    let err = err_text(
        &ReadTool,
        &ctx(dir.path()),
        json!({"file_path": "d.txt", "offset": 99}),
    )
    .await;
    assert!(
        err.contains("past the end of the file, which has 1 lines"),
        "{err}"
    );
}

#[tokio::test]
async fn empty_files_get_a_short_note() {
    let dir = project(&[("e.txt", "")]);
    let path = dir.path().join("e.txt");
    let output = ReadTool
        .call(
            json!({"file_path": path.display().to_string()}),
            &ctx(dir.path()),
        )
        .await
        .unwrap();
    assert_eq!(output.text_content(), "e.txt exists but is empty.");
    assert_eq!(output.summary, "Empty file");
}

#[tokio::test]
async fn directories_and_missing_files_are_errors_with_guidance() {
    let dir = project(&[("sub/x.txt", "x")]);
    let ctx = ctx(dir.path());
    let err = err_text(&ReadTool, &ctx, json!({"file_path": "sub"})).await;
    assert!(
        err.contains("is a directory") && err.contains("Glob"),
        "{err}"
    );
    let err = err_text(&ReadTool, &ctx, json!({"file_path": "nope/missing.txt"})).await;
    assert!(
        err.contains("File does not exist: nope/missing.txt"),
        "{err}"
    );
    let err = err_text(&ReadTool, &ctx, json!({"offset": 1})).await;
    assert!(err.contains("missing required field `file_path`"), "{err}");
}

#[tokio::test]
async fn binary_files_are_described_not_dumped() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("blob.bin"), [0u8, 1, 2, 3, 0, 5]).unwrap();
    let output = ReadTool
        .call(json!({"file_path": "blob.bin"}), &ctx(dir.path()))
        .await
        .unwrap();
    assert_eq!(
        output.text_content(),
        "blob.bin is a binary file (6 bytes); its contents are not shown."
    );
}

#[tokio::test]
async fn pages_only_apply_to_pdfs() {
    let dir = project(&[("a.txt", "x\n")]);
    let err = err_text(
        &ReadTool,
        &ctx(dir.path()),
        json!({"file_path": "a.txt", "pages": "1-2"}),
    )
    .await;
    assert!(err.contains("`pages` applies only to PDF files"), "{err}");
}

#[tokio::test]
async fn action_and_title_name_the_resolved_file() {
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    let input = json!({"file_path": "src/main.rs"});
    assert_eq!(
        ReadTool.action(&input, &ctx),
        Action::Read {
            paths: vec![dir.path().join("src/main.rs")]
        }
    );
    assert_eq!(ReadTool.title(&input, &ctx), "Read src/main.rs");
    assert!(ReadTool.is_read_only(&input) && ReadTool.is_concurrency_safe(&input));
}
