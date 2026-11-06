//! Read-only repository queries against real temporary repositories.

mod support;

use std::path::Path;

use support::{canonical, git as run_git, init_repo, repo_with, write_files};
use z_engine_host::{
    HostError, changed_files, current_branch, diff, diff_head_file, git, git_available,
    git_with_env, head_sha, is_repo, recent_commits, repo_root, status_porcelain, summary,
};

#[tokio::test]
async fn identity_branch_head_and_history() {
    if !git_available() {
        return;
    }
    let repo = repo_with(&[("a.txt", "one\n")]);
    assert!(is_repo(repo.path()).await);
    let root = repo_root(&repo.path().join("a.txt")).await.unwrap();
    assert_eq!(canonical(&root), canonical(repo.path()));
    let head = head_sha(repo.path()).await.unwrap().unwrap();
    assert_eq!(head.len(), 40);
    assert_eq!(
        current_branch(repo.path()).await.unwrap().as_deref(),
        Some("main")
    );
    let commits = recent_commits(repo.path(), 5).await.unwrap();
    assert_eq!(commits.len(), 1);
    assert!(commits[0].ends_with(" initial"), "{commits:?}");

    let plain = tempfile::tempdir().unwrap();
    assert!(!is_repo(plain.path()).await);
    assert!(repo_root(plain.path()).await.is_none());
    assert!(summary(plain.path()).await.is_none());
}

#[tokio::test]
async fn status_handles_modified_untracked_and_renamed_files() {
    if !git_available() {
        return;
    }
    let repo = repo_with(&[("a.txt", "one\n"), ("b.txt", "bee\n")]);
    write_files(
        repo.path(),
        &[("a.txt", "two\n"), ("new dir/new.txt", "fresh\n")],
    );
    run_git(repo.path(), &["mv", "b.txt", "c.txt"]);

    let entries = status_porcelain(repo.path()).await.unwrap();
    let modified = entries.iter().find(|e| e.path == "a.txt").unwrap();
    assert_eq!((modified.index, modified.worktree), (' ', 'M'));
    let renamed = entries.iter().find(|e| e.path == "c.txt").unwrap();
    assert_eq!(renamed.index, 'R');
    assert_eq!(renamed.orig_path.as_deref(), Some("b.txt"));
    let untracked = entries
        .iter()
        .find(|e| e.path == "new dir/new.txt")
        .unwrap();
    assert_eq!((untracked.index, untracked.worktree), ('?', '?'));

    assert_eq!(
        changed_files(repo.path()).await.unwrap(),
        ["a.txt", "c.txt", "new dir/new.txt"]
    );
}

#[tokio::test]
async fn diffs_cover_worktree_staged_and_untracked_files() {
    if !git_available() {
        return;
    }
    let repo = repo_with(&[("a.txt", "one\n"), ("b.txt", "bee\n")]);
    write_files(
        repo.path(),
        &[
            ("a.txt", "two\n"),
            ("b.txt", "BEE\n"),
            ("new.txt", "hello\n"),
        ],
    );
    run_git(repo.path(), &["add", "b.txt"]);

    let worktree = diff(repo.path(), None, false).await.unwrap();
    assert!(
        worktree.contains("-one") && worktree.contains("+two"),
        "{worktree}"
    );
    assert!(!worktree.contains("BEE"));
    let staged = diff(repo.path(), None, true).await.unwrap();
    assert!(
        staged.contains("+BEE") && !staged.contains("+two"),
        "{staged}"
    );
    let only_a = diff(repo.path(), Some(Path::new("a.txt")), false)
        .await
        .unwrap();
    assert!(only_a.contains("a/a.txt"));

    let tracked = diff_head_file(repo.path(), Path::new("b.txt"))
        .await
        .unwrap();
    assert!(
        tracked.contains("-bee") && tracked.contains("+BEE"),
        "{tracked}"
    );
    let untracked = diff_head_file(repo.path(), Path::new("new.txt"))
        .await
        .unwrap();
    assert!(untracked.contains("new file mode"), "{untracked}");
    assert!(untracked.contains("+hello"));
    assert_eq!(
        diff_head_file(repo.path(), Path::new("absent.txt"))
            .await
            .unwrap(),
        ""
    );
    assert!(matches!(
        diff_head_file(repo.path(), Path::new("../escape.txt")).await,
        Err(HostError::Blocked(_))
    ));
}

#[tokio::test]
async fn summary_reports_branch_head_and_dirty_files() {
    if !git_available() {
        return;
    }
    let repo = repo_with(&[("a.txt", "one\n")]);
    write_files(repo.path(), &[("a.txt", "two\n"), ("b.txt", "new\n")]);
    let summary = summary(repo.path()).await.unwrap();
    assert_eq!(summary.branch.as_deref(), Some("main"));
    assert_eq!(summary.head, head_sha(repo.path()).await.unwrap());
    assert_eq!(summary.dirty_files, 2);
    assert_eq!(summary.status_short, " M a.txt\n?? b.txt");
    assert_eq!(summary.recent_commits.len(), 1);
}

#[tokio::test]
async fn unborn_repositories_have_no_head_and_show_files_as_added() {
    if !git_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    write_files(dir.path(), &[("first.txt", "content\n")]);
    run_git(dir.path(), &["add", "first.txt"]);
    assert_eq!(head_sha(dir.path()).await.unwrap(), None);
    assert!(recent_commits(dir.path(), 3).await.unwrap().is_empty());
    assert_eq!(
        current_branch(dir.path()).await.unwrap().as_deref(),
        Some("main")
    );
    let added = diff_head_file(dir.path(), Path::new("first.txt"))
        .await
        .unwrap();
    assert!(added.contains("+content"), "{added}");
}

#[tokio::test]
async fn failures_are_typed_and_extra_env_reaches_git() {
    if !git_available() {
        return;
    }
    let repo = repo_with(&[("a.txt", "one\n")]);
    match git(repo.path(), &["rev-parse", "--verify", "no-such-ref"]).await {
        Err(HostError::Git { args, stderr }) => {
            assert_eq!(args, ["rev-parse", "--verify", "no-such-ref"]);
            assert!(!stderr.is_empty());
        }
        other => panic!("expected a git error, got {other:?}"),
    }
    let ident = git_with_env(
        repo.path(),
        &["var", "GIT_AUTHOR_IDENT"],
        &[
            ("GIT_AUTHOR_NAME", "Env Person"),
            ("GIT_AUTHOR_EMAIL", "env@example.com"),
        ],
    )
    .await
    .unwrap();
    assert!(ident.starts_with("Env Person <env@example.com>"), "{ident}");
    let missing = git(&repo.path().join("missing"), &["status"]).await;
    assert!(matches!(missing, Err(HostError::NotFound(_))));
}
