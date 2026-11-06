//! Instruction files (AGENTS.md, CLAUDE.md, rules) rendered for the model.

use z_engine_prompts::reminders::INSTRUCTIONS_PREAMBLE;

/// One instruction file whose contents the caller has already read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionDoc {
    /// Origin label, e.g. `Project instructions (AGENTS.md)`.
    pub label: String,
    pub path: String,
    pub content: String,
}

/// The instructions section: [`INSTRUCTIONS_PREAMBLE`] followed by every
/// non-blank doc under a `# <label> — <path>` heading, in the given order.
/// Callers pass general files first so later, more specific files take
/// precedence. `None` when no doc has content.
pub fn render_instructions(docs: &[InstructionDoc]) -> Option<String> {
    let body = render_docs(docs)?;
    Some(format!("{}\n\n{body}", INSTRUCTIONS_PREAMBLE.trim()))
}

/// Non-blank docs under their headings, or `None` when there are none.
pub(crate) fn render_docs(docs: &[InstructionDoc]) -> Option<String> {
    let rendered: Vec<String> = docs
        .iter()
        .filter(|doc| !doc.content.trim().is_empty())
        .map(|doc| {
            format!(
                "# {} — {}\n\n{}",
                doc.label.trim(),
                doc.path.trim(),
                doc.content.trim()
            )
        })
        .collect();
    (!rendered.is_empty()).then(|| rendered.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(label: &str, path: &str, content: &str) -> InstructionDoc {
        InstructionDoc {
            label: label.into(),
            path: path.into(),
            content: content.into(),
        }
    }

    #[test]
    fn no_block_without_content() {
        assert_eq!(render_instructions(&[]), None);
        assert_eq!(
            render_instructions(&[doc("User instructions", "~/AGENTS.md", " \n")]),
            None
        );
    }

    #[test]
    fn preamble_then_docs_in_order_under_headings() {
        let out = render_instructions(&[
            doc(
                "User instructions",
                "/home/u/.z-engine/AGENTS.md",
                "Be terse.\n",
            ),
            doc("Empty", "/x", ""),
            doc(
                "Project instructions (AGENTS.md)",
                "/work/app/AGENTS.md",
                "Run clippy.",
            ),
        ])
        .unwrap();
        assert!(out.starts_with(INSTRUCTIONS_PREAMBLE.trim()));
        let user = out
            .find("# User instructions — /home/u/.z-engine/AGENTS.md\n\nBe terse.")
            .unwrap();
        let project = out
            .find("# Project instructions (AGENTS.md) — /work/app/AGENTS.md\n\nRun clippy.")
            .unwrap();
        assert!(user < project);
        assert!(!out.contains("# Empty"));
    }
}
