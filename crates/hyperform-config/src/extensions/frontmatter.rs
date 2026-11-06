//! Markdown with an optional YAML frontmatter header (`---` fenced), read
//! leniently: scalars convert to text, and list fields accept a YAML list
//! or a comma-separated string, as Claude Code files use both.

use serde_yaml_ng::{Mapping, Value};

pub(crate) struct Document<'a> {
    pub(crate) meta: Meta,
    /// Body without surrounding blank lines.
    pub(crate) body: &'a str,
}

/// How a string-valued list field splits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListStyle {
    /// Commas or whitespace outside parentheses: `Read, Bash(git add:*)`.
    Tools,
    /// Commas outside braces: `src/**/*.{ts,tsx}, lib/**`.
    Globs,
}

pub(crate) fn parse_document(text: &str) -> Result<Document<'_>, String> {
    let (yaml, body) = split(text)?;
    let meta = match yaml {
        Some(yaml) => Meta::parse(yaml)?,
        None => Meta::default(),
    };
    let body = body.trim_start_matches(['\r', '\n']).trim_end();
    Ok(Document { meta, body })
}

/// The frontmatter text (if any) and the body.
fn split(text: &str) -> Result<(Option<&str>, &str), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = text.split_inclusive('\n').next().unwrap_or_default();
    if first.trim_end() != "---" {
        return Ok((None, text));
    }
    let rest = &text[first.len()..];
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Ok((Some(&rest[..offset]), &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    Err("frontmatter is not closed by a `---` line".to_string())
}

#[derive(Debug, Default)]
pub(crate) struct Meta(Mapping);

impl Meta {
    fn parse(yaml: &str) -> Result<Self, String> {
        let blank = |line: &str| line.trim().is_empty() || line.trim_start().starts_with('#');
        if yaml.lines().all(blank) {
            return Ok(Self::default());
        }
        let value = match serde_yaml_ng::from_str::<Value>(yaml) {
            Ok(value) => value,
            Err(error) => serde_yaml_ng::from_str(&quote_bracket_hints(yaml))
                .map_err(|_| format!("invalid frontmatter: {error}"))?,
        };
        match value {
            Value::Null => Ok(Self::default()),
            Value::Mapping(map) => Ok(Self(map)),
            _ => Err("frontmatter must be a YAML mapping".to_string()),
        }
    }

    /// The first present, non-null key among `keys` (spelling aliases).
    fn get<'k>(&self, keys: &[&'k str]) -> Option<(&'k str, &Value)> {
        keys.iter().find_map(|key| {
            self.0
                .get(*key)
                .filter(|value| !value.is_null())
                .map(|value| (*key, value))
        })
    }

    /// Trimmed text; blank counts as absent.
    pub(crate) fn text(&self, keys: &[&str]) -> Result<Option<String>, String> {
        let Some((key, value)) = self.get(keys) else {
            return Ok(None);
        };
        let text = scalar_text(value).ok_or_else(|| format!("`{key}` must be text"))?;
        Ok(Some(text.trim().to_string()).filter(|text| !text.is_empty()))
    }

    /// A YAML list or a delimited string; a blank string counts as absent.
    pub(crate) fn list(
        &self,
        keys: &[&str],
        style: ListStyle,
    ) -> Result<Option<Vec<String>>, String> {
        let Some((key, value)) = self.get(keys) else {
            return Ok(None);
        };
        let items = match value {
            Value::Sequence(items) => items
                .iter()
                .map(|item| {
                    scalar_text(item).ok_or_else(|| format!("`{key}` entries must be text"))
                })
                .collect::<Result<Vec<_>, _>>()?,
            other => {
                let text =
                    scalar_text(other).ok_or_else(|| format!("`{key}` must be text or a list"))?;
                if text.trim().is_empty() {
                    return Ok(None);
                }
                split_list(&text, style)
            }
        };
        let items = items.into_iter().map(|item| item.trim().to_string());
        Ok(Some(items.filter(|item| !item.is_empty()).collect()))
    }

    pub(crate) fn flag(&self, keys: &[&str]) -> Result<Option<bool>, String> {
        let Some((key, value)) = self.get(keys) else {
            return Ok(None);
        };
        let flag = match value {
            Value::Bool(flag) => Some(*flag),
            Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "true" | "yes" => Some(true),
                "false" | "no" => Some(false),
                _ => None,
            },
            _ => None,
        };
        flag.map(Some)
            .ok_or_else(|| format!("`{key}` must be true or false"))
    }

    pub(crate) fn count(&self, keys: &[&str]) -> Result<Option<u32>, String> {
        let Some((key, value)) = self.get(keys) else {
            return Ok(None);
        };
        let count = match value {
            Value::Number(number) => number.as_u64().and_then(|n| u32::try_from(n).ok()),
            Value::String(text) => text.trim().parse().ok(),
            _ => None,
        };
        count
            .map(Some)
            .ok_or_else(|| format!("`{key}` must be a whole number"))
    }

    /// Like [`Meta::text`], but a YAML list renders as `[a] [b]`, since
    /// `argument-hint: [message]` parses as a list.
    pub(crate) fn hint(&self, keys: &[&str]) -> Result<Option<String>, String> {
        match self.get(keys) {
            Some((_, Value::Sequence(items))) => {
                let parts: Vec<String> = items
                    .iter()
                    .filter_map(scalar_text)
                    .map(|item| format!("[{item}]"))
                    .collect();
                Ok(Some(parts.join(" ")).filter(|hint| !hint.is_empty()))
            }
            _ => self.text(keys),
        }
    }
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// Claude Code files write `argument-hint: [pr] [priority]`, which is not
/// valid YAML; retry with such values quoted.
fn quote_bracket_hints(yaml: &str) -> String {
    let lines = yaml.lines().map(|line| match line.split_once(':') {
        Some((key, value))
            if key.trim() == "argument-hint" && value.trim_start().starts_with('[') =>
        {
            format!("{key}: '{}'", value.trim().replace('\'', "''"))
        }
        _ => line.to_string(),
    });
    lines.collect::<Vec<_>>().join("\n")
}

pub(crate) fn split_list(text: &str, style: ListStyle) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in text.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        let separator =
            depth == 0 && (ch == ',' || (style == ListStyle::Tools && ch.is_whitespace()));
        if separator {
            items.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    items.push(current);
    let items = items.into_iter().map(|item| item.trim().to_string());
    items.filter(|item| !item.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_fenced_frontmatter_from_the_body() {
        let doc = parse_document("\u{feff}---\r\nname: a\r\n---\r\n\r\nBody\n").unwrap();
        assert_eq!(doc.meta.text(&["name"]).unwrap().as_deref(), Some("a"));
        assert_eq!(doc.body, "Body");
        let plain = parse_document("# Title\n---\nnot frontmatter").unwrap();
        assert!(plain.body.starts_with("# Title"));
        assert!(parse_document("---\nname: a\n").is_err());
        assert!(
            parse_document("---\n---\nbody")
                .unwrap()
                .meta
                .text(&["name"])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn lists_split_outside_brackets() {
        let tools = split_list("Read, Bash(git add:*) Grep", ListStyle::Tools);
        assert_eq!(tools, ["Read", "Bash(git add:*)", "Grep"]);
        let globs = split_list("src/**/*.{ts,tsx}, lib/**", ListStyle::Globs);
        assert_eq!(globs, ["src/**/*.{ts,tsx}", "lib/**"]);
    }

    #[test]
    fn scalars_are_read_leniently() {
        let doc = parse_document("---\nmaxTurns: \"12\"\nflag: yes\nn: 5\nhint: [message]\n---\n")
            .unwrap();
        assert_eq!(doc.meta.count(&["maxTurns"]).unwrap(), Some(12));
        assert_eq!(doc.meta.flag(&["flag"]).unwrap(), Some(true));
        assert_eq!(doc.meta.text(&["n"]).unwrap().as_deref(), Some("5"));
        assert_eq!(
            doc.meta.hint(&["hint"]).unwrap().as_deref(),
            Some("[message]")
        );
        assert!(doc.meta.count(&["flag"]).is_err());
    }

    #[test]
    fn invalid_yaml_is_an_error_but_bracket_hints_are_accepted() {
        let hint = parse_document("---\nargument-hint: [pr] [priority]\n---\nx").unwrap();
        assert_eq!(
            hint.meta.hint(&["argument-hint"]).unwrap().as_deref(),
            Some("[pr] [priority]")
        );
        let error = parse_document("---\nname: [unclosed\n---\n").err().unwrap();
        assert!(error.starts_with("invalid frontmatter"), "{error}");
        assert!(parse_document("---\n- a\n- b\n---\n").is_err());
    }
}
