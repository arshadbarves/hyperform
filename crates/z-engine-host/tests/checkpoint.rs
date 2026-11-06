//! Shadow-repository checkpoints: exact restores, untouched paths, and
//! isolation from the user's own repository.

mod support;

use std::collections::BTreeMap;
use std::path::Path;

use support::{git as run_git, repo_with, set_mtime, write_files};
use z_engine_host::{ChangeKind, HostError, PathChange, ShadowRepo, git_available};

/// Every file under `root` (outside `.git` and `.z-engine`) with content.
fn tree(root: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if rel == ".git" || rel == ".z-engine" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                files.insert(rel, std::fs::read_to_string(&path).unwrap());
            }
        }
    }
    files
}

fn change(path: &str, kind: ChangeKind) -> PathChange {
    PathChange {
        path: path.to_string(),
        kind,
    }
}

#[tokio::test]
async fn restore_brings_back_the_exact_tree_including_shell_changes() {
    if !git_available() {
        return;
    }
    let project = tempfile::tempdir().unwrap();
    write_files(
        project.path(),
        &[
            ("a.txt", "a0"),
            ("b.txt", "b0"),
            ("sub/c.txt", "c0"),
            (".gitignore", "ignored.txt\n"),
            ("ignored.txt", "i0"),
        ],
    );
    let store = project.path().join(".z-engine/checkpoints");
    let shadow = ShadowRepo::open(&store, project.path()).await.unwrap();
    let before = tree(project.path());
    let first = shadow.snapshot("turn 1").await.unwrap();
    assert_eq!(shadow.snapshot("unchanged").await.unwrap(), first);

    write_files(
        project.path(),
        &[
            ("a.txt", "a1"),
            ("sub/new/d.txt", "d"),
            ("ignored.txt", "i1"),
        ],
    );
    std::fs::remove_file(project.path().join("b.txt")).unwrap();
    #[cfg(unix)]
    {
        let command = "echo shell > made_by_shell.txt && rm sub/c.txt";
        let spec = z_engine_host::RunSpec::new(command, project.path());
        let out = z_engine_host::run(spec, Default::default(), None)
            .await
            .unwrap();
        assert_eq!(out.exit_code, Some(0), "{}", out.combined);
    }
    #[cfg(not(unix))]
    {
        write_files(project.path(), &[("made_by_shell.txt", "shell\n")]);
        std::fs::remove_file(project.path().join("sub/c.txt")).unwrap();
    }

    let second = shadow.snapshot("turn 2").await.unwrap();
    assert_eq!(
        shadow.changed_paths(&first, &second).await.unwrap(),
        [
            change("a.txt", ChangeKind::Modified),
            change("b.txt", ChangeKind::Deleted),
            change("made_by_shell.txt", ChangeKind::Added),
            change("sub/c.txt", ChangeKind::Deleted),
            change("sub/new/d.txt", ChangeKind::Added),
        ]
    );

    let report = shadow.restore(&first).await.unwrap();
    assert_eq!(report.restored, ["a.txt", "b.txt", "sub/c.txt"]);
    assert_eq!(report.deleted, ["made_by_shell.txt", "sub/new/d.txt"]);
    let mut expected = before;
    expected.insert("ignored.txt".to_string(), "i1".to_string());
    assert_eq!(
        tree(project.path()),
        expected,
        "ignored files keep their edits"
    );
    assert!(!project.path().join("sub/new").exists());
    assert!(
        !project.path().join(".git").exists(),
        "the project stays a plain directory"
    );

    // Restoring again to the newest snapshot redoes the turn.
    let redo = shadow.restore(&second).await.unwrap();
    assert_eq!(redo.deleted, ["b.txt", "sub/c.txt"]);
    assert_eq!(
        std::fs::read_to_string(project.path().join("a.txt")).unwrap(),
        "a1"
    );
}

#[tokio::test]
async fn identical_paths_are_never_rewritten() {
    if !git_available() {
        return;
    }
    let project = tempfile::tempdir().unwrap();
    write_files(project.path(), &[("keep.txt", "same"), ("edit.txt", "v0")]);
    set_mtime(&project.path().join("keep.txt"), 0);
    let store = tempfile::tempdir().unwrap();
    let shadow = ShadowRepo::open(store.path(), project.path())
        .await
        .unwrap();
    let first = shadow.snapshot("one").await.unwrap();
    write_files(project.path(), &[("edit.txt", "v1")]);
    let keep_mtime = std::fs::metadata(project.path().join("keep.txt"))
        .unwrap()
        .modified()
        .unwrap();

    let report = shadow.restore(&first).await.unwrap();
    assert_eq!(report.restored, ["edit.txt"]);
    assert!(report.deleted.is_empty());
    let after = std::fs::metadata(project.path().join("keep.txt"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(after, keep_mtime);
}

#[tokio::test]
async fn the_users_repository_index_and_refs_are_untouched() {
    if !git_available() {
        return;
    }
    let repo = repo_with(&[
        ("app/src/main.rs", "fn main() {}\n"),
        (".gitignore", "*.log\n"),
    ]);
    write_files(
        repo.path(),
        &[
            ("app/src/main.rs", "fn main() { edited(); }\n"),
            ("app/run.log", "l0"),
        ],
    );
    run_git(repo.path(), &["add", "app/src/main.rs"]);
    let status = run_git(repo.path(), &["status", "--porcelain"]);
    let index = std::fs::read(repo.path().join(".git/index")).unwrap();

    // The project is a subdirectory: the repository-root .gitignore still
    // keeps `*.log` out of checkpoints.
    let store = tempfile::tempdir().unwrap();
    let shadow = ShadowRepo::open(store.path(), &repo.path().join("app"))
        .await
        .unwrap();
    let first = shadow.snapshot("start").await.unwrap();
    write_files(
        repo.path(),
        &[("app/src/main.rs", "broken"), ("app/run.log", "l1")],
    );
    let report = shadow.restore(&first).await.unwrap();
    assert_eq!(report.restored, ["src/main.rs"]);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("app/run.log")).unwrap(),
        "l1"
    );

    // Compare the index before `git status` below may refresh it.
    assert_eq!(
        std::fs::read(repo.path().join(".git/index")).unwrap(),
        index
    );
    assert_eq!(run_git(repo.path(), &["status", "--porcelain"]), status);
    assert!(!run_git(repo.path(), &["for-each-ref"]).contains("zengine"));
}

#[tokio::test]
async fn large_files_are_left_alone_and_unknown_targets_fail() {
    if !git_available() {
        return;
    }
    let project = tempfile::tempdir().unwrap();
    let big = project.path().join("big.bin");
    std::fs::write(&big, vec![b'x'; 9 * 1024 * 1024]).unwrap();
    write_files(project.path(), &[("small.txt", "s0")]);
    let store = tempfile::tempdir().unwrap();
    let shadow = ShadowRepo::open(store.path(), project.path())
        .await
        .unwrap();
    let first = shadow.snapshot("one").await.unwrap();
    // Now small enough to store, but its state at `first` is unknown.
    std::fs::write(&big, b"shrunk").unwrap();
    write_files(project.path(), &[("small.txt", "s1")]);
    let report = shadow.restore(&first).await.unwrap();
    assert_eq!(report.restored, ["small.txt"]);
    assert!(report.deleted.is_empty());
    assert_eq!(report.skipped, ["big.bin"]);
    assert_eq!(std::fs::read(&big).unwrap(), b"shrunk");

    assert!(matches!(
        shadow.restore("0123456789abcdef").await,
        Err(HostError::NotFound(_))
    ));
}

#[tokio::test]
async fn trees_over_the_file_limit_are_refused() {
    if !git_available() {
        return;
    }
    // The production limit is 50,000; creating that many files takes a
    // minute on some machines, so the same check runs with a small limit.
    let project = tempfile::tempdir().unwrap();
    for i in 0..6 {
        write_files(project.path(), &[(&format!("d/f{i}.txt"), "x")]);
    }
    write_files(
        project.path(),
        &[
            (".gitignore", "ignored/\n"),
            ("ignored/a", ""),
            ("ignored/b", ""),
        ],
    );
    let store = tempfile::tempdir().unwrap();
    let within = ShadowRepo::open_with_file_limit(store.path(), project.path(), 7).await;
    assert!(within.is_ok(), "ignored files do not count: {within:?}");
    write_files(project.path(), &[("d/one-too-many.txt", "x")]);
    let refused = within.unwrap().snapshot("more").await;
    assert!(matches!(refused, Err(HostError::Blocked(_))), "{refused:?}");
    let reopened = ShadowRepo::open_with_file_limit(store.path(), project.path(), 7).await;
    assert!(
        matches!(reopened, Err(HostError::Blocked(_))),
        "{reopened:?}"
    );
}
