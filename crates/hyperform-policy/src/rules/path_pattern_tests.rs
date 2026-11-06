use std::path::{Path, PathBuf};

use super::*;

fn ctx() -> PolicyContext {
    PolicyContext {
        project_root: PathBuf::from("/work/proj"),
        additional_dirs: Vec::new(),
        home: Some(PathBuf::from("/home/me")),
    }
}

fn matches(pattern: &str, path: &str) -> bool {
    PathPattern::parse(pattern)
        .unwrap()
        .matches(Path::new(path), &ctx())
}

#[test]
fn relative_patterns_are_anchored_at_the_project_root() {
    assert!(matches("src/**", "/work/proj/src/main.rs"));
    assert!(matches("src/**", "/work/proj/src/a/b/c.rs"));
    assert!(!matches("src/**", "/work/proj/tests/src/x.rs"));
    assert!(!matches("src/**", "/elsewhere/src/main.rs"));
    assert!(matches("./docs/*.md", "/work/proj/docs/guide.md"));
    assert!(matches("src/*.rs", "/work/proj/src/lib.rs"));
    assert!(!matches("src/*.rs", "/work/proj/src/deep/lib.rs"));
}

#[test]
fn patterns_without_a_slash_match_at_any_depth() {
    assert!(matches(".env", "/work/proj/.env"));
    assert!(matches(".env", "/work/proj/packages/api/.env"));
    assert!(matches("*.pem", "/work/proj/certs/server.pem"));
    assert!(!matches(".env", "/work/proj/.env.example"));
    assert!(!matches(".env", "/other/.env"));
}

#[test]
fn directories_cover_their_contents() {
    assert!(matches("secrets", "/work/proj/secrets"));
    assert!(matches("secrets", "/work/proj/secrets/api/key.txt"));
    assert!(matches("config/prod", "/work/proj/config/prod/db.yml"));
    assert!(matches("build/", "/work/proj/build/out.o"));
    assert!(matches("src/*", "/work/proj/src/nested/deep.rs"));
    assert!(!matches("secrets", "/work/proj/secretsx"));
    assert!(matches("secrets/**", "/work/proj/secrets"));
    assert!(matches("~/.ssh/**", "/home/me/.ssh"));
    assert!(matches("**/node_modules/**", "/work/proj/web/node_modules"));
    assert!(!matches("secrets/**", "/work/proj"));
}

#[test]
fn home_and_absolute_anchors() {
    assert!(matches("~/.ssh/**", "/home/me/.ssh/id_ed25519"));
    assert!(matches("~/.ssh", "/home/me/.ssh/config"));
    assert!(!matches("~/.ssh/**", "/work/proj/.ssh/x"));
    assert!(matches("~", "/home/me/anything"));
    assert!(matches("//etc/**", "/etc/hosts"));
    assert!(matches("//etc", "/etc/ssh/sshd_config"));
    assert!(!matches("//etc/**", "/work/proj/etc/hosts"));
    assert!(matches("//work/proj/src/**", "/work/proj/src/a.rs"));
}

#[test]
fn single_slash_is_tried_as_absolute_and_project_relative() {
    assert!(matches("/etc/**", "/etc/passwd"));
    assert!(matches("/src/**", "/work/proj/src/main.rs"));
    assert!(matches("/Users/me/other/**", "/Users/me/other/file"));
    assert!(!matches("/src/**", "/work/proj/lib/src/x.rs"));
}

#[test]
fn everything_patterns() {
    assert!(matches("**", "/work/proj/any/thing"));
    assert!(matches("**", "/work/proj"));
    assert!(matches(".", "/work/proj/x"));
    assert!(!matches("**", "/elsewhere"));
}

#[test]
fn paths_are_normalized_before_matching() {
    assert!(matches("src/**", "/work/proj/tests/../src/main.rs"));
    assert!(!matches("src/**", "/work/proj/src/../../proj2/src/main.rs"));
    assert!(matches("src/**", "src/relative.rs"));
}

#[test]
fn home_patterns_need_a_home() {
    let mut context = ctx();
    context.home = None;
    let pattern = PathPattern::parse("~/.ssh/**").unwrap();
    assert!(!pattern.matches(Path::new("/home/me/.ssh/id"), &context));
}

#[test]
fn invalid_patterns() {
    assert!(matches!(
        PathPattern::parse("!src/**"),
        Err(PatternError::Invalid(_))
    ));
    assert!(matches!(
        PathPattern::parse("~other/x"),
        Err(PatternError::Invalid(_))
    ));
    assert!(matches!(
        PathPattern::parse("src/[unclosed"),
        Err(PatternError::Glob(_))
    ));
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[test]
fn case_insensitive_filesystems_ignore_case() {
    assert!(matches(".env", "/work/proj/.ENV"));
}

#[test]
fn display_keeps_the_source() {
    let pattern = PathPattern::parse(" ~/.ssh/** ").unwrap();
    assert_eq!(pattern.to_string(), "~/.ssh/**");
}
