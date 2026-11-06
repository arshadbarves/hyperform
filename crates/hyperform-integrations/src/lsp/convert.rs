//! Decoding of language-server results into raw values that still carry
//! UTF-16 ranges; `resolve` turns them into model coordinates. Locations in
//! non-`file` URIs are skipped; broken shapes are protocol errors.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::position::LspPosition;
use super::uri::uri_to_path;
use crate::error::IntegrationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawLocation {
    pub path: PathBuf,
    pub range: LspRange,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: u32,
    pub range: LspRange,
    pub selection_range: LspRange,
    #[serde(default)]
    pub children: Vec<RawSymbol>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawWorkspaceSymbol {
    pub name: String,
    pub kind: u32,
    pub container: Option<String>,
    pub location: RawLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawCallItem {
    pub name: String,
    pub kind: u32,
    pub detail: Option<String>,
    pub location: RawLocation,
    /// The item as the server sent it, for follow-up requests.
    pub wire: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawCall {
    pub item: RawCallItem,
    pub ranges: Vec<LspRange>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawDiagnostic {
    pub range: LspRange,
    pub severity: Option<u64>,
    pub code: Option<String>,
    pub source: Option<String>,
    pub message: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WireLocation {
    #[serde(rename_all = "camelCase")]
    Link {
        target_uri: String,
        target_selection_range: LspRange,
    },
    Plain {
        uri: String,
        range: LspRange,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireSymbolInformation {
    name: String,
    kind: u32,
    location: WireMaybeRange,
    container_name: Option<String>,
}

#[derive(Deserialize)]
struct WireMaybeRange {
    uri: String,
    range: Option<LspRange>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireCallItem {
    name: String,
    kind: u32,
    detail: Option<String>,
    uri: String,
    selection_range: LspRange,
}

#[derive(Deserialize)]
struct WireTextEdit {
    range: LspRange,
    #[serde(rename = "newText")]
    new_text: String,
}

fn malformed(what: &str, problem: impl std::fmt::Display) -> IntegrationError {
    IntegrationError::Protocol(format!("malformed {what} result: {problem}"))
}

fn decode<T: DeserializeOwned>(value: &Value, what: &str) -> Result<T, IntegrationError> {
    T::deserialize(value).map_err(|e| malformed(what, e))
}

/// `null`, one object, or an array of objects.
fn list<'a>(result: &'a Value, what: &str) -> Result<&'a [Value], IntegrationError> {
    match result {
        Value::Null => Ok(&[]),
        Value::Array(items) => Ok(items),
        Value::Object(_) => Ok(std::slice::from_ref(result)),
        other => Err(malformed(what, format!("unexpected {other}"))),
    }
}

fn located(uri: &str, range: LspRange) -> Option<RawLocation> {
    match uri_to_path(uri) {
        Some(path) => Some(RawLocation { path, range }),
        None => {
            tracing::debug!(%uri, "skipped a location outside the file system");
            None
        }
    }
}

/// `Location | Location[] | LocationLink[] | null`.
pub(crate) fn locations(result: &Value) -> Result<Vec<RawLocation>, IntegrationError> {
    let mut out = Vec::new();
    for item in list(result, "location")? {
        let (uri, range) = match decode::<WireLocation>(item, "location")? {
            WireLocation::Link {
                target_uri,
                target_selection_range,
            } => (target_uri, target_selection_range),
            WireLocation::Plain { uri, range } => (uri, range),
        };
        out.extend(located(&uri, range));
    }
    Ok(out)
}

/// `DocumentSymbol[] | SymbolInformation[] | null`; flat symbol
/// information becomes childless nodes.
pub(crate) fn document_symbols(result: &Value) -> Result<Vec<RawSymbol>, IntegrationError> {
    list(result, "documentSymbol")?
        .iter()
        .map(|item| {
            if item.get("location").is_none() {
                return decode::<RawSymbol>(item, "documentSymbol");
            }
            let info: WireSymbolInformation = decode(item, "documentSymbol")?;
            let range = info.location.range.unwrap_or(ZERO_RANGE);
            Ok(RawSymbol {
                name: info.name,
                detail: info.container_name,
                kind: info.kind,
                range,
                selection_range: range,
                children: Vec::new(),
            })
        })
        .collect()
}

const ZERO_RANGE: LspRange = LspRange {
    start: LspPosition {
        line: 0,
        character: 0,
    },
    end: LspPosition {
        line: 0,
        character: 0,
    },
};

/// `SymbolInformation[] | WorkspaceSymbol[] | null`.
pub(crate) fn workspace_symbols(
    result: &Value,
) -> Result<Vec<RawWorkspaceSymbol>, IntegrationError> {
    let mut out = Vec::new();
    for item in list(result, "workspace/symbol")? {
        let info: WireSymbolInformation = decode(item, "workspace/symbol")?;
        let range = info.location.range.unwrap_or(ZERO_RANGE);
        if let Some(location) = located(&info.location.uri, range) {
            out.push(RawWorkspaceSymbol {
                name: info.name,
                kind: info.kind,
                container: info.container_name,
                location,
            });
        }
    }
    Ok(out)
}

fn call_item(value: &Value) -> Result<Option<RawCallItem>, IntegrationError> {
    let item: WireCallItem = decode(value, "call hierarchy item")?;
    Ok(
        located(&item.uri, item.selection_range).map(|location| RawCallItem {
            name: item.name,
            kind: item.kind,
            detail: item.detail,
            location,
            wire: value.clone(),
        }),
    )
}

/// `CallHierarchyItem[] | null` from `prepareCallHierarchy`.
pub(crate) fn call_items(result: &Value) -> Result<Vec<RawCallItem>, IntegrationError> {
    let mut out = Vec::new();
    for item in list(result, "prepareCallHierarchy")? {
        out.extend(call_item(item)?);
    }
    Ok(out)
}

/// Incoming (`key` = "from") or outgoing (`key` = "to") calls.
pub(crate) fn calls(result: &Value, key: &str) -> Result<Vec<RawCall>, IntegrationError> {
    let mut out = Vec::new();
    for entry in list(result, "call hierarchy")? {
        let item = entry
            .get(key)
            .ok_or_else(|| malformed("call hierarchy", format!("a call without `{key}`")))?;
        let ranges = match entry.get("fromRanges") {
            Some(ranges) => decode::<Vec<LspRange>>(ranges, "call hierarchy")?,
            None => Vec::new(),
        };
        if let Some(item) = call_item(item)? {
            out.push(RawCall { item, ranges });
        }
    }
    Ok(out)
}

/// A `WorkspaceEdit` as text edits per file. File creation, renaming and
/// deletion cannot be previewed as text edits and are refused.
pub(crate) fn workspace_edit(
    result: &Value,
) -> Result<BTreeMap<PathBuf, Vec<(LspRange, String)>>, IntegrationError> {
    let mut files: BTreeMap<PathBuf, Vec<(LspRange, String)>> = BTreeMap::new();
    let mut add = |uri: &str, edits: &Value| -> Result<(), IntegrationError> {
        let path = uri_to_path(uri).ok_or_else(|| {
            IntegrationError::Unsupported(format!("the edit touches a non-file URI `{uri}`"))
        })?;
        let edits: Vec<WireTextEdit> = decode(edits, "workspace edit")?;
        files
            .entry(path)
            .or_default()
            .extend(edits.into_iter().map(|edit| (edit.range, edit.new_text)));
        Ok(())
    };
    if let Some(changes) = result.get("documentChanges").and_then(Value::as_array) {
        for change in changes {
            if let Some(kind) = change.get("kind").and_then(Value::as_str) {
                return Err(IntegrationError::Unsupported(format!(
                    "the edit would {kind} a file, which a text preview cannot express"
                )));
            }
            let uri = change["textDocument"]["uri"]
                .as_str()
                .ok_or_else(|| malformed("workspace edit", "a document change without a URI"))?;
            add(uri, change.get("edits").unwrap_or(&Value::Null))?;
        }
    } else if let Some(changes) = result.get("changes").and_then(Value::as_object) {
        for (uri, edits) in changes {
            add(uri, edits)?;
        }
    } else if !result.is_null() {
        return Err(malformed(
            "workspace edit",
            "neither `changes` nor `documentChanges`",
        ));
    }
    Ok(files)
}

/// The `diagnostics` array of a publish.
pub(crate) fn diagnostics(items: &[Value]) -> Vec<RawDiagnostic> {
    items
        .iter()
        .filter_map(|item| {
            let range = LspRange::deserialize(item.get("range")?).ok()?;
            let code = match item.get("code") {
                Some(Value::String(code)) => Some(code.clone()),
                Some(Value::Number(code)) => Some(code.to_string()),
                _ => None,
            };
            Some(RawDiagnostic {
                range,
                severity: item.get("severity").and_then(Value::as_u64),
                code,
                source: item
                    .get("source")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                message: item.get("message").and_then(Value::as_str)?.to_string(),
            })
        })
        .collect()
}

/// Hover contents as markdown; `None` when the server has nothing.
pub(crate) fn hover(result: &Value) -> Option<String> {
    let text = marked(result.get("contents")?);
    (!text.trim().is_empty()).then_some(text)
}

fn marked(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(marked).collect::<Vec<_>>().join("\n\n"),
        Value::Object(object) => {
            let text = object
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or_default();
            match object.get("language").and_then(Value::as_str) {
                Some(language) => format!("```{language}\n{text}\n```"),
                None => text.to_string(),
            }
        }
        _ => String::new(),
    }
}

// The fixtures use Unix-style `file:///a.rs` URIs.
#[cfg(all(test, unix))]
#[path = "convert_tests.rs"]
mod tests;
