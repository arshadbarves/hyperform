//! Instruction files are found in precedence order, bounded by the
//! repository root or the home directory, and nested lookups stay below
//! the project root.

#[allow(dead_code)]
mod support;

use std::path::{Path, PathBuf};

use support::{Fixture, fixture, write};
use z_engine_config::instructions::TRUNCATION_NOTE;
use z_engine_config::{
    InstructionFile, InstructionScope, discover_instructions, nested_instructions,
};

fn listed(files: &[InstructionFile], base: &Path) -> Vec<(InstructionScope, String)> {
    files
        .iter()
        .map(|file| {
            let path = Path::new(&file.path);
            let shown = path
                .strip_prefix(base)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
            (file.scope, shown)
        })
        .collect()
}

/// A monorepo `repo/` (with `.git`) whose project root is `repo/packages/app`.
fn monorepo(f: &mut Fixture) -> PathBuf {
    let home = f.tmp.path().join("home");
    f.paths.home_dir = Some(home.clone());
    let repo = f.tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let app = repo.join("packages/app");
    write(&home.join(".claude/CLAUDE.md"), "claude user");
    write(&f.paths.config_dir.join("AGENTS.md"), "user");
    write(&repo.join("AGENTS.md"), "repo");
    write(&repo.join("CLAUDE.md"), "repo claude");
    write(&repo.join("packages/AGENTS.md"), "   \n");
    write(&app.join("AGENTS.md"), "app");
    write(&app.join("AGENTS.local.md"), "mine");
    write(&app.join("CLAUDE.local.md"), "mine claude");
    app
}

#[test]
fn files_are_listed_lowest_precedence_first() {
    let mut f = fixture();
    let app = monorepo(&mut f);
    let files = discover_instructions(&f.paths, &app, true);
    use InstructionScope::{Local, Project, User};
    assert_eq!(
        listed(&files, f.tmp.path()),
        [
            (User, "home/.claude/CLAUDE.md".to_string()),
            (User, "config/AGENTS.md".into()),
            (Project, "repo/CLAUDE.md".into()),
            (Project, "repo/AGENTS.md".into()),
            (Project, "repo/packages/app/AGENTS.md".into()),
            (Local, "repo/packages/app/CLAUDE.local.md".into()),
            (Local, "repo/packages/app/AGENTS.local.md".into()),
        ]
    );
    assert_eq!(files[3].content, "repo");

    let native = discover_instructions(&f.paths, &app, false);
    let names: Vec<_> = listed(&native, f.tmp.path())
        .into_iter()
        .map(|(_, path)| path)
        .collect();
    assert_eq!(
        names,
        [
            "config/AGENTS.md",
            "repo/AGENTS.md",
            "repo/packages/app/AGENTS.md",
            "repo/packages/app/AGENTS.local.md"
        ]
    );
}

#[test]
fn without_git_the_walk_stops_below_home() {
    let mut f = fixture();
    let home = f.tmp.path().join("home");
    f.paths.home_dir = Some(home.clone());
    let project = home.join("code/tool");
    write(&home.join("AGENTS.md"), "home");
    write(&home.join("code/AGENTS.md"), "code");
    write(&project.join("AGENTS.md"), "tool");
    let files = discover_instructions(&f.paths, &project, false);
    let contents: Vec<_> = files.iter().map(|file| file.content.as_str()).collect();
    assert_eq!(contents, ["code", "tool"]);
}

#[test]
fn large_files_are_truncated_with_a_note() {
    let f = fixture();
    write(&f.project.join("AGENTS.md"), &"é".repeat(40 * 1024));
    let files = discover_instructions(&f.paths, &f.project, false);
    let content = &files[0].content;
    assert!(content.ends_with(TRUNCATION_NOTE));
    assert!(content.len() <= 64 * 1024 + TRUNCATION_NOTE.len());
}

#[cfg(unix)]
#[test]
fn a_symlinked_claude_file_is_listed_once() {
    let f = fixture();
    write(&f.project.join("AGENTS.md"), "shared");
    std::os::unix::fs::symlink("AGENTS.md", f.project.join("CLAUDE.md")).unwrap();
    let outside = f.tmp.path().join("secret.txt");
    write(&outside, "secret");
    std::os::unix::fs::symlink(&outside, f.project.join("AGENTS.local.md")).unwrap();
    let files = discover_instructions(&f.paths, &f.project, true);
    assert_eq!(files.len(), 1, "{files:?}");
    assert!(files[0].path.ends_with("AGENTS.md"));
}

#[test]
fn nested_files_lie_strictly_between_the_root_and_the_file() {
    let f = fixture();
    let root = &f.project;
    write(&root.join("AGENTS.md"), "root");
    write(&root.join("src/AGENTS.md"), "src");
    write(&root.join("src/a/CLAUDE.md"), "a claude");
    write(&root.join("src/a/b/AGENTS.md"), "b");
    write(&root.join("src/a/b/c/AGENTS.md"), "below the file");
    let file = root.join("src/a/b/lib.rs");
    let found: Vec<_> = nested_instructions(root, &file, true)
        .into_iter()
        .map(|f| f.content)
        .collect();
    assert_eq!(found, ["src", "a claude", "b"]);
    let relative: Vec<_> = nested_instructions(root, Path::new("src/a/b/lib.rs"), false)
        .into_iter()
        .map(|f| (f.scope, f.content))
        .collect();
    assert_eq!(
        relative,
        [
            (InstructionScope::Nested, "src".into()),
            (InstructionScope::Nested, "b".into())
        ]
    );
    assert!(nested_instructions(root, &f.tmp.path().join("elsewhere/x.rs"), true).is_empty());
    assert!(nested_instructions(root, &root.join("../project/src/x.rs"), true).is_empty());
    assert!(nested_instructions(root, &root.join("top.rs"), true).is_empty());
}
