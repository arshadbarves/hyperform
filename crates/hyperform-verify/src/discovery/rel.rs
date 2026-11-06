//! Root-relative directory arithmetic on `/`-separated strings. The
//! discovery root is the empty string.

/// `dir/name`, or `name` at the root.
pub(crate) fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// `.` for the root.
pub(crate) fn display(dir: &str) -> &str {
    if dir.is_empty() { "." } else { dir }
}

/// `dir`, then each ancestor up to and including the root.
pub(crate) fn ancestors(dir: &str) -> impl Iterator<Item = &str> {
    let mut next = Some(dir);
    std::iter::from_fn(move || {
        let current = next?;
        next = (!current.is_empty()).then(|| current.rfind('/').map_or("", |i| &current[..i]));
        Some(current)
    })
}

/// Whether `dir` is `ancestor` or lies inside it.
pub(crate) fn within(dir: &str, ancestor: &str) -> bool {
    ancestor.is_empty()
        || dir == ancestor
        || dir
            .strip_prefix(ancestor)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// `dir` relative to `ancestor` (which must contain it).
pub(crate) fn strip<'a>(dir: &'a str, ancestor: &str) -> &'a str {
    if ancestor.is_empty() {
        return dir;
    }
    dir.strip_prefix(ancestor)
        .map_or(dir, |rest| rest.trim_start_matches('/'))
}

/// Whether one of `dirs` strictly contains `dir`.
pub(crate) fn nested_in<'a>(dir: &str, dirs: impl IntoIterator<Item = &'a str>) -> bool {
    dirs.into_iter()
        .any(|other| other != dir && within(dir, other))
}

/// Number of components (0 for the root).
pub(crate) fn depth(dir: &str) -> usize {
    if dir.is_empty() {
        0
    } else {
        dir.split('/').count()
    }
}

/// A manifest-provided relative path in this form: `/`-separated, no `.`
/// components, no leading `./` or trailing `/`.
pub(crate) fn clean(path: &str) -> String {
    path.trim()
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// Component-wise order that puts the root first and a directory right
/// before its children.
pub(crate) fn tree_order(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |dir: &str| -> Vec<String> {
        dir.split('/')
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect()
    };
    parts(a).cmp(&parts(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_ancestors_and_containment() {
        assert_eq!(join("", "Cargo.toml"), "Cargo.toml");
        assert_eq!(join("web", "package.json"), "web/package.json");
        assert_eq!(ancestors("a/b").collect::<Vec<_>>(), ["a/b", "a", ""]);
        assert_eq!(ancestors("").collect::<Vec<_>>(), [""]);
        assert!(within("a/b", "a") && within("a", "a") && within("a", ""));
        assert!(!within("ab", "a"));
        assert_eq!(strip("crates/x", "crates"), "x");
        assert!(nested_in("a/b", ["a", "c"]));
        assert!(!nested_in("a", ["a", "b"]));
        assert_eq!((depth(""), depth("a/b")), (0, 2));
    }

    #[test]
    fn cleaning_and_tree_order() {
        assert_eq!(clean(" ./examples/demo/ "), "examples/demo");
        assert_eq!(clean(r"tools\gen"), "tools/gen");
        let mut dirs = vec!["b", "a-b", "a/b", "", "a"];
        dirs.sort_by(|x, y| tree_order(x, y));
        assert_eq!(dirs, ["", "a", "a/b", "a-b", "b"]);
    }
}
