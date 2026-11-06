//! What a successful tool call returns: model-facing content, a one-line
//! summary for the tool card, and the effects the engine must account for.

use std::path::PathBuf;

use z_engine_protocol::ToolResultPart;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolOutput {
    /// Model-facing result (text and images).
    pub content: Vec<ToolResultPart>,
    /// One line for the tool card, e.g. "Read 120 lines".
    pub summary: String,
    /// The call ran but its outcome is a failure the model should see as
    /// such (non-zero exit code, HTTP error, failed check).
    pub is_error: bool,
    pub effects: Effects,
}

/// Side effects the engine tracks: mutations mark the run as changed and
/// refresh read tracking; reads feed nested-instruction reminders.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Effects {
    pub files_written: Vec<PathBuf>,
    pub files_read: Vec<PathBuf>,
    pub ran_command: bool,
}

impl ToolOutput {
    /// A single text part.
    pub fn text(text: impl Into<String>, summary: impl Into<String>) -> Self {
        Self::parts(vec![ToolResultPart::Text { text: text.into() }], summary)
    }

    pub fn parts(content: Vec<ToolResultPart>, summary: impl Into<String>) -> Self {
        Self {
            content,
            summary: summary.into(),
            is_error: false,
            effects: Effects::default(),
        }
    }

    pub fn with_error(mut self, is_error: bool) -> Self {
        self.is_error = is_error;
        self
    }

    pub fn wrote(mut self, path: impl Into<PathBuf>) -> Self {
        self.effects.files_written.push(path.into());
        self
    }

    pub fn read(mut self, path: impl Into<PathBuf>) -> Self {
        self.effects.files_read.push(path.into());
        self
    }

    pub fn ran_command(mut self) -> Self {
        self.effects.ran_command = true;
        self
    }

    /// Every text part joined with newlines (images are skipped).
    pub fn text_content(&self) -> String {
        let texts: Vec<&str> = self
            .content
            .iter()
            .filter_map(|part| match part {
                ToolResultPart::Text { text } => Some(text.as_str()),
                ToolResultPart::Image { .. } => None,
            })
            .collect();
        texts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use z_engine_protocol::MediaSource;

    #[test]
    fn builders_record_effects_and_join_text() {
        let output = ToolOutput::parts(
            vec![
                ToolResultPart::Text { text: "a".into() },
                ToolResultPart::Image {
                    source: MediaSource::Url { url: "x".into() },
                },
                ToolResultPart::Text { text: "b".into() },
            ],
            "done",
        )
        .wrote("/p/a.rs")
        .ran_command()
        .with_error(true);
        assert_eq!(output.text_content(), "a\nb");
        assert_eq!(output.effects.files_written, vec![PathBuf::from("/p/a.rs")]);
        assert!(output.effects.ran_command && output.is_error);
    }
}
