//! The opt-in decision dataset (`decisions.record_dataset`): one JSONL file
//! per question under `<data dir>/decisions/dataset/`, for fitting
//! calibration and fine-tuning offline. Lines are appended, never rewritten.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::StoreError;

const MAX_NAME_CHARS: usize = 64;

#[derive(Debug, Clone)]
pub struct DecisionDataset {
    dir: PathBuf,
}

impl DecisionDataset {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Appends `line` to `<question>.jsonl`; the name is reduced to
    /// `[a-z0-9_-]`.
    pub fn append(&self, question: &str, line: &Value) -> Result<PathBuf, StoreError> {
        fs::create_dir_all(&self.dir).map_err(|error| StoreError::io(&self.dir, error))?;
        let path = self.dir.join(format!("{}.jsonl", file_stem(question)));
        let mut text = line.to_string();
        text.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| StoreError::io(&path, error))?;
        file.write_all(text.as_bytes())
            .map_err(|error| StoreError::io(&path, error))?;
        Ok(path)
    }
}

fn file_stem(question: &str) -> String {
    let stem: String = question
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '_' | '-' => c,
            _ => '_',
        })
        .take(MAX_NAME_CHARS)
        .collect();
    if stem.is_empty() {
        "question".into()
    } else {
        stem
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn appends_one_line_per_record_under_a_safe_name() {
        let tmp = tempfile::tempdir().unwrap();
        let dataset = DecisionDataset::new(tmp.path().join("dataset"));
        let path = dataset
            .append("../Relevant Output", &json!({ "a": 1 }))
            .unwrap();
        dataset
            .append("../Relevant Output", &json!({ "a": 2 }))
            .unwrap();
        assert_eq!(path.file_name().unwrap(), "___relevant_output.jsonl");
        assert_eq!(path.parent().unwrap(), dataset.dir());
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text, "{\"a\":1}\n{\"a\":2}\n");
        assert!(
            dataset
                .append("", &json!({}))
                .unwrap()
                .ends_with("question.jsonl")
        );
    }
}
