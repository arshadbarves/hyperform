//! Path containment against `..`, symlinks, and missing components.

use std::path::Path;

use z_engine_host::{is_within, relative_display};

#[test]
fn nested_existing_and_missing_paths_are_within() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    assert!(is_within(root.path(), root.path()));
    assert!(is_within(root.path(), Path::new("src")));
    assert!(is_within(root.path(), Path::new("src/new/deeper/file.rs")));
    assert!(is_within(
        root.path(),
        &root.path().join("src/../README.md")
    ));
}

#[test]
fn dotdot_escapes_are_not_within() {
    let root = tempfile::tempdir().unwrap();
    assert!(!is_within(root.path(), Path::new("../outside.txt")));
    assert!(!is_within(
        root.path(),
        Path::new("missing/../../outside.txt")
    ));
    assert!(!is_within(root.path(), Path::new("/etc/passwd")));
}

#[cfg(unix)]
#[test]
fn symlinks_resolve_the_way_the_os_follows_them() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(outside.path().join("sub")).unwrap();
    std::fs::create_dir(root.path().join("inner")).unwrap();

    symlink(outside.path(), root.path().join("escape")).unwrap();
    symlink(root.path().join("inner"), root.path().join("alias")).unwrap();
    symlink(outside.path().join("sub"), root.path().join("deep")).unwrap();
    symlink(outside.path().join("missing"), root.path().join("dangling")).unwrap();

    assert!(!is_within(root.path(), Path::new("escape/file.txt")));
    assert!(is_within(root.path(), Path::new("alias/file.txt")));
    // `..` after a symlink climbs from the link target, not the link.
    assert!(!is_within(root.path(), Path::new("deep/../x.txt")));
    assert!(!is_within(root.path(), Path::new("dangling")));
    assert!(!is_within(root.path(), Path::new("dangling/child.txt")));
}

#[test]
fn relative_display_shortens_paths_inside_the_root() {
    let root = tempfile::tempdir().unwrap();
    let inside = root.path().join("src").join("a.rs");
    assert_eq!(
        relative_display(root.path(), &inside),
        Path::new("src").join("a.rs").display().to_string()
    );
    assert_eq!(relative_display(root.path(), root.path()), ".");
    let outside = Path::new("/definitely/elsewhere.txt");
    assert_eq!(
        relative_display(root.path(), outside),
        outside.display().to_string()
    );
}
