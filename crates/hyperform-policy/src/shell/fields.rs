//! What words with parameter expansions may become at run time, for deny
//! and ask matching only: `rm${IFS}-rf` can run as `rm -rf` after word
//! splitting, and `r${EMPTY}m` as `rm` when the variable is empty.

use super::syntax::Word;

/// Two readings of `words` when any word expands: every expansion as a
/// field separator, and every expansion as empty. `None` when nothing
/// expands.
pub(crate) fn expansion_readings(words: &[Word]) -> Option<[Vec<Word>; 2]> {
    if !words.iter().any(|word| word.param) {
        return None;
    }
    let mut split = Vec::new();
    let mut joined = Vec::new();
    for word in words {
        if !word.param {
            split.push(word.clone());
            joined.push(word.clone());
            continue;
        }
        let (fields, concatenated) = fields(&word.text);
        let with = |text: String| Word {
            text,
            param: false,
            ..word.clone()
        };
        split.extend(fields.into_iter().map(with));
        if !concatenated.is_empty() {
            joined.push(with(concatenated));
        }
    }
    Some([split, joined])
}

/// The literal runs between expansions, and their concatenation.
fn fields(text: &str) -> (Vec<String>, String) {
    let chars: Vec<char> = text.chars().collect();
    let mut fields = vec![String::new()];
    let mut joined = String::new();
    let mut at = 0;
    while at < chars.len() {
        let end = expansion_end(&chars, at);
        if end > at {
            fields.push(String::new());
            at = end;
            continue;
        }
        if let Some(field) = fields.last_mut() {
            field.push(chars[at]);
        }
        joined.push(chars[at]);
        at += 1;
    }
    fields.retain(|field| !field.is_empty());
    (fields, joined)
}

/// End of the expansion starting at `start`, or `start` when there is none.
fn expansion_end(chars: &[char], start: usize) -> usize {
    if chars[start] != '$' {
        return start;
    }
    let close = |open: char, close: char| {
        let mut depth = 0usize;
        for (offset, &c) in chars[start + 1..].iter().enumerate() {
            if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    return start + offset + 2;
                }
            }
        }
        chars.len()
    };
    match chars.get(start + 1) {
        Some('{') => close('{', '}'),
        Some('(') => close('(', ')'),
        Some(c) if c.is_ascii_digit() || "@*#?$!-".contains(*c) => start + 2,
        Some(c) if *c == '_' || c.is_ascii_alphabetic() => {
            let name = chars[start + 1..]
                .iter()
                .take_while(|c| **c == '_' || c.is_ascii_alphanumeric())
                .count();
            start + 1 + name
        }
        _ => start,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::lexer::lex;

    fn readings(command: &str) -> Option<[Vec<String>; 2]> {
        let lexed = lex(command);
        expansion_readings(lexed.segments[0].command())
            .map(|readings| readings.map(|words| words.into_iter().map(|w| w.text).collect()))
    }

    #[test]
    fn expansions_split_or_vanish() {
        let [split, joined] = readings("rm${IFS}-rf${IFS}/").unwrap();
        assert_eq!(split, ["rm", "-rf", "/"]);
        assert_eq!(joined, ["rm-rf/"]);
        let [split, joined] = readings("r${EMPTY}m -rf $HOME").unwrap();
        assert_eq!(split, ["r", "m", "-rf"]);
        assert_eq!(joined, ["rm", "-rf"]);
        let [split, _] = readings("git push$IFS--force").unwrap();
        assert_eq!(split, ["git", "push", "--force"]);
        assert!(readings("ls -la").is_none());
        assert!(readings("echo '$HOME'").is_none());
    }
}
