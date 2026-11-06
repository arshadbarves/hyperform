//! `Glob`: relative display paths, newest first, limits, bases, actions.

mod support;

use std::time::{Duration, SystemTime};

use serde_json::json;
use support::{ctx, err_text, ok_text, project};
use z_engine_policy::Action;
use z_engine_tools::Tool;
use z_engine_tools::builtin::GlobTool;

fn age(path: &std::path::Path, secs_ago: u64) {
    let when = SystemTime::now() - Duration::from_secs(secs_ago);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

#[tokio::test]
async fn matches_are_relative_and_newest_first() {
    let dir = project(&[
        ("src/a.rs", ""),
        ("src/deep/b.rs", ""),
        ("c.rs", ""),
        ("notes.md", ""),
    ]);
    age(&dir.path().join("src/a.rs"), 300);
    age(&dir.path().join("src/deep/b.rs"), 100);
    age(&dir.path().join("c.rs"), 200);
    let output = GlobTool
        .call(json!({"pattern": "**/*.rs"}), &ctx(dir.path()))
        .await
        .unwrap();
    assert_eq!(output.text_content(), "src/deep/b.rs\nc.rs\nsrc/a.rs\n");
    assert_eq!(output.summary, "Found 3 files");
}

#[tokio::test]
async fn path_narrows_the_search_and_single_star_stays_in_one_segment() {
    let dir = project(&[("src/a.rs", ""), ("src/deep/b.rs", ""), ("c.rs", "")]);
    let ctx = ctx(dir.path());
    let text = ok_text(&GlobTool, &ctx, json!({"pattern": "*.rs", "path": "src"})).await;
    assert_eq!(text, "src/a.rs\n");
    let text = ok_text(
        &GlobTool,
        &ctx,
        json!({"pattern": "*.rs", "path": "undefined"}),
    )
    .await;
    assert_eq!(text, "c.rs\n");
    let text = ok_text(&GlobTool, &ctx, json!({"pattern": "*.toml"})).await;
    assert_eq!(text, "No files found");
}

#[tokio::test]
async fn results_beyond_the_limit_are_truncated_with_a_note() {
    let files: Vec<(String, &str)> = (0..5).map(|i| (format!("f{i}.txt"), "")).collect();
    let refs: Vec<(&str, &str)> = files.iter().map(|(p, c)| (p.as_str(), *c)).collect();
    let dir = project(&refs);
    let mut ctx = ctx(dir.path());
    ctx.limits.glob_limit = 2;
    let output = GlobTool
        .call(json!({"pattern": "*.txt"}), &ctx)
        .await
        .unwrap();
    let text = output.text_content();
    assert_eq!(
        text.lines().filter(|line| line.ends_with(".txt")).count(),
        2
    );
    assert!(
        text.contains("Results are truncated to the 2 most recently modified files"),
        "{text}"
    );
    assert_eq!(output.summary, "Found more than 2 files");
}

#[tokio::test]
async fn bad_patterns_and_missing_directories_are_errors() {
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    let err = err_text(&GlobTool, &ctx, json!({"pattern": "a[b"})).await;
    assert!(err.starts_with("invalid input: bad glob"), "{err}");
    let err = err_text(&GlobTool, &ctx, json!({"pattern": "*", "path": "missing"})).await;
    assert!(err.contains("Directory does not exist: missing"), "{err}");
    let err = err_text(&GlobTool, &ctx, json!({"pattern": " "})).await;
    assert!(err.contains("`pattern` must not be empty"), "{err}");
}

#[tokio::test]
async fn gates_as_a_read_of_the_search_base() {
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    assert_eq!(
        GlobTool.action(&json!({"pattern": "*.rs"}), &ctx),
        Action::Read {
            paths: vec![dir.path().to_path_buf()]
        }
    );
    assert_eq!(
        GlobTool.action(&json!({"pattern": "*.rs", "path": "src"}), &ctx),
        Action::Read {
            paths: vec![dir.path().join("src")]
        }
    );
    assert_eq!(
        GlobTool.action(&json!({"pattern": "/etc/**/*.conf"}), &ctx),
        Action::Read {
            paths: vec!["/etc".into()]
        }
    );
    assert_eq!(
        GlobTool.title(&json!({"pattern": "*.rs", "path": "src"}), &ctx),
        "Glob *.rs in src"
    );
}
