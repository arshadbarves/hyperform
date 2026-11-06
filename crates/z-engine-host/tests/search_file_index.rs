//! The `@`-mention file index: fuzzy ranking, ignores, recency, limits.

mod support;

use support::{set_mtime, write_files};
use z_engine_host::FileIndex;

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_files(
        dir.path(),
        &[
            ("src/main.rs", ""),
            ("src/lib.rs", ""),
            ("src/domain/manifest.rs", ""),
            ("docs/maintenance.md", ""),
            ("README.md", ""),
            (".github/workflows/ci.yml", ""),
            (".gitignore", "secret/\n"),
            ("secret/key.txt", ""),
            ("node_modules/pkg/index.js", ""),
            ("target/debug/out.txt", ""),
        ],
    );
    dir
}

#[test]
fn basenames_and_segment_starts_rank_first() {
    let dir = fixture();
    let index = FileIndex::build(dir.path(), 1_000).unwrap();
    assert!(!index.truncated());
    assert_eq!(index.search("main", 3)[0], "src/main.rs");
    assert_eq!(index.search("slib", 3)[0], "src/lib.rs");
    assert_eq!(index.search("ci.yml", 1), [".github/workflows/ci.yml"]);
    let domain = index.search("domain", 5);
    assert_eq!(domain[0], "src/domain/");
    assert!(domain.contains(&"src/domain/manifest.rs".to_string()));
    assert!(index.search("zzz", 5).is_empty());
}

#[test]
fn ignored_and_vendored_directories_are_not_indexed() {
    let dir = fixture();
    let index = FileIndex::build(dir.path(), 1_000).unwrap();
    assert!(index.search("key", 10).is_empty());
    assert!(index.search("index.js", 10).is_empty());
    assert!(index.search("out.txt", 10).is_empty());
}

#[test]
fn empty_queries_list_recently_modified_files() {
    let dir = fixture();
    for rel in [
        "src/domain/manifest.rs",
        "docs/maintenance.md",
        ".github/workflows/ci.yml",
        ".gitignore",
    ] {
        set_mtime(&dir.path().join(rel), 0);
    }
    for (rel, offset) in [("README.md", 30), ("src/lib.rs", 20), ("src/main.rs", 10)] {
        set_mtime(&dir.path().join(rel), offset);
    }
    let index = FileIndex::build(dir.path(), 1_000).unwrap();
    assert_eq!(index.search("", 2), ["README.md", "src/lib.rs"]);
    assert!(index.search("  ", 50).iter().all(|p| !p.ends_with('/')));
}

#[test]
fn the_walk_stops_at_max_files() {
    let dir = fixture();
    let index = FileIndex::build(dir.path(), 3).unwrap();
    assert_eq!(index.len(), 3);
    assert!(index.truncated());
}
