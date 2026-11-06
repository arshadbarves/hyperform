//! Command arguments: shell-style splitting, `$ARGUMENTS` / `$1`..`$9`
//! substitution in templates, and MCP prompt arguments given positionally
//! or as `key=value` pairs.

use std::collections::BTreeMap;

use z_engine_integrations::PromptArgument;

/// Splits like a POSIX shell: whitespace separates words, single quotes
/// are literal, double quotes group (with `\"` and `\\` escapes), and a
/// backslash outside quotes escapes the next character.
pub(crate) fn split_args(raw: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                started = true;
                word.extend(chars.by_ref().take_while(|&c| c != '\''));
            }
            '"' => {
                started = true;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some(next @ ('"' | '\\')) => word.push(next),
                            Some(next) => {
                                word.push('\\');
                                word.push(next);
                            }
                            None => word.push('\\'),
                        },
                        other => word.push(other),
                    }
                }
            }
            '\\' => {
                started = true;
                word.extend(chars.next());
            }
            c if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            other => {
                started = true;
                word.push(other);
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// Replaces `$ARGUMENTS` with the whole argument string and `$1`..`$9`
/// with positional words (empty when missing). A template without any
/// placeholder gets the arguments appended, so they are never lost.
pub(crate) fn substitute(template: &str, raw: &str) -> String {
    let raw = raw.trim();
    let words = split_args(raw);
    let mut out = String::with_capacity(template.len() + raw.len());
    let mut used = false;
    let mut rest = template;
    while let Some(index) = rest.find('$') {
        out.push_str(&rest[..index]);
        let after = &rest[index + 1..];
        if let Some(tail) = after.strip_prefix("ARGUMENTS") {
            out.push_str(raw);
            used = true;
            rest = tail;
            continue;
        }
        match after.chars().next() {
            Some(digit @ '1'..='9') => {
                let position = digit as usize - '1' as usize;
                out.push_str(words.get(position).map_or("", String::as_str));
                used = true;
                rest = &after[1..];
            }
            _ => {
                out.push('$');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    if !used && !raw.is_empty() {
        out = format!("{}\n\nARGUMENTS: {raw}", out.trim_end());
    }
    out
}

/// Maps arguments onto a prompt's declared arguments: `key=value` words
/// name one, other words fill the remaining ones in order. Returns the
/// missing required argument names as the error.
pub(crate) fn prompt_arguments(
    declared: &[PromptArgument],
    raw: &str,
) -> Result<BTreeMap<String, String>, Vec<String>> {
    let mut values = BTreeMap::new();
    let mut positional = Vec::new();
    for word in split_args(raw) {
        match word.split_once('=') {
            Some((key, value)) if declared.iter().any(|arg| arg.name == key) => {
                values.insert(key.to_string(), value.to_string());
            }
            _ => positional.push(word),
        }
    }
    let mut positional = positional.into_iter();
    for arg in declared {
        if values.contains_key(&arg.name) {
            continue;
        }
        match positional.next() {
            Some(value) => {
                values.insert(arg.name.clone(), value);
            }
            None => break,
        }
    }
    let missing: Vec<String> = declared
        .iter()
        .filter(|arg| arg.required && !values.contains_key(&arg.name))
        .map(|arg| arg.name.clone())
        .collect();
    if missing.is_empty() {
        Ok(values)
    } else {
        Err(missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitting_respects_quotes_and_escapes() {
        assert_eq!(split_args("  a  b "), ["a", "b"]);
        assert_eq!(
            split_args(r#"one "two words" 'it''s' x\ y "q\"uote""#),
            ["one", "two words", "its", "x y", "q\"uote"]
        );
        assert_eq!(split_args(r#""" ''"#), ["", ""]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn placeholders_are_substituted() {
        let body = "Fix $1 in $2 ($ARGUMENTS) costs $5 and $0";
        assert_eq!(
            substitute(body, r#"bug "src/main.rs""#),
            r#"Fix bug in src/main.rs (bug "src/main.rs") costs  and $0"#
        );
        assert_eq!(substitute("Price: $", "x"), "Price: $\n\nARGUMENTS: x");
        assert_eq!(substitute("No args.\n", ""), "No args.\n");
    }

    fn arg(name: &str, required: bool) -> PromptArgument {
        PromptArgument {
            name: name.into(),
            description: None,
            required,
        }
    }

    #[test]
    fn prompt_arguments_map_by_key_or_position() {
        let declared = [arg("file", true), arg("focus", false)];
        let mapped = prompt_arguments(&declared, "focus=security src/lib.rs").unwrap();
        assert_eq!(mapped["file"], "src/lib.rs");
        assert_eq!(mapped["focus"], "security");
        let positional = prompt_arguments(&declared, "a.rs b").unwrap();
        assert_eq!(
            (positional["file"].as_str(), positional["focus"].as_str()),
            ("a.rs", "b")
        );
        assert_eq!(
            prompt_arguments(&declared, "focus=x"),
            Err(vec!["file".into()])
        );
    }
}
