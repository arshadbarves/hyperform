//! `cat -n` numbering: the line number right-aligned in six columns, a tab,
//! then the line. Over-long lines are cut with a marker.

/// Appends one numbered line (with a trailing newline) to `out`.
pub(crate) fn push_numbered(out: &mut String, number: usize, line: &str, max_chars: usize) {
    out.push_str(&format!("{number:>6}\t"));
    match cut_line(line, max_chars) {
        Some((kept, omitted)) => {
            out.push_str(kept);
            out.push_str(&format!("... [line truncated: {omitted} more characters]"));
        }
        None => out.push_str(line),
    }
    out.push('\n');
}

/// Numbers `lines`, the first being line `first` (1-based).
pub(crate) fn numbered<'a>(
    lines: impl IntoIterator<Item = &'a str>,
    first: usize,
    max_chars: usize,
) -> String {
    let mut out = String::new();
    for (index, line) in lines.into_iter().enumerate() {
        push_numbered(&mut out, first + index, line, max_chars);
    }
    out
}

/// The kept prefix and the count of omitted characters, when `line` has
/// more than `max_chars` characters.
fn cut_line(line: &str, max_chars: usize) -> Option<(&str, usize)> {
    let (index, _) = line.char_indices().nth(max_chars)?;
    Some((&line[..index], line[index..].chars().count()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_right_aligned_and_tab_separated() {
        assert_eq!(numbered(["a", "b"], 9, 100), "     9\ta\n    10\tb\n");
    }

    #[test]
    fn long_lines_are_cut_with_a_marker() {
        let mut out = String::new();
        push_numbered(&mut out, 1, "abcdefé", 3);
        assert_eq!(out, "     1\tabc... [line truncated: 4 more characters]\n");
    }
}
