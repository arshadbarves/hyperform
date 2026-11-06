//! Test-only language server: Content-Length framed JSON-RPC on stdio with
//! deterministic answers computed from the open documents (see
//! `lsp_text.rs`). Diagnostics are published after every didOpen/didChange.
//! Flags: `--crash-on-init` exits during `initialize`; `--minimal` offers no
//! implementation or call hierarchy; `--quiet` never publishes diagnostics;
//! `--busy` begins an indexing progress that never ends.

mod lsp_text;

use std::collections::BTreeMap;
use std::io::{BufRead, Write};

use lsp_text::{Decl, Docs, occurrences, range};
use serde_json::{Value, json};

fn read_message(input: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            if length.is_some() {
                break;
            }
            continue;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    input.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn write_message(out: &mut impl Write, message: &Value) {
    let body = message.to_string();
    write!(out, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    out.flush().unwrap();
}

fn capabilities(minimal: bool) -> Value {
    let mut caps = json!({
        "positionEncoding": "utf-16",
        "textDocumentSync": {"openClose": true, "change": 1, "save": {"includeText": false}},
        "hoverProvider": true, "definitionProvider": true, "referencesProvider": true,
        "documentSymbolProvider": true, "workspaceSymbolProvider": true, "renameProvider": true,
    });
    if !minimal {
        caps["implementationProvider"] = json!(true);
        caps["callHierarchyProvider"] = json!(true);
    }
    json!({"capabilities": caps, "serverInfo": {"name": "zengine-fake-lsp", "version": "0.1.0"}})
}

fn named<'a>(decls: &'a [Decl], name: &str, keywords: &[&str]) -> Vec<&'a Decl> {
    decls
        .iter()
        .filter(|d| d.name == name && keywords.contains(&d.keyword))
        .collect()
}

/// Every `name(` call site: (uri, line index, byte offset).
fn call_sites(docs: &Docs, name: &str) -> Vec<(String, usize, usize)> {
    let mut sites = Vec::new();
    for (uri, lines) in docs.all() {
        for (index, line) in lines.iter().enumerate() {
            for at in occurrences(line, name) {
                if line[at + name.len()..].starts_with('(') && !line.trim_start().starts_with("fn ")
                {
                    sites.push((uri.clone(), index, at));
                }
            }
        }
    }
    sites
}

fn incoming(docs: &Docs, decls: &[Decl], name: &str) -> Value {
    let mut groups: BTreeMap<(String, usize), Vec<Value>> = BTreeMap::new();
    for (uri, index, at) in call_sites(docs, name) {
        let enclosing = decls
            .iter()
            .filter(|d| d.keyword == "fn" && d.uri == uri && d.line <= index && d.end >= index)
            .max_by_key(|d| d.line);
        if let Some(caller) = enclosing {
            let line = &docs.lines(&uri)[index];
            groups
                .entry((uri.clone(), caller.line))
                .or_default()
                .push(range(index, line, at, name.len()));
        }
    }
    let calls = groups.into_iter().filter_map(|((uri, line), ranges)| {
        let caller = decls
            .iter()
            .find(|d| d.keyword == "fn" && d.uri == uri && d.line == line)?;
        Some(json!({"from": docs.item(caller), "fromRanges": ranges}))
    });
    Value::Array(calls.collect())
}

fn outgoing(docs: &Docs, decls: &[Decl], name: &str) -> Value {
    let Some(origin) = named(decls, name, &["fn"]).into_iter().next() else {
        return json!([]);
    };
    let lines = docs.lines(&origin.uri);
    let mut calls = Vec::new();
    for callee in decls.iter().filter(|d| d.keyword == "fn" && d.name != name) {
        let mut ranges = Vec::new();
        for (index, line) in lines
            .iter()
            .enumerate()
            .take(origin.end)
            .skip(origin.line + 1)
        {
            for at in occurrences(line, &callee.name) {
                if line[at + callee.name.len()..].starts_with('(') {
                    ranges.push(range(index, line, at, callee.name.len()));
                }
            }
        }
        if !ranges.is_empty() {
            calls.push(json!({"to": docs.item(callee), "fromRanges": ranges}));
        }
    }
    Value::Array(calls)
}

fn rename(docs: &Docs, word: &str, new_name: &str) -> Value {
    let mut changes = serde_json::Map::new();
    for (uri, lines) in docs.all() {
        let edits: Vec<Value> = lines
            .iter()
            .enumerate()
            .flat_map(|(index, line)| {
                occurrences(line, word)
                    .into_iter()
                    .map(move |at| json!({"range": range(index, line, at, word.len()), "newText": new_name}))
            })
            .collect();
        if !edits.is_empty() {
            changes.insert(uri.clone(), Value::Array(edits));
        }
    }
    json!({"changes": changes})
}

fn answer(docs: &Docs, method: &str, params: &Value) -> Option<Value> {
    let decls = docs.decls();
    let word = docs.word_at(params).unwrap_or_default();
    Some(match method {
        "textDocument/definition" => named(&decls, &word, &["fn", "struct"])
            .first()
            .map_or(Value::Null, |d| docs.location(d)),
        "textDocument/references" => {
            let mut out = Vec::new();
            for (uri, lines) in docs.all() {
                for (index, line) in lines.iter().enumerate() {
                    for at in occurrences(line, &word) {
                        out.push(json!({"uri": uri, "range": range(index, line, at, word.len())}));
                    }
                }
            }
            Value::Array(out)
        }
        "textDocument/hover" => named(&decls, &word, &["fn", "struct"]).first().map_or(Value::Null, |d| {
            let signature = docs.lines(&d.uri)[d.line].trim().trim_end_matches('{').trim().to_string();
            json!({"contents": {"kind": "markdown", "value": format!("```rust\n{signature}\n```")}})
        }),
        "textDocument/implementation" => {
            Value::Array(named(&decls, &word, &["impl"]).iter().map(|d| docs.location(d)).collect())
        }
        "textDocument/documentSymbol" => docs.symbols(params["textDocument"]["uri"].as_str()?),
        "workspace/symbol" => {
            let query = params["query"].as_str().unwrap_or_default().to_lowercase();
            let symbols = decls.iter().filter(|d| d.keyword != "impl" && d.name.to_lowercase().contains(&query));
            let kind = |d: &Decl| if d.keyword == "struct" { 23 } else { 12 };
            Value::Array(
                symbols
                    .map(|d| json!({"name": d.name, "kind": kind(d), "location": docs.location(d), "containerName": "fake"}))
                    .collect(),
            )
        }
        "textDocument/prepareCallHierarchy" => {
            Value::Array(named(&decls, &word, &["fn"]).iter().map(|d| docs.item(d)).collect())
        }
        "callHierarchy/incomingCalls" => incoming(docs, &decls, params["item"]["name"].as_str()?),
        "callHierarchy/outgoingCalls" => outgoing(docs, &decls, params["item"]["name"].as_str()?),
        "textDocument/rename" => rename(docs, &word, params["newName"].as_str()?),
        _ => return None,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let (crash, minimal, quiet) = (flag("--crash-on-init"), flag("--minimal"), flag("--quiet"));
    let busy = flag("--busy");
    let mut input = std::io::stdin().lock();
    let mut out = std::io::stdout().lock();
    let mut docs = Docs::default();
    eprintln!("fake-lsp: started");
    while let Some(message) = read_message(&mut input) {
        let Some(method) = message["method"].as_str() else {
            continue; // a reply to one of our requests
        };
        let params = &message["params"];
        let result = match method {
            "initialize" if crash => {
                eprintln!("fake-lsp: crashing during initialize");
                std::process::exit(2);
            }
            "initialize" => capabilities(minimal),
            "initialized" => {
                let config = json!({"jsonrpc": "2.0", "id": "cfg-1", "method": "workspace/configuration",
                                    "params": {"items": [{"section": "fake"}]}});
                write_message(&mut out, &config);
                let progress = json!({"jsonrpc": "2.0", "id": "progress-1",
                                      "method": "window/workDoneProgress/create", "params": {"token": "t"}});
                write_message(&mut out, &progress);
                if busy {
                    let begin = json!({"jsonrpc": "2.0", "method": "$/progress",
                                       "params": {"token": "index", "value": {"kind": "begin", "title": "Indexing"}}});
                    write_message(&mut out, &begin);
                }
                continue;
            }
            "shutdown" => Value::Null,
            "exit" => std::process::exit(0),
            "textDocument/didOpen" | "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let text = params["textDocument"]["text"]
                    .as_str()
                    .or_else(|| params["contentChanges"][0]["text"].as_str())
                    .unwrap_or_default();
                docs.set(&uri, text);
                if !quiet {
                    let publish = json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
                                         "params": {"uri": uri, "diagnostics": docs.diagnostics(&uri)}});
                    write_message(&mut out, &publish);
                }
                continue;
            }
            other => match answer(&docs, other, params) {
                Some(result) => result,
                None => {
                    if let Some(id) = message.get("id") {
                        let error = json!({"jsonrpc": "2.0", "id": id,
                                           "error": {"code": -32601, "message": format!("unsupported {other}")}});
                        write_message(&mut out, &error);
                    }
                    continue;
                }
            },
        };
        if let Some(id) = message.get("id") {
            write_message(
                &mut out,
                &json!({"jsonrpc": "2.0", "id": id, "result": result}),
            );
        }
    }
}
