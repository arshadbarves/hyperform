//! Instruction files as labelled prompt docs, and discovery of nested
//! instruction files for directories an agent touches.

use std::collections::HashSet;
use std::path::Path;

use z_engine_config::{
    Extensions, InstructionFile, InstructionScope, Paths, discover_instructions,
    nested_instructions,
};
use z_engine_context::InstructionDoc;

/// User, project and local files (lowest precedence first), then rules
/// marked `alwaysApply`.
pub(crate) fn instruction_docs(
    paths: &Paths,
    root: &Path,
    compat_claude: bool,
    extensions: &Extensions,
) -> Vec<InstructionDoc> {
    let mut docs: Vec<InstructionDoc> = discover_instructions(paths, root, compat_claude)
        .into_iter()
        .map(doc_of)
        .collect();
    docs.extend(
        extensions
            .rules
            .iter()
            .filter(|rule| rule.always_apply && !rule.body.trim().is_empty())
            .map(|rule| InstructionDoc {
                label: format!("Rule ({})", rule.name),
                path: rule.source.path.clone(),
                content: rule.body.clone(),
            }),
    );
    docs
}

/// Nested instruction files for `files` not in `seen`; newly found paths
/// are added to `seen` so each file is announced once per agent.
pub(crate) fn nested_docs<'a>(
    root: &Path,
    files: impl IntoIterator<Item = &'a Path>,
    compat_claude: bool,
    seen: &mut HashSet<String>,
) -> Vec<InstructionDoc> {
    let mut docs = Vec::new();
    for file in files {
        for found in nested_instructions(root, file, compat_claude) {
            if seen.insert(found.path.clone()) {
                docs.push(doc_of(found));
            }
        }
    }
    docs
}

fn doc_of(file: InstructionFile) -> InstructionDoc {
    let name = Path::new(&file.path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let scope = match file.scope {
        InstructionScope::User => "User instructions",
        InstructionScope::Project => "Project instructions",
        InstructionScope::Local => "Local instructions",
        InstructionScope::Nested => "Directory instructions",
    };
    InstructionDoc {
        label: format!("{scope} ({name})"),
        path: file.path,
        content: file.content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_name_scope_and_file() {
        let doc = doc_of(InstructionFile {
            scope: InstructionScope::User,
            path: "/cfg/AGENTS.md".into(),
            content: "x".into(),
        });
        assert_eq!(doc.label, "User instructions (AGENTS.md)");
        let doc = doc_of(InstructionFile {
            scope: InstructionScope::Nested,
            path: "/p/src/CLAUDE.md".into(),
            content: "y".into(),
        });
        assert_eq!(doc.label, "Directory instructions (CLAUDE.md)");
    }

    #[test]
    fn nested_files_are_announced_once() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src/deep")).unwrap();
        std::fs::write(root.join("src/AGENTS.md"), "src rules").unwrap();
        let mut seen = HashSet::new();
        let file = root.join("src/deep/lib.rs");
        let first = nested_docs(root, [file.as_path()], false, &mut seen);
        assert_eq!(first.len(), 1);
        assert!(first[0].content.contains("src rules"));
        assert!(nested_docs(root, [file.as_path()], false, &mut seen).is_empty());
    }
}
