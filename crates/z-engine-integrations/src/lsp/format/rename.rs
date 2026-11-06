//! A rename preview as a per-file list of edits; nothing is applied.

use std::path::Path;

use z_engine_host::relative_display;

use super::super::types::WorkspaceEditPlan;

pub fn format_rename_plan(root: &Path, plan: &WorkspaceEditPlan) -> String {
    if plan.files.is_empty() {
        return "The rename changes nothing.".to_string();
    }
    let edits = plan.edit_count();
    let mut lines = vec![format!(
        "Rename preview (not applied): {edits} edit{} in {} file{}",
        plural(edits),
        plan.files.len(),
        plural(plan.files.len())
    )];
    for file in &plan.files {
        lines.push(format!(
            "{} ({} edit{})",
            relative_display(root, &file.path),
            file.edits.len(),
            plural(file.edits.len())
        ));
        lines.extend(file.edits.iter().map(|edit| {
            format!(
                "  {}:{}-{}:{} -> {:?}",
                edit.start_line, edit.start_col, edit.end_line, edit.end_col, edit.new_text
            )
        }));
    }
    lines.join("\n")
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::lsp::types::{FileEdits, TextEditPlan};

    fn edit(line: u32, start: u32, end: u32) -> TextEditPlan {
        TextEditPlan {
            start_line: line,
            start_col: start,
            end_line: line,
            end_col: end,
            new_text: "welcome".into(),
        }
    }

    #[test]
    fn lists_edits_per_file() {
        let plan = WorkspaceEditPlan {
            files: vec![
                FileEdits {
                    path: PathBuf::from("/proj/src/lib.rs"),
                    edits: vec![edit(5, 4, 9), edit(9, 5, 10)],
                },
                FileEdits {
                    path: PathBuf::from("/proj/src/main.rs"),
                    edits: vec![edit(2, 1, 6)],
                },
            ],
        };
        assert_eq!(
            format_rename_plan(Path::new("/proj"), &plan),
            "Rename preview (not applied): 3 edits in 2 files\n\
             src/lib.rs (2 edits)\n  5:4-5:9 -> \"welcome\"\n  9:5-9:10 -> \"welcome\"\n\
             src/main.rs (1 edit)\n  2:1-2:6 -> \"welcome\""
        );
        let empty = WorkspaceEditPlan { files: Vec::new() };
        assert_eq!(
            format_rename_plan(Path::new("/proj"), &empty),
            "The rename changes nothing."
        );
    }
}
