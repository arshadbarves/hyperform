//! Repo map through the public API: every language, focus ranking, budget
//! truncation and skipped files.

use z_engine_context::{SourceFile, repo_map, supported_extension};

fn file(path: &str, text: &str) -> SourceFile {
    SourceFile {
        path: path.into(),
        text: text.into(),
    }
}

fn position(map: &str, path: &str) -> usize {
    map.find(&format!("{path}:"))
        .unwrap_or_else(|| panic!("{path} missing from:\n{map}"))
}

#[test]
fn outlines_every_supported_language() {
    let files = vec![
        file(
            "src/lib.rs",
            "pub struct Config;\nimpl Config {\n    pub fn load() -> Self { Config }\n}\n",
        ),
        file(
            "web/api.ts",
            "export interface Api {\n  get(): void;\n}\nexport function createApi(): Api { return api; }\n",
        ),
        file("web/App.tsx", "export const App = () => <main />;\n"),
        file("web/util.mjs", "export function debounce(fn) {}\n"),
        file(
            "tools/build.py",
            "class Builder:\n    def run(self):\n        pass\n",
        ),
        file("cmd/main.go", "package main\n\nfunc main() {}\n"),
        file("README.md", "Use Config and createApi.\n"),
    ];
    let map = repo_map(&files, &[], 10_000);
    for expected in [
        "src/lib.rs:\n  struct Config (L1)\n  impl Config (L2)\n    fn load (L3)",
        "web/api.ts:\n  interface Api (L1)\n    method get (L2)\n  function createApi (L4)",
        "web/App.tsx:\n  function App (L1)",
        "web/util.mjs:\n  function debounce (L1)",
        "tools/build.py:\n  class Builder (L1)\n    def run (L2)",
        "cmd/main.go:\n  func main (L3)",
    ] {
        assert!(map.contains(expected), "missing {expected:?} in:\n{map}");
    }
    assert!(!map.contains("README"), "{map}");
    assert!(!map.contains("omitted"), "{map}");
    assert!(supported_extension("tsx") && !supported_extension("md"));
}

#[test]
fn focus_files_and_their_neighbours_rank_first() {
    let files = vec![
        file("src/popular.rs", "pub fn popular_helper() {}\n"),
        file(
            "src/user_a.rs",
            "fn user_a() { popular_helper(); popular_helper(); }\n",
        ),
        file("src/user_b.rs", "fn user_b() { popular_helper(); }\n"),
        file(
            "src/focus.rs",
            "pub fn focused_entry() { neighbour_fn(); }\n",
        ),
        file("src/neighbour.rs", "pub fn neighbour_fn() {}\n"),
    ];
    let unfocused = repo_map(&files, &[], 10_000);
    assert!(unfocused.starts_with("src/popular.rs:"), "{unfocused}");

    let focus = vec!["/work/app/src/focus.rs".to_string()];
    let map = repo_map(&files, &focus, 10_000);
    assert!(map.starts_with("src/focus.rs:"), "{map}");
    assert!(position(&map, "src/neighbour.rs") < position(&map, "src/popular.rs"));
    assert!(position(&map, "src/popular.rs") < position(&map, "src/user_a.rs"));
}

#[test]
fn over_budget_maps_drop_the_lowest_ranked_files() {
    let mut files: Vec<SourceFile> = (0..20)
        .map(|i| {
            file(
                &format!("src/m{i:02}.rs"),
                &format!("pub fn item_{i:02}() {{}}\npub struct Kind{i:02};\n"),
            )
        })
        .collect();
    // item_00 is referenced 20 times, item_19 once.
    let calls: String = (0..20)
        .map(|i| format!("item_{i:02}();\n").repeat(20 - i))
        .collect();
    files.push(file(
        "src/caller.rs",
        &format!("fn caller_main() {{\n{calls}}}\n"),
    ));

    let full = repo_map(&files, &[], usize::MAX);
    assert_eq!(full.matches(".rs:").count(), 21);
    let budget = full.chars().count() / 3;
    let map = repo_map(&files, &[], budget);
    assert!(map.chars().count() <= budget, "{map}");
    assert!(map.starts_with("src/m00.rs:"), "{map}");
    let shown = map.matches(".rs:").count();
    assert!(
        map.ends_with(&format!("({} more files omitted)", 21 - shown)),
        "{map}"
    );
    for i in 0..shown {
        assert!(map.contains(&format!("src/m{i:02}.rs:")), "{map}");
    }
    assert!(
        !map.contains("src/m19.rs") && !map.contains("caller.rs"),
        "{map}"
    );
    assert_eq!(repo_map(&files, &[], 5), "");
}

#[test]
fn unparseable_oversized_and_unsupported_files_are_skipped() {
    let files = vec![
        file("src/broken.rs", "\u{0}\u{1}\u{2} }}}} ((( ;;"),
        file("src/good.rs", "pub fn fine_fn() {}\n"),
        file("notes.txt", "fn not_code() {}\n"),
        file("gen/huge.rs", &"pub fn generated() {}\n".repeat(50_000)),
    ];
    assert_eq!(
        repo_map(&files, &[], 10_000),
        "src/good.rs:\n  fn fine_fn (L1)"
    );
    assert_eq!(repo_map(&[], &[], 10_000), "");
}

#[test]
fn local_syntax_errors_keep_recovered_definitions() {
    let files = vec![file(
        "src/editing.rs",
        "pub fn intact() {}\n\nfn half_written( {\n",
    )];
    let map = repo_map(&files, &[], 10_000);
    assert!(
        map.starts_with("src/editing.rs:\n  fn intact (L1)"),
        "{map}"
    );
}
