//! Character-level word classification.

use super::super::syntax::Word;

/// Adds an unquoted, unescaped character to `word`, recording the expansion
/// it may trigger. `brace_at` remembers the last unquoted `{`.
pub(super) fn plain(word: &mut Word, c: char, started: bool, brace_at: &mut Option<usize>) {
    match c {
        '*' | '?' | '[' => word.glob = true,
        '~' if !started => word.tilde = true,
        '{' => *brace_at = Some(word.text.len()),
        '}' => {
            if let Some(at) = *brace_at {
                let inner = &word.text[at..];
                word.brace |= inner.contains(',') || inner.contains("..");
            }
        }
        '=' if !word.assignment
            && !word.quoted
            && is_name(word.text.strip_suffix('+').unwrap_or(&word.text)) =>
        {
            word.assignment = true;
        }
        _ => {}
    }
    word.text.push(c);
}

fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// `2` in `2>file` or `{fd}` in `{fd}>file`.
pub(super) fn names_descriptor(word: &Word) -> bool {
    let text = word.text.as_str();
    let digits = !text.is_empty() && text.chars().all(|c| c.is_ascii_digit());
    let variable = text
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .is_some_and(is_name);
    !word.quoted && !word.param && (digits || variable)
}

/// The target of `>&` or `<&` names a descriptor (`1`, `2-`) or closes one (`-`).
pub(super) fn is_descriptor(word: &Word) -> bool {
    let digits = word.text.strip_suffix('-').unwrap_or(&word.text);
    !word.quoted && !word.expands() && digits.chars().all(|c| c.is_ascii_digit())
}
