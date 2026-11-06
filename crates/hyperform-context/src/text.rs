//! Whitespace shaping for text placed on a single rendered line.

/// Collapses every whitespace run, including newlines, to one space.
pub(crate) fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_newlines_and_runs() {
        assert_eq!(single_line("  a\n\tb   c \n"), "a b c");
    }
}
