//! Extension files (agents, commands, skills, rules, output styles) and
//! instruction files (`AGENTS.md`, ...). Paths from the webview must stay
//! in the folders the engine reads them from; writes reload sessions.

use std::path::{Path, PathBuf};

use hyperform_config::{
    ExtensionScope, Extensions, InstructionFile, Paths, discover_extensions, discover_instructions,
    project_dir,
};
use tauri::State;

use crate::guard::{
    ExtensionKind, extension_bases, extension_file, extension_target, instruction_file,
    remove_extension,
};
use crate::ipc::{IpcResult, fail};
use crate::state::AppState;

/// Extension files larger than the engine reads are refused.
const MAX_FILE_BYTES: usize = 256 * 1024;

/// `project_root` null lists user-level extensions only.
#[tauri::command]
pub(crate) fn list_extensions(
    project_root: Option<String>,
    state: State<'_, AppState>,
) -> Extensions {
    let paths = state.engine.paths();
    let root = project_root.map(PathBuf::from);
    let compat = state
        .engine
        .settings(root.as_deref())
        .settings
        .compat
        .claude;
    match root {
        Some(root) => discover_extensions(paths, &root, compat),
        None => user_only(discover_extensions(paths, &no_project(paths), compat)),
    }
}

#[tauri::command]
pub(crate) fn read_extension_file(path: String, state: State<'_, AppState>) -> IpcResult<String> {
    let bases = extension_bases(state.engine.paths(), &state.known_roots());
    let file = extension_file(&path, &bases)?;
    std::fs::read_to_string(&file).map_err(|error| format!("{}: {error}", file.display()))
}

/// Writes `<kind>/<name>.md` (or `skills/<name>/SKILL.md`) at the user or
/// project level and returns the path written.
#[tauri::command]
pub(crate) fn write_extension_file(
    project_root: Option<String>,
    kind: ExtensionKind,
    name: String,
    content: String,
    state: State<'_, AppState>,
) -> IpcResult<String> {
    let root = project_root.map(PathBuf::from);
    let base = match &root {
        Some(root) if is_known(root, &state.known_roots()) => project_dir(root),
        Some(root) => return Err(format!("{} is not an open workspace", root.display())),
        None => state.engine.paths().config_dir.clone(),
    };
    let file = extension_target(&base, kind, &name)?;
    write_text(&file, &content)?;
    state.engine.reload_settings(root.as_deref());
    Ok(file.to_string_lossy().into_owned())
}

#[tauri::command]
pub(crate) fn delete_extension_file(path: String, state: State<'_, AppState>) -> IpcResult<()> {
    let bases = extension_bases(state.engine.paths(), &state.known_roots());
    remove_extension(&extension_file(&path, &bases)?)?;
    state.engine.reload_settings(None);
    Ok(())
}

/// `project_root` null lists the user file only.
#[tauri::command]
pub(crate) fn list_instruction_files(
    project_root: Option<String>,
    state: State<'_, AppState>,
) -> Vec<InstructionFile> {
    let paths = state.engine.paths();
    let root = project_root
        .map(PathBuf::from)
        .unwrap_or_else(|| no_project(paths));
    let compat = state.engine.settings(Some(&root)).settings.compat.claude;
    discover_instructions(paths, &root, compat)
}

#[tauri::command]
pub(crate) fn write_instruction_file(
    path: String,
    content: String,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    let paths = state.engine.paths();
    let roots = state.known_roots();
    let mut dirs = vec![paths.config_dir.clone()];
    dirs.extend(paths.claude_user_dir());
    dirs.extend(roots.iter().cloned());
    let listed: Vec<String> = roots
        .iter()
        .flat_map(|root| discover_instructions(paths, root, true))
        .map(|file| file.path)
        .collect();
    let file = instruction_file(&path, &dirs, &listed)?;
    write_text(&file, &content)?;
    state.engine.reload_settings(None);
    Ok(())
}

fn is_known(root: &Path, known: &[PathBuf]) -> bool {
    let canonical =
        |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let root = canonical(root);
    known.iter().any(|known| canonical(known) == root)
}

/// A project root that does not exist, so discovery reads user folders only.
fn no_project(paths: &Paths) -> PathBuf {
    paths.config_dir.join(".hyperform-no-project")
}

fn user_only(mut extensions: Extensions) -> Extensions {
    let user =
        |scope: ExtensionScope| matches!(scope, ExtensionScope::User | ExtensionScope::ClaudeUser);
    extensions.agents.retain(|def| user(def.source.scope));
    extensions.commands.retain(|def| user(def.source.scope));
    extensions.skills.retain(|def| user(def.source.scope));
    extensions.rules.retain(|def| user(def.source.scope));
    extensions
        .output_styles
        .retain(|def| user(def.source.scope));
    extensions
}

/// Creates parent folders and replaces the file through a temp file.
fn write_text(file: &Path, content: &str) -> IpcResult<()> {
    if content.len() > MAX_FILE_BYTES {
        return Err(format!("{} is larger than 256 KiB", file.display()));
    }
    let parent = file.parent().ok_or("the file has no folder")?;
    std::fs::create_dir_all(parent).map_err(fail)?;
    let tmp = file.with_extension("md.tmp");
    std::fs::write(&tmp, content).map_err(fail)?;
    std::fs::rename(&tmp, file).map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_level_listing_ignores_projects() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::with_roots(dir.path().join("config"), dir.path().join("data"));
        let agents = paths.config_dir.join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(agents.join("a.md"), "---\ndescription: A\n---\nBody\n").unwrap();
        let found = user_only(discover_extensions(&paths, &no_project(&paths), false));
        assert_eq!(found.agents.len(), 1);
        assert!(found.errors.is_empty(), "{:?}", found.errors);
        let instructions = discover_instructions(&paths, &no_project(&paths), false);
        assert!(
            instructions
                .iter()
                .all(|file| !file.path.contains("no-project"))
        );
    }

    #[test]
    fn writes_replace_files_and_refuse_oversized_content() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("agents/sub/a.md");
        write_text(&file, "one").unwrap();
        write_text(&file, "two").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
        assert!(write_text(&file, &"x".repeat(MAX_FILE_BYTES + 1)).is_err());
    }
}
