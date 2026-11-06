//! Worktree lifecycle and applying an agent's patch back to the user tree.

mod support;

use std::path::Path;

use support::{git as run_git, repo_with, write_files};
use z_engine_host::{
    HostError, WorktreeHandle, apply_patch, commit_all, create_worktree, diff_range,
    diffstat_range, git_available, head_sha, prune_worktrees, remove_worktree, status_porcelain,
};

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

async fn worktree(repo: &Path, name: &str) -> WorktreeHandle {
    let path = repo.join(".z-engine/worktrees").join(name);
    create_worktree(repo, &path, &format!("zengine/{name}"), None)
        .await
        .unwrap()
}

/// Commits `files` in a fresh worktree and returns the patch from its base.
async fn agent_patch(repo: &Path, name: &str, files: &[(&str, &str)]) -> (WorktreeHandle, String) {
    let handle = worktree(repo, name).await;
    write_files(&handle.path, files);
    let head = commit_all(&handle.path, "agent work")
        .await
        .unwrap()
        .unwrap();
    let patch = diff_range(repo, &handle.base_sha, &head).await.unwrap();
    (handle, patch)
}

#[tokio::test]
async fn worktree_lifecycle_from_create_to_remove() {
    if !git_available() {
        return;
    }
    let repo = repo_with(&[("a.txt", "line1\nline2\n")]);
    let handle = worktree(repo.path(), "agent1").await;
    assert!(handle.path.join("a.txt").is_file());
    assert_eq!(handle.branch, "zengine/agent1");
    assert_eq!(
        Some(handle.base_sha.clone()),
        head_sha(repo.path()).await.unwrap()
    );
    assert!(
        status_porcelain(repo.path()).await.unwrap().is_empty(),
        "nested worktree is excluded"
    );

    let other = repo.path().join(".z-engine/worktrees/other");
    let duplicate = create_worktree(repo.path(), &other, "zengine/agent1", None).await;
    assert!(matches!(duplicate, Err(HostError::Invalid(_))));
    let invalid = create_worktree(repo.path(), &other, "bad..name", None).await;
    assert!(matches!(invalid, Err(HostError::Invalid(_))));

    assert_eq!(commit_all(&handle.path, "nothing").await.unwrap(), None);
    write_files(
        &handle.path,
        &[("a.txt", "line1\nchanged\n"), ("new.txt", "hello\n")],
    );
    let head = commit_all(&handle.path, "agent work")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(head.len(), 40);

    let patch = diff_range(repo.path(), &handle.base_sha, &head)
        .await
        .unwrap();
    assert!(
        patch.contains("b/new.txt") && patch.contains("+changed"),
        "{patch}"
    );
    let (files, stat) = diffstat_range(repo.path(), &handle.base_sha, &head)
        .await
        .unwrap();
    assert_eq!(files, 2);
    assert!(stat.contains("2 files changed"), "{stat}");

    remove_worktree(repo.path(), &handle.path, true)
        .await
        .unwrap();
    assert!(!handle.path.exists());
    let branches = run_git(repo.path(), &["branch", "--list", "zengine/*"]);
    assert!(branches.trim().is_empty(), "{branches}");
    prune_worktrees(repo.path()).await.unwrap();
}

#[tokio::test]
async fn clean_patches_apply_to_the_work_tree_only() {
    if !git_available() {
        return;
    }
    let repo = repo_with(&[("a.txt", "line1\nline2\n")]);
    let files = [("a.txt", "line1\nchanged\n"), ("new.txt", "hello\n")];
    let (handle, patch) = agent_patch(repo.path(), "clean", &files).await;
    let outcome = apply_patch(repo.path(), &patch).await.unwrap();
    assert!(outcome.applied, "{outcome:?}");
    assert!(outcome.conflicts.is_empty());
    assert_eq!(read(&repo.path().join("a.txt")), "line1\nchanged\n");
    assert_eq!(read(&repo.path().join("new.txt")), "hello\n");
    assert_eq!(
        run_git(repo.path(), &["diff", "--cached", "--name-only"]),
        ""
    );
    remove_worktree(repo.path(), &handle.path, true)
        .await
        .unwrap();
}

#[tokio::test]
async fn diverged_trees_merge_three_way_without_touching_the_index() {
    if !git_available() {
        return;
    }
    let base: String = (1..=10).map(|i| format!("L{i}\n")).collect();
    let repo = repo_with(&[("f.txt", &base)]);
    let theirs = base.replace("L8\n", "WT8\n");
    let (_handle, patch) = agent_patch(repo.path(), "merge", &[("f.txt", &theirs)]).await;

    // The user edits a nearby line without committing: context no longer
    // matches, but a 3-way merge is clean.
    write_files(repo.path(), &[("f.txt", &base.replace("L6\n", "MAIN6\n"))]);
    let outcome = apply_patch(repo.path(), &patch).await.unwrap();
    assert!(outcome.applied, "{outcome:?}");
    let merged = read(&repo.path().join("f.txt"));
    assert!(
        merged.contains("MAIN6\n") && merged.contains("WT8\n"),
        "{merged}"
    );
    assert_eq!(
        run_git(repo.path(), &["diff", "--cached", "--name-only"]),
        ""
    );
}

#[tokio::test]
async fn conflicting_patches_leave_the_tree_and_index_unchanged() {
    if !git_available() {
        return;
    }
    let repo = repo_with(&[("f.txt", "a\nb\nc\n"), ("g.txt", "g\n")]);
    let files = [
        ("f.txt", "a\nWORKTREE\nc\n"),
        ("g.txt", "g2\n"),
        ("h.txt", "new\n"),
    ];
    let (_handle, patch) = agent_patch(repo.path(), "conflict", &files).await;

    write_files(repo.path(), &[("f.txt", "a\nMAIN\nc\n")]);
    let status_before = run_git(repo.path(), &["status", "--porcelain"]);
    let outcome = apply_patch(repo.path(), &patch).await.unwrap();
    assert!(!outcome.applied, "{outcome:?}");
    assert_eq!(outcome.conflicts, ["f.txt"]);
    assert!(!outcome.message.is_empty());
    assert_eq!(read(&repo.path().join("f.txt")), "a\nMAIN\nc\n");
    assert_eq!(read(&repo.path().join("g.txt")), "g\n");
    assert!(!repo.path().join("h.txt").exists());
    assert_eq!(
        run_git(repo.path(), &["status", "--porcelain"]),
        status_before
    );
}

#[tokio::test]
async fn empty_and_malformed_patches() {
    if !git_available() {
        return;
    }
    let repo = repo_with(&[("a.txt", "one\n")]);
    assert!(apply_patch(repo.path(), "  \n").await.unwrap().applied);
    let garbage = apply_patch(repo.path(), "this is not a patch\n")
        .await
        .unwrap();
    assert!(!garbage.applied);
    assert_eq!(read(&repo.path().join("a.txt")), "one\n");
}
