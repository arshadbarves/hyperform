//! Grep with both engines: every case runs on the builtin engine and, when
//! `rg` is installed, on ripgrep too, and the outputs must be identical.

mod support;

use std::path::{Path, PathBuf};

use support::write_files;
use tokio_util::sync::CancellationToken;
use z_engine_host::{
    GrepEngine, GrepMode, GrepQuery, GrepResult, HostError, grep, grep_with_engine, rg_available,
};

const A_RS: &str =
    "fn alpha() {}\nfn beta() {}\nlet x = 1;\nlet y = 2;\nfn alpha2() {\n  body\n}\n";

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let long = format!("alpha{}\n", "x".repeat(600));
    write_files(
        dir.path(),
        &[
            ("src/a.rs", A_RS),
            ("src/deep/b.rs", "let alpha_count = 1;\n"),
            ("notes.md", "# Alpha notes\nalpha lower\n"),
            (".hidden.txt", "alpha hidden\n"),
            (".gitignore", "ignored/\n"),
            ("ignored/secret.rs", "alpha secret\n"),
            (".git/config", "alpha in the git dir\n"),
            ("long.txt", &long),
        ],
    );
    std::fs::write(dir.path().join("bin.dat"), b"alpha\0binary\n").unwrap();
    dir
}

fn query(pattern: &str, mode: GrepMode) -> GrepQuery {
    GrepQuery {
        mode,
        ..GrepQuery::new(pattern)
    }
}

/// Runs `q` on every available engine, asserting they agree.
async fn both(root: &Path, q: &GrepQuery) -> GrepResult {
    let builtin = grep_with_engine(root, q, CancellationToken::new(), GrepEngine::Builtin)
        .await
        .unwrap();
    if rg_available() {
        let rg = grep_with_engine(root, q, CancellationToken::new(), GrepEngine::Ripgrep)
            .await
            .unwrap();
        assert_eq!(rg.text, builtin.text, "engines disagree on {q:?}");
        assert_eq!(
            (rg.matches, rg.files, rg.truncated),
            (builtin.matches, builtin.files, builtin.truncated),
            "engines disagree on counts for {q:?}"
        );
    }
    builtin
}

async fn both_err(root: &Path, q: &GrepQuery) -> Vec<HostError> {
    let mut engines = vec![GrepEngine::Builtin];
    if rg_available() {
        engines.push(GrepEngine::Ripgrep);
    }
    let mut errors = Vec::new();
    for engine in engines {
        let cancel = CancellationToken::new();
        errors.push(grep_with_engine(root, q, cancel, engine).await.unwrap_err());
    }
    errors
}

#[tokio::test]
async fn files_mode_honors_ignores_and_skips_binary_and_git() {
    let dir = fixture();
    let result = both(dir.path(), &query("alpha", GrepMode::FilesWithMatches)).await;
    assert_eq!(
        result.text,
        ".hidden.txt\nlong.txt\nnotes.md\nsrc/a.rs\nsrc/deep/b.rs"
    );
    assert_eq!(
        (result.files, result.matches, result.truncated),
        (5, 5, false)
    );
}

#[tokio::test]
async fn content_mode_with_and_without_line_numbers() {
    let dir = fixture();
    let numbered = both(dir.path(), &query("beta", GrepMode::Content)).await;
    assert_eq!(numbered.text, "src/a.rs:2:fn beta() {}");

    let mut plain = query("alpha", GrepMode::Content);
    plain.line_numbers = false;
    plain.glob = Some("*.rs".to_string());
    let result = both(dir.path(), &plain).await;
    assert_eq!(
        result.text,
        "src/a.rs:fn alpha() {}\nsrc/a.rs:fn alpha2() {\nsrc/deep/b.rs:let alpha_count = 1;"
    );
    assert_eq!((result.matches, result.files), (3, 2));
}

#[tokio::test]
async fn count_mode_reports_matching_lines_per_file() {
    let dir = fixture();
    let result = both(dir.path(), &query("alpha", GrepMode::Count)).await;
    assert_eq!(
        result.text,
        ".hidden.txt:1\nlong.txt:1\nnotes.md:1\nsrc/a.rs:2\nsrc/deep/b.rs:1"
    );
    assert_eq!(result.matches, 6);
}

#[tokio::test]
async fn context_lines_use_dashes_and_group_separators() {
    let dir = fixture();
    let mut after = query("^let", GrepMode::Content);
    after.path = Some(PathBuf::from("src/a.rs"));
    after.after = 1;
    let result = both(dir.path(), &after).await;
    assert_eq!(
        result.text,
        "src/a.rs:3:let x = 1;\nsrc/a.rs:4:let y = 2;\nsrc/a.rs-5-fn alpha2() {"
    );

    let mut before = query("alpha", GrepMode::Content);
    before.file_type = Some("rust".to_string());
    before.before = 1;
    let result = both(dir.path(), &before).await;
    assert_eq!(
        result.text,
        "src/a.rs:1:fn alpha() {}\n--\nsrc/a.rs-4-let y = 2;\nsrc/a.rs:5:fn alpha2() {\n--\nsrc/deep/b.rs:1:let alpha_count = 1;"
    );
    assert_eq!(result.matches, 3);
}

#[tokio::test]
async fn multiline_patterns_span_lines() {
    let dir = fixture();
    let mut spanning = query(r"alpha2\(\) \{.*?\}", GrepMode::Content);
    spanning.multiline = true;
    let result = both(dir.path(), &spanning).await;
    assert_eq!(
        result.text,
        "src/a.rs:5:fn alpha2() {\nsrc/a.rs:6:  body\nsrc/a.rs:7:}"
    );
    spanning.mode = GrepMode::Count;
    assert_eq!(both(dir.path(), &spanning).await.text, "src/a.rs:1");
}

#[tokio::test]
async fn case_glob_and_type_filters() {
    let dir = fixture();
    let mut insensitive = query("ALPHA NOTES", GrepMode::Content);
    insensitive.case_insensitive = true;
    assert_eq!(
        both(dir.path(), &insensitive).await.text,
        "notes.md:1:# Alpha notes"
    );
    assert_eq!(
        both(dir.path(), &query("ALPHA", GrepMode::Content))
            .await
            .text,
        ""
    );

    let mut markdown = query("alpha", GrepMode::FilesWithMatches);
    markdown.file_type = Some("md".to_string());
    assert_eq!(both(dir.path(), &markdown).await.text, "notes.md");

    let mut anchored = query("alpha", GrepMode::FilesWithMatches);
    anchored.glob = Some("src/**/*.rs".to_string());
    assert_eq!(
        both(dir.path(), &anchored).await.text,
        "src/a.rs\nsrc/deep/b.rs"
    );
}

#[tokio::test]
async fn head_limit_and_offset_window_the_output() {
    let dir = fixture();
    let mut window = query("alpha", GrepMode::Count);
    window.offset = 1;
    window.head_limit = Some(2);
    let result = both(dir.path(), &window).await;
    assert_eq!(result.text, "long.txt:1\nnotes.md:1");
    assert!(result.truncated);
    assert_eq!(result.files, 5, "totals cover the whole result");

    let mut first_line = query("alpha", GrepMode::Content);
    first_line.head_limit = Some(1);
    let result = both(dir.path(), &first_line).await;
    assert_eq!(result.text, ".hidden.txt:1:alpha hidden");
    assert!(result.truncated);

    let mut past_end = query("alpha", GrepMode::FilesWithMatches);
    past_end.offset = 10;
    let result = both(dir.path(), &past_end).await;
    assert_eq!(result.text, "");
    assert!(!result.truncated);
}

#[tokio::test]
async fn paths_narrow_the_search_and_stay_root_relative() {
    let dir = fixture();
    let mut subdir = query("alpha", GrepMode::FilesWithMatches);
    subdir.path = Some(PathBuf::from("src"));
    assert_eq!(
        both(dir.path(), &subdir).await.text,
        "src/a.rs\nsrc/deep/b.rs"
    );

    let mut file = query("alpha", GrepMode::Content);
    file.path = Some(dir.path().join("src/deep/b.rs"));
    assert_eq!(
        both(dir.path(), &file).await.text,
        "src/deep/b.rs:1:let alpha_count = 1;"
    );

    // An explicitly named file is searched even if a filter excludes it.
    let mut named = query("alpha", GrepMode::FilesWithMatches);
    named.path = Some(PathBuf::from("src/a.rs"));
    named.glob = Some("*.md".to_string());
    assert_eq!(both(dir.path(), &named).await.text, "src/a.rs");
    named.glob = None;
    named.file_type = Some("md".to_string());
    assert_eq!(both(dir.path(), &named).await.text, "src/a.rs");
}

#[tokio::test]
async fn long_lines_are_previewed_like_ripgrep() {
    let dir = fixture();
    let mut long = query("alpha", GrepMode::Content);
    long.path = Some(PathBuf::from("long.txt"));
    let result = both(dir.path(), &long).await;
    let expected = format!(
        "long.txt:1:alpha{} [... omitted end of long line]",
        "x".repeat(495)
    );
    assert_eq!(result.text, expected);
}

#[tokio::test]
async fn invalid_queries_fail_the_same_way_on_both_engines() {
    let dir = fixture();
    let invalid = [
        query("(", GrepMode::Content),
        query("", GrepMode::Content),
        GrepQuery {
            file_type: Some("no-such-type".to_string()),
            ..query("alpha", GrepMode::Content)
        },
        GrepQuery {
            glob: Some("a[".to_string()),
            ..query("alpha", GrepMode::Content)
        },
    ];
    for q in &invalid {
        for error in both_err(dir.path(), q).await {
            assert!(matches!(error, HostError::Invalid(_)), "{q:?}: {error}");
        }
    }
    let missing = GrepQuery {
        path: Some(PathBuf::from("nope")),
        ..query("alpha", GrepMode::Content)
    };
    for error in both_err(dir.path(), &missing).await {
        assert!(error.is_not_found(), "{error}");
    }
}

#[tokio::test]
async fn cancellation_and_engine_selection() {
    let dir = fixture();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let q = query("alpha", GrepMode::Content);
    for engine in [GrepEngine::Builtin, GrepEngine::Ripgrep] {
        if engine == GrepEngine::Ripgrep && !rg_available() {
            continue;
        }
        let result = grep_with_engine(dir.path(), &q, cancel.clone(), engine).await;
        assert!(matches!(result, Err(HostError::Cancelled)));
    }
    let auto = grep(dir.path(), &q, CancellationToken::new())
        .await
        .unwrap();
    let expected = if rg_available() {
        GrepEngine::Ripgrep
    } else {
        GrepEngine::Builtin
    };
    assert_eq!(auto.engine, expected);
}
