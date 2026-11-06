//! File globbing: semantics, ignore rules, ordering, and limits.

mod support;

use std::path::{Path, PathBuf};

use support::{set_mtime, write_files};
use z_engine_host::{HostError, glob};

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_files(
        dir.path(),
        &[
            ("src/a.rs", "a"),
            ("src/deep/b.rs", "b"),
            ("top.rs", "top"),
            (".hidden.rs", "hidden"),
            ("notes.md", "notes"),
            (".gitignore", "ignored/\n"),
            ("ignored/secret.rs", "secret"),
            (".git/hooks/x.rs", "git internals"),
        ],
    );
    for (rel, offset) in [
        ("src/a.rs", 0),
        (".hidden.rs", 5),
        ("src/deep/b.rs", 10),
        ("top.rs", 20),
    ] {
        set_mtime(&dir.path().join(rel), offset);
    }
    dir
}

fn rel(root: &Path, paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| {
            p.strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

#[test]
fn recursive_patterns_list_files_newest_first_honoring_ignores() {
    let dir = fixture();
    let result = glob(dir.path(), "**/*.rs", None, 100).unwrap();
    assert!(!result.truncated);
    assert!(result.paths.iter().all(|p| p.is_absolute()));
    assert_eq!(
        rel(dir.path(), &result.paths),
        ["top.rs", "src/deep/b.rs", ".hidden.rs", "src/a.rs"]
    );
}

#[test]
fn a_star_stays_within_one_segment() {
    let dir = fixture();
    let top = glob(dir.path(), "*.rs", None, 100).unwrap();
    assert_eq!(rel(dir.path(), &top.paths), ["top.rs", ".hidden.rs"]);
    let nested = glob(dir.path(), "src/*.rs", None, 100).unwrap();
    assert_eq!(rel(dir.path(), &nested.paths), ["src/a.rs"]);
    let dotted = glob(dir.path(), "./src/**/b.rs", None, 100).unwrap();
    assert_eq!(rel(dir.path(), &dotted.paths), ["src/deep/b.rs"]);
}

#[test]
fn patterns_match_relative_to_the_base() {
    let dir = fixture();
    let under_src = glob(dir.path(), "*.rs", Some(Path::new("src")), 100).unwrap();
    assert_eq!(rel(dir.path(), &under_src.paths), ["src/a.rs"]);
    let absolute_base = dir.path().join("src");
    let all = glob(dir.path(), "**/*.rs", Some(&absolute_base), 100).unwrap();
    assert_eq!(rel(dir.path(), &all.paths), ["src/deep/b.rs", "src/a.rs"]);
}

#[test]
fn limits_truncate_after_sorting() {
    let dir = fixture();
    let result = glob(dir.path(), "**/*.rs", None, 2).unwrap();
    assert!(result.truncated);
    assert_eq!(rel(dir.path(), &result.paths), ["top.rs", "src/deep/b.rs"]);
}

#[test]
fn bad_patterns_and_missing_bases_are_errors() {
    let dir = fixture();
    assert!(matches!(
        glob(dir.path(), "a[", None, 10),
        Err(HostError::Invalid(_))
    ));
    assert!(matches!(
        glob(dir.path(), "  ", None, 10),
        Err(HostError::Invalid(_))
    ));
    assert!(matches!(
        glob(dir.path(), "*.rs", Some(Path::new("nope")), 10),
        Err(HostError::NotFound(_))
    ));
}
