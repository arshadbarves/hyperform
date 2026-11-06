//! `{{name}}` placeholder substitution for the markdown prompt templates.
//!
//! - `{{name}}` is replaced by the value for `name`, which may be empty or
//!   span several lines.
//! - `{{name?}}` marks its line optional: when the value is empty or only
//!   whitespace (or no value is given), the whole line is removed.
//! - A required placeholder without a value is left verbatim, so a missing
//!   key stays visible instead of silently disappearing.
//!
//! Values are inserted as-is and never scanned for placeholders, so file
//! contents or user text containing `{{...}}` pass through unchanged.

/// Renders `template` with `values` given as `(name, value)` pairs.
pub fn render_template(template: &str, values: &[(&str, &str)]) -> String {
    template
        .split('\n')
        .filter_map(|line| render_line(line, values))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `None` when the line holds an optional placeholder without a value.
fn render_line(line: &str, values: &[(&str, &str)]) -> Option<String> {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after
            .find("}}")
            .filter(|&end| is_placeholder(&after[..end]))
        else {
            out.push_str("{{");
            rest = after;
            continue;
        };
        let name = &after[..end];
        let (key, optional) = match name.strip_suffix('?') {
            Some(key) => (key, true),
            None => (name, false),
        };
        match lookup(values, key) {
            Some(value) if !(optional && value.trim().is_empty()) => out.push_str(value),
            None if !optional => out.push_str(&rest[start..start + end + 4]),
            _ => return None,
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Some(out)
}

fn is_placeholder(name: &str) -> bool {
    let key = name.strip_suffix('?').unwrap_or(name);
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn lookup<'a>(values: &[(&str, &'a str)], key: &str) -> Option<&'a str> {
    values
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| *value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_values_without_rescanning_them() {
        let out = render_template(
            "Hello {{name}}, today is {{day}}.",
            &[("name", "{{day}}"), ("day", "Monday")],
        );
        assert_eq!(out, "Hello {{day}}, today is Monday.");
    }

    #[test]
    fn optional_lines_are_dropped_when_empty_or_missing() {
        let template = "a: {{a}}\nb: {{b?}}\nc: {{c?}}\nend\n";
        let out = render_template(template, &[("a", "1"), ("b", "  ")]);
        assert_eq!(out, "a: 1\nend\n");
        let out = render_template(template, &[("a", ""), ("b", "2"), ("c", "x\ny")]);
        assert_eq!(out, "a: \nb: 2\nc: x\ny\nend\n");
    }

    #[test]
    fn missing_required_and_non_placeholder_braces_stay_verbatim() {
        let out = render_template("{{missing}} {{ spaced }} {{}} {{a}}}", &[("a", "v")]);
        assert_eq!(out, "{{missing}} {{ spaced }} {{}} v}");
    }

    #[test]
    fn unterminated_placeholder_is_left_alone() {
        assert_eq!(render_template("x {{name", &[("name", "v")]), "x {{name");
    }
}
