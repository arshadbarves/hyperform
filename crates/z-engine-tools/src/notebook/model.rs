//! Jupyter notebooks as JSON: cells found by id (or `cell-N` for notebooks
//! without ids), sources in Jupyter's list-of-lines form, and output in
//! Jupyter's own style (one-space indent, trailing newline). Every field the
//! edit does not touch is preserved.

use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellType {
    Code,
    Markdown,
}

impl CellType {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "code" => Some(Self::Code),
            "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Markdown => "markdown",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Notebook {
    doc: Map<String, Value>,
}

impl Notebook {
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(text)
            .map_err(|e| format!("it is not valid notebook JSON: {e}"))?;
        match value {
            Value::Object(doc) if doc.get("cells").is_some_and(Value::is_array) => Ok(Self { doc }),
            _ => Err("it is not a notebook: there is no `cells` array".to_string()),
        }
    }

    pub(crate) fn cells(&self) -> &[Value] {
        self.doc
            .get("cells")
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }

    fn cells_mut(&mut self) -> Result<&mut Vec<Value>, String> {
        self.doc
            .get_mut("cells")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "the notebook has no `cells` array".to_string())
    }

    /// The kernel language, e.g. `python`.
    pub(crate) fn language(&self) -> Option<&str> {
        let metadata = self.doc.get("metadata")?;
        metadata
            .pointer("/language_info/name")
            .or_else(|| metadata.pointer("/kernelspec/language"))
            .and_then(Value::as_str)
    }

    /// Index of the cell with `id`, or of `cell-N`.
    pub(crate) fn find(&self, id: &str) -> Option<usize> {
        let id = id.trim();
        let cells = self.cells();
        cells
            .iter()
            .position(|cell| cell.get("id").and_then(Value::as_str) == Some(id))
            .or_else(|| {
                id.strip_prefix("cell-")
                    .and_then(|n| n.parse::<usize>().ok())
                    .filter(|&n| n < cells.len())
            })
    }

    /// nbformat 4.5+ notebooks give every cell an id.
    fn uses_ids(&self) -> bool {
        let version = |key: &str| self.doc.get(key).and_then(Value::as_u64).unwrap_or(0);
        (version("nbformat"), version("nbformat_minor")) >= (4, 5)
            || self.cells().iter().any(|cell| cell.get("id").is_some())
    }

    /// Replaces the source; a code cell's stale outputs are cleared.
    pub(crate) fn replace(
        &mut self,
        index: usize,
        source: &str,
        cell_type: Option<CellType>,
    ) -> Result<(), String> {
        let cell = self
            .cells_mut()?
            .get_mut(index)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("cell {index} is not a JSON object"))?;
        cell.insert("source".into(), source_lines(source));
        if let Some(kind) = cell_type {
            cell.insert("cell_type".into(), Value::from(kind.label()));
        }
        if cell.get("cell_type").and_then(Value::as_str) == Some("code") {
            cell.insert("execution_count".into(), Value::Null);
            cell.insert("outputs".into(), Value::Array(Vec::new()));
        } else {
            cell.remove("execution_count");
            cell.remove("outputs");
        }
        Ok(())
    }

    /// Inserts a new cell at `at`; returns its id.
    pub(crate) fn insert(
        &mut self,
        at: usize,
        source: &str,
        kind: CellType,
    ) -> Result<String, String> {
        let id = self.uses_ids().then(|| self.fresh_id());
        let mut cell = Map::new();
        cell.insert("cell_type".into(), Value::from(kind.label()));
        if let Some(id) = &id {
            cell.insert("id".into(), Value::from(id.as_str()));
        }
        cell.insert("metadata".into(), Value::Object(Map::new()));
        cell.insert("source".into(), source_lines(source));
        if kind == CellType::Code {
            cell.insert("execution_count".into(), Value::Null);
            cell.insert("outputs".into(), Value::Array(Vec::new()));
        }
        let cells = self.cells_mut()?;
        let at = at.min(cells.len());
        cells.insert(at, Value::Object(cell));
        Ok(id.unwrap_or_else(|| format!("cell-{at}")))
    }

    pub(crate) fn delete(&mut self, index: usize) -> Result<Value, String> {
        let cells = self.cells_mut()?;
        if index >= cells.len() {
            return Err(format!("there is no cell {index}"));
        }
        Ok(cells.remove(index))
    }

    pub(crate) fn to_json(&self) -> Result<String, String> {
        let mut out = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
        let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
        self.doc
            .serialize(&mut serializer)
            .map_err(|e| format!("could not serialize the notebook: {e}"))?;
        let mut text =
            String::from_utf8(out).map_err(|e| format!("could not serialize the notebook: {e}"))?;
        text.push('\n');
        Ok(text)
    }

    fn fresh_id(&self) -> String {
        loop {
            let ulid = ulid::Ulid::new().to_string().to_lowercase();
            let id = ulid[ulid.len() - 8..].to_string();
            if self.find(&id).is_none() {
                return id;
            }
        }
    }
}

/// The cell's id, or `cell-N` when it has none.
pub(crate) fn cell_id(cell: &Value, index: usize) -> String {
    cell.get("id")
        .and_then(Value::as_str)
        .map_or_else(|| format!("cell-{index}"), str::to_string)
}

pub(crate) fn cell_kind(cell: &Value) -> &str {
    cell.get("cell_type")
        .and_then(Value::as_str)
        .unwrap_or("code")
}

/// A multiline field stored as a string or a list of strings.
pub(crate) fn joined(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).collect(),
        _ => String::new(),
    }
}

/// Jupyter's list-of-lines form: every line keeps its `\n` but the last.
pub(crate) fn source_lines(text: &str) -> Value {
    Value::Array(text.split_inclusive('\n').map(Value::from).collect())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn notebook() -> Notebook {
        let doc = json!({
            "cells": [
                {"cell_type": "markdown", "id": "intro", "metadata": {}, "source": ["# Title\n", "text"]},
                {"cell_type": "code", "id": "load", "metadata": {"tags": ["x"]}, "execution_count": 3,
                 "outputs": [{"output_type": "stream", "name": "stdout", "text": ["hi\n"]}],
                 "source": "import os"}
            ],
            "metadata": {"language_info": {"name": "python"}},
            "nbformat": 4, "nbformat_minor": 5
        });
        Notebook::parse(&doc.to_string()).unwrap()
    }

    #[test]
    fn cells_are_found_by_id_or_index() {
        let nb = notebook();
        assert_eq!(nb.find("load"), Some(1));
        assert_eq!(nb.find("cell-0"), Some(0));
        assert_eq!(nb.find("cell-9"), None);
        assert_eq!(nb.language(), Some("python"));
        assert_eq!(joined(nb.cells()[0].get("source")), "# Title\ntext");
    }

    #[test]
    fn edits_keep_other_fields_and_clear_stale_outputs() {
        let mut nb = notebook();
        nb.replace(1, "import sys\nprint(1)", None).unwrap();
        let cell = &nb.cells()[1];
        assert_eq!(cell["source"], json!(["import sys\n", "print(1)"]));
        assert_eq!(cell["outputs"], json!([]));
        assert_eq!(cell["metadata"], json!({"tags": ["x"]}));
        let id = nb.insert(1, "new", CellType::Markdown).unwrap();
        assert_eq!(id.len(), 8);
        assert_eq!(nb.cells()[1]["id"], json!(id));
        nb.delete(0).unwrap();
        assert_eq!(nb.cells().len(), 2);
        let text = nb.to_json().unwrap();
        assert!(text.starts_with("{\n \"cells\": [\n  {\n"), "{text}");
        assert!(text.ends_with("}\n"));
    }
}
