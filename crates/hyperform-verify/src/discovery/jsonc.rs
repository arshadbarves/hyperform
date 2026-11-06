//! JSON with comments (`deno.jsonc`): `//` and `/* */` comments and
//! trailing commas are removed outside strings, so the rest parses as JSON.

pub(crate) fn strip(text: &str) -> String {
    without_trailing_commas(&without_comments(text))
}

fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => while chars.next_if(|&next| next != '\n').is_some() {},
            ('/', Some('*')) => {
                chars.next();
                let mut previous = '\0';
                for next in chars.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    previous = next;
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}

fn without_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let (mut in_string, mut escaped) = (false, false);
    for (i, &c) in chars.iter().enumerate() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
        }
        let closes = || {
            chars[i + 1..]
                .iter()
                .find(|next| !next.is_whitespace())
                .is_some_and(|next| matches!(next, '}' | ']'))
        };
        if c == ',' && closes() {
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_trailing_commas_outside_strings_are_removed() {
        let text = r#"{
  // tasks run with `deno task`
  "tasks": {
    "test": "deno test -A", /* all permissions */
    "url": "https://example.com/a//b", "glob": "src/**/*.ts",
    "quote": "say \"hi\", // not a comment",
  },
  "exclude": ["dist",],
}"#;
        let value: serde_json::Value = serde_json::from_str(&strip(text)).unwrap();
        assert_eq!(value["tasks"]["test"], "deno test -A");
        assert_eq!(value["tasks"]["url"], "https://example.com/a//b");
        assert_eq!(value["tasks"]["glob"], "src/**/*.ts");
        assert_eq!(value["tasks"]["quote"], "say \"hi\", // not a comment");
        assert_eq!(value["exclude"][0], "dist");
    }
}
