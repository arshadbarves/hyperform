//! File ranking. A file earns one unit for every reference another file
//! makes to a name it defines; a name defined in several files splits the
//! unit between them. Focus files come first, then files judged relevant
//! to the request, then their neighbours (files exchanging at least one
//! unit of references with focus or relevant files, in either direction),
//! then the rest. Each tier is ordered by score, then path.

use std::collections::HashMap;

use super::symbol::Symbol;

/// Weight of one reference to a name defined in exactly one file. Integer
/// weights keep the ranking independent of summation order.
const UNIT: u64 = 1_000_000;
/// Shorter names are too generic to count as references.
const MIN_IDENT_CHARS: usize = 3;

/// Indices of files with a non-empty outline, best first. `texts`,
/// `outlines`, `focus`, `relevant` and `paths` are parallel slices, one
/// entry per file.
pub(crate) fn rank(
    texts: &[&str],
    outlines: &[Vec<Symbol>],
    focus: &[bool],
    relevant: &[bool],
    paths: &[&str],
) -> Vec<usize> {
    let owners = definitions(outlines);
    let anchor: Vec<bool> = (0..outlines.len())
        .map(|file| focus[file] || relevant[file])
        .collect();
    let mut scores = vec![0u64; outlines.len()];
    let mut exchange = vec![0u64; outlines.len()];
    for (file, text) in texts.iter().enumerate() {
        for word in identifiers(text) {
            let Some(defining) = owners.get(word) else {
                continue;
            };
            let weight = UNIT / defining.len() as u64;
            for &owner in defining.iter().filter(|&&owner| owner != file) {
                scores[owner] = scores[owner].saturating_add(weight);
                if anchor[file] != anchor[owner] {
                    let other = if anchor[file] { owner } else { file };
                    exchange[other] = exchange[other].saturating_add(weight);
                }
            }
        }
    }
    let tier = |file: usize| {
        if focus[file] {
            0
        } else if relevant[file] {
            1
        } else if exchange[file] >= UNIT {
            2
        } else {
            3
        }
    };
    let mut order: Vec<usize> = (0..outlines.len())
        .filter(|&file| !outlines[file].is_empty())
        .collect();
    order.sort_by(|&a, &b| {
        tier(a)
            .cmp(&tier(b))
            .then(scores[b].cmp(&scores[a]))
            .then(paths[a].cmp(paths[b]))
    });
    order
}

/// Name -> indices of the files defining it (each file once, ascending).
fn definitions(outlines: &[Vec<Symbol>]) -> HashMap<&str, Vec<usize>> {
    let mut owners: HashMap<&str, Vec<usize>> = HashMap::new();
    for (file, symbols) in outlines.iter().enumerate() {
        let idents = symbols.iter().filter_map(|symbol| symbol.ident.as_deref());
        for ident in idents.filter(|ident| long_enough(ident)) {
            let files = owners.entry(ident).or_default();
            if files.last() != Some(&file) {
                files.push(file);
            }
        }
    }
    owners
}

fn identifiers(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| long_enough(word) && !word.starts_with(|c: char| c.is_ascii_digit()))
}

fn long_enough(word: &str) -> bool {
    word.chars().nth(MIN_IDENT_CHARS - 1).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(name: &str) -> Symbol {
        Symbol {
            kind: "fn",
            name: name.to_string(),
            ident: Some(name.to_string()),
            line: 1,
            depth: 0,
        }
    }

    fn ranked(files: &[(&str, &str, &[&str])], focus: &[&str]) -> Vec<String> {
        ranked_with(files, focus, &[])
    }

    fn ranked_with(
        files: &[(&str, &str, &[&str])],
        focus: &[&str],
        relevant: &[&str],
    ) -> Vec<String> {
        let paths: Vec<&str> = files.iter().map(|(path, _, _)| *path).collect();
        let texts: Vec<&str> = files.iter().map(|(_, text, _)| *text).collect();
        let outlines: Vec<Vec<Symbol>> = files
            .iter()
            .map(|(_, _, defs)| defs.iter().map(|name| def(name)).collect())
            .collect();
        let focused: Vec<bool> = paths.iter().map(|path| focus.contains(path)).collect();
        let picked: Vec<bool> = paths.iter().map(|path| relevant.contains(path)).collect();
        rank(&texts, &outlines, &focused, &picked, &paths)
            .into_iter()
            .map(|index| paths[index].to_string())
            .collect()
    }

    #[test]
    fn relevant_files_follow_focus_and_pull_their_neighbours_up() {
        let files: &[(&str, &str, &[&str])] = &[
            ("popular.rs", "", &["popular_fn"]),
            ("helper.rs", "", &["helper_fn"]),
            ("asked.rs", "helper_fn()", &["asked_fn"]),
            ("focus.rs", "", &["focus_fn"]),
            ("other.rs", "popular_fn popular_fn", &["other_fn"]),
        ];
        assert_eq!(
            ranked_with(files, &["focus.rs"], &["asked.rs"]),
            [
                "focus.rs",
                "asked.rs",
                "helper.rs",
                "popular.rs",
                "other.rs"
            ]
        );
        assert_eq!(
            ranked_with(files, &["focus.rs"], &[]),
            ranked(files, &["focus.rs"])
        );
    }

    #[test]
    fn referenced_definitions_rank_first_and_files_without_defs_drop_out() {
        let files: &[(&str, &str, &[&str])] = &[
            ("a_unused.rs", "fn unused_thing() {}", &["unused_thing"]),
            ("b_config.rs", "fn parse_config() {}", &["parse_config"]),
            (
                "main.rs",
                "parse_config(); parse_config(); x.parse_config",
                &[],
            ),
        ];
        assert_eq!(ranked(files, &[]), ["b_config.rs", "a_unused.rs"]);
    }

    #[test]
    fn self_references_and_short_names_do_not_count() {
        let files: &[(&str, &str, &[&str])] = &[
            ("a.rs", "run_all run_all run_all", &["run_all"]),
            ("b.rs", "fn go() {} go() go()", &["go", "zzz_b"]),
            ("c.rs", "go(); zzz_b();", &[]),
        ];
        assert_eq!(ranked(files, &[]), ["b.rs", "a.rs"]);
        let files: &[(&str, &str, &[&str])] =
            &[("a.rs", "", &["run_all"]), ("b.rs", "go go go go", &["go"])];
        assert_eq!(ranked(files, &[]), ["a.rs", "b.rs"], "ties sort by path");
    }

    #[test]
    fn names_defined_twice_split_their_references() {
        let files: &[(&str, &str, &[&str])] = &[
            ("dup1.rs", "", &["shared_name"]),
            ("dup2.rs", "", &["shared_name"]),
            ("solo.rs", "", &["solo_name"]),
            (
                "user.rs",
                "shared_name shared_name solo_name solo_name",
                &[],
            ),
        ];
        // solo.rs: 2 units; dup1/dup2: 1 unit each.
        assert_eq!(ranked(files, &[]), ["solo.rs", "dup1.rs", "dup2.rs"]);
    }

    #[test]
    fn focus_files_lead_and_their_neighbours_follow() {
        let files: &[(&str, &str, &[&str])] = &[
            ("popular.rs", "", &["popular_fn"]),
            ("helper.rs", "", &["helper_fn"]),
            ("caller.rs", "focus_fn()", &["caller_fn"]),
            ("focus.rs", "helper_fn()", &["focus_fn"]),
            (
                "other.rs",
                "popular_fn popular_fn popular_fn",
                &["other_fn"],
            ),
        ];
        // Neighbours: helper.rs (referenced by focus.rs, 1 unit) and
        // caller.rs (references focus.rs, 0 units of its own).
        assert_eq!(
            ranked(files, &["focus.rs"]),
            [
                "focus.rs",
                "helper.rs",
                "caller.rs",
                "popular.rs",
                "other.rs"
            ]
        );
    }
}
