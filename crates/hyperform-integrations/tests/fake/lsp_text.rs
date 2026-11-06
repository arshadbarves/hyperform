//! Text analysis for the fake language server: words, declarations and
//! ranges. Columns are UTF-16 code units, computed independently of the
//! client's conversion code.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// A `struct N`, `impl N` or `fn N` line.
pub struct Decl {
    pub uri: String,
    pub line: usize,
    pub indent: usize,
    pub byte: usize,
    pub name: String,
    pub keyword: &'static str,
    /// The line of the matching closing brace.
    pub end: usize,
}

/// Open documents: URI -> lines without terminators.
#[derive(Default)]
pub struct Docs {
    docs: BTreeMap<String, Vec<String>>,
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn utf16(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The LSP range of `len` bytes at `byte` in line `line`.
pub fn range(line: usize, text: &str, byte: usize, len: usize) -> Value {
    let start = utf16(&text[..byte]);
    let end = start + utf16(&text[byte..byte + len]);
    json!({"start": {"line": line, "character": start}, "end": {"line": line, "character": end}})
}

fn byte_at(text: &str, units: usize) -> usize {
    let mut seen = 0;
    for (byte, c) in text.char_indices() {
        if seen >= units {
            return byte;
        }
        seen += c.len_utf16();
    }
    text.len()
}

/// Whole-word occurrences of `word` (byte offsets).
pub fn occurrences(text: &str, word: &str) -> Vec<usize> {
    text.match_indices(word)
        .map(|(at, _)| at)
        .filter(|at| {
            let before = text[..*at].chars().next_back().is_none_or(|c| !is_ident(c));
            let after = text[at + word.len()..]
                .chars()
                .next()
                .is_none_or(|c| !is_ident(c));
            before && after
        })
        .collect()
}

impl Docs {
    pub fn set(&mut self, uri: &str, text: &str) {
        let lines = text
            .split('\n')
            .map(|l| l.trim_end_matches('\r').to_string())
            .collect();
        self.docs.insert(uri.to_string(), lines);
    }

    pub fn lines(&self, uri: &str) -> &[String] {
        self.docs.get(uri).map_or(&[], Vec::as_slice)
    }

    pub fn all(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.docs.iter()
    }

    /// The identifier at `{textDocument, position}`.
    pub fn word_at(&self, params: &Value) -> Option<String> {
        let uri = params["textDocument"]["uri"].as_str()?;
        let index = usize::try_from(params["position"]["line"].as_u64()?).ok()?;
        let line = self.lines(uri).get(index)?;
        let units = usize::try_from(params["position"]["character"].as_u64()?).ok()?;
        let at = byte_at(line, units);
        let start = line[..at]
            .char_indices()
            .rev()
            .take_while(|(_, c)| is_ident(*c))
            .last()
            .map_or(at, |(i, _)| i);
        let end = line[at..]
            .char_indices()
            .find(|(_, c)| !is_ident(*c))
            .map_or(line.len(), |(i, _)| at + i);
        (start < end).then(|| line[start..end].to_string())
    }

    pub fn decls(&self) -> Vec<Decl> {
        let mut out = Vec::new();
        for (uri, lines) in &self.docs {
            for (index, line) in lines.iter().enumerate() {
                let trimmed = line.trim_start();
                let indent = line.len() - trimmed.len();
                for keyword in ["struct", "impl", "fn"] {
                    let Some(rest) = trimmed
                        .strip_prefix(keyword)
                        .and_then(|r| r.strip_prefix(' '))
                    else {
                        continue;
                    };
                    let name: String = rest.chars().take_while(|c| is_ident(*c)).collect();
                    if name.is_empty() {
                        continue;
                    }
                    let close = format!("{}}}", " ".repeat(indent));
                    let end = lines[index..]
                        .iter()
                        .position(|l| *l == close)
                        .map_or(index, |o| index + o);
                    out.push(Decl {
                        uri: uri.clone(),
                        line: index,
                        indent,
                        byte: indent + keyword.len() + 1,
                        name,
                        keyword,
                        end,
                    });
                }
            }
        }
        out
    }

    pub fn name_range(&self, decl: &Decl) -> Value {
        range(
            decl.line,
            &self.lines(&decl.uri)[decl.line],
            decl.byte,
            decl.name.len(),
        )
    }

    pub fn location(&self, decl: &Decl) -> Value {
        json!({"uri": decl.uri, "range": self.name_range(decl)})
    }

    /// A call hierarchy item for a function declaration.
    pub fn item(&self, decl: &Decl) -> Value {
        let lines = self.lines(&decl.uri);
        let full = json!({
            "start": {"line": decl.line, "character": 0},
            "end": {"line": decl.end, "character": utf16(&lines[decl.end])}
        });
        json!({"name": decl.name, "kind": 12, "uri": decl.uri, "range": full,
               "selectionRange": self.name_range(decl), "data": {"name": decl.name}})
    }

    /// A hierarchical symbol tree: structs with fields, impls with methods,
    /// and top-level functions.
    pub fn symbols(&self, uri: &str) -> Value {
        let decls: Vec<Decl> = self.decls().into_iter().filter(|d| d.uri == uri).collect();
        let lines = self.lines(uri);
        let node = |decl: &Decl, kind: u32, name: String, children: Vec<Value>| {
            let full = json!({"start": {"line": decl.line, "character": 0},
                              "end": {"line": decl.end, "character": utf16(&lines[decl.end])}});
            json!({"name": name, "kind": kind, "range": full, "selectionRange": self.name_range(decl), "children": children})
        };
        let mut out = Vec::new();
        for top in decls.iter().filter(|d| d.indent == 0) {
            let mut children = Vec::new();
            if top.keyword == "struct" {
                for (index, line) in lines.iter().enumerate().take(top.end).skip(top.line + 1) {
                    let trimmed = line.trim_start();
                    if let Some((field, _)) = trimmed.split_once(':') {
                        let byte = line.len() - trimmed.len();
                        let r = range(index, line, byte, field.len());
                        children.push(
                            json!({"name": field, "kind": 8, "range": r, "selectionRange": r}),
                        );
                    }
                }
            }
            for inner in decls
                .iter()
                .filter(|d| d.indent > 0 && d.line > top.line && d.end <= top.end)
            {
                children.push(node(inner, 6, inner.name.clone(), Vec::new()));
            }
            let (kind, name) = match top.keyword {
                "struct" => (23, top.name.clone()),
                "impl" => (19, format!("impl {}", top.name)),
                _ => (12, top.name.clone()),
            };
            out.push(node(top, kind, name, children));
        }
        Value::Array(out)
    }

    /// Diagnostics for `ERROR` (error, string code) and `WARN` (warning,
    /// numeric code) markers.
    pub fn diagnostics(&self, uri: &str) -> Value {
        let mut out = Vec::new();
        for (index, line) in self.lines(uri).iter().enumerate() {
            if let Some(at) = line.find("ERROR") {
                out.push(
                    json!({"range": range(index, line, at, 5), "severity": 1, "code": "E0001",
                                "source": "fake", "message": "found an ERROR marker"}),
                );
            }
            if let Some(at) = line.find("WARN") {
                out.push(
                    json!({"range": range(index, line, at, 4), "severity": 2, "code": 7,
                                "source": "fake", "message": "found a WARN marker"}),
                );
            }
        }
        Value::Array(out)
    }
}
