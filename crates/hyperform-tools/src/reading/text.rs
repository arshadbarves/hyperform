//! The text view of `Read`: a window of `cat -n` numbered lines, with a note
//! when more lines follow or the output budget ended the window early.

use crate::text::push_numbered;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Window {
    pub(crate) body: String,
    pub(crate) first: usize,
    pub(crate) last: usize,
    pub(crate) total: usize,
}

impl Window {
    pub(crate) fn summary(&self) -> String {
        if self.first == 1 && self.last == self.total {
            format!("Read {} lines", self.total)
        } else {
            format!("Read lines {}-{} of {}", self.first, self.last, self.total)
        }
    }
}

/// Up to `limit` lines from line `offset` (1-based), cut early when the
/// numbered text would exceed `max_chars`. At least one line is shown.
pub(crate) fn window(
    content: &str,
    offset: usize,
    limit: usize,
    max_line_chars: usize,
    max_chars: usize,
) -> Result<Window, String> {
    let total = content.lines().count();
    if offset > total {
        return Err(format!(
            "offset {offset} is past the end of the file, which has {total} lines"
        ));
    }
    let mut body = String::new();
    let mut chars = 0;
    let mut last = offset - 1;
    let mut budget_hit = false;
    for (index, line) in content.lines().enumerate().skip(offset - 1).take(limit) {
        let before = body.len();
        push_numbered(&mut body, index + 1, line, max_line_chars);
        chars += body[before..].chars().count();
        if chars > max_chars && last >= offset {
            body.truncate(before);
            budget_hit = true;
            break;
        }
        last = index + 1;
    }
    if budget_hit {
        body.push_str(&format!(
            "\n(Output limited to about {max_chars} characters: showing lines {offset}-{last} of {total}. Continue with offset={}.)\n",
            last + 1
        ));
    } else if last < total {
        body.push_str(&format!(
            "\n(Showing lines {offset}-{last} of {total}. Continue with offset={} to read more.)\n",
            last + 1
        ));
    }
    Ok(Window {
        body,
        first: offset,
        last,
        total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: usize) -> String {
        (1..=lines).map(|i| format!("line{i}\n")).collect()
    }

    #[test]
    fn windows_number_lines_and_point_at_the_rest() {
        let view = window(&text(10), 4, 3, 100, 10_000).unwrap();
        assert!(
            view.body
                .starts_with("     4\tline4\n     5\tline5\n     6\tline6\n")
        );
        assert!(view.body.contains("Continue with offset=7"));
        assert_eq!(view.summary(), "Read lines 4-6 of 10");
        let whole = window(&text(3), 1, 100, 100, 10_000).unwrap();
        assert_eq!(whole.summary(), "Read 3 lines");
        assert!(!whole.body.contains("Continue"));
    }

    #[test]
    fn budget_and_offset_limits() {
        let view = window(&text(1_000), 1, 1_000, 100, 200).unwrap();
        assert!(view.last < 1_000 && view.last > 1);
        assert!(view.body.contains("Output limited"));
        let err = window(&text(2), 5, 10, 100, 1_000).unwrap_err();
        assert!(err.contains("past the end"), "{err}");
    }
}
