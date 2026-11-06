//! Budgeted rendering of ranked outlines.

use super::symbol::Symbol;

/// `path:` then one indented `kind name (L<line>)` line per definition,
/// for the files in `order`. When everything does not fit, files are added
/// best-first while they fit and a closing line counts the files left out.
/// The result never exceeds `budget_chars` characters and is empty when no
/// file fits.
pub(crate) fn render(
    paths: &[&str],
    outlines: &[Vec<Symbol>],
    order: &[usize],
    budget_chars: usize,
) -> String {
    let blocks: Vec<String> = order
        .iter()
        .map(|&file| block(paths[file], &outlines[file]))
        .collect();
    let sizes: Vec<usize> = blocks.iter().map(|block| block.chars().count()).collect();
    let everything = sizes.iter().sum::<usize>() + blocks.len().saturating_sub(1);
    if everything <= budget_chars {
        return blocks.join("\n");
    }
    // Room for the newline and the longest possible omission note.
    let reserve = omitted_note(blocks.len()).chars().count() + 1;
    let mut kept: Vec<&str> = Vec::new();
    let mut used = 0;
    for (block, size) in blocks.iter().zip(sizes) {
        let needed = size + usize::from(!kept.is_empty());
        if used + needed + reserve <= budget_chars {
            kept.push(block);
            used += needed;
        }
    }
    if kept.is_empty() {
        return String::new();
    }
    format!(
        "{}\n{}",
        kept.join("\n"),
        omitted_note(blocks.len() - kept.len())
    )
}

fn block(path: &str, symbols: &[Symbol]) -> String {
    let mut out = format!("{path}:");
    for symbol in symbols {
        out.push('\n');
        out.push_str(&"  ".repeat(symbol.depth + 1));
        out.push_str(&format!(
            "{} {} (L{})",
            symbol.kind, symbol.name, symbol.line
        ));
    }
    out
}

fn omitted_note(count: usize) -> String {
    let noun = if count == 1 { "file" } else { "files" };
    format!("({count} more {noun} omitted)")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol(kind: &'static str, name: &str, line: usize, depth: usize) -> Symbol {
        Symbol {
            kind,
            name: name.to_string(),
            ident: None,
            line,
            depth,
        }
    }

    #[test]
    fn renders_paths_with_indented_definitions() {
        let outlines = vec![
            vec![symbol("impl", "Zebra", 3, 0), symbol("fn", "new", 4, 1)],
            vec![symbol("def", "main", 1, 0)],
        ];
        let out = render(&["src/zebra.rs", "app.py"], &outlines, &[1, 0], 1_000);
        assert_eq!(
            out,
            "app.py:\n  def main (L1)\nsrc/zebra.rs:\n  impl Zebra (L3)\n    fn new (L4)"
        );
    }

    #[test]
    fn drops_lowest_ranked_files_and_counts_them() {
        let paths: Vec<String> = (0..10).map(|i| format!("src/file_{i}.rs")).collect();
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        let outlines: Vec<Vec<Symbol>> = (0..10)
            .map(|i| vec![symbol("fn", &format!("function_{i}"), i + 1, 0)])
            .collect();
        let order: Vec<usize> = (0..10).collect();
        let full = render(&paths, &outlines, &order, usize::MAX);
        assert!(!full.contains("omitted"));
        let budget = full.chars().count() / 2;
        let out = render(&paths, &outlines, &order, budget);
        assert!(out.chars().count() <= budget, "{out}");
        assert!(out.starts_with("src/file_0.rs:"), "{out}");
        let shown = out.matches(".rs:").count();
        assert!((1..10).contains(&shown), "{out}");
        assert!(
            out.ends_with(&format!("({} more files omitted)", 10 - shown)),
            "{out}"
        );
        assert!(!out.contains(&format!("file_{}.rs", shown)), "{out}");
    }

    #[test]
    fn empty_when_nothing_fits() {
        let outlines = vec![vec![symbol("fn", "long_function_name", 1, 0)]];
        assert_eq!(render(&["a.rs"], &outlines, &[0], 10), "");
        assert_eq!(render(&[], &[], &[], 0), "");
    }
}
