//! Instruction files (`AGENTS.md`; also `CLAUDE.md` in Claude compat mode)
//! that become part of the system prompt, in precedence order.
//!
//! At every level the Claude file comes before the native one, so native
//! instructions take precedence. A file that resolves to one already listed
//! (e.g. `CLAUDE.md` symlinked to `AGENTS.md`) is listed once, at the later
//! position. Project files resolving outside the walked directories are
//! skipped.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::files::{lossy_text, read_capped};
use crate::paths::Paths;

/// Instruction content beyond this is cut, with [`TRUNCATION_NOTE`] appended.
pub const INSTRUCTION_FILE_LIMIT: usize = 64 * 1024;
pub const TRUNCATION_NOTE: &str = "\n\n[Truncated: this file is larger than 64 KiB.]";

const NATIVE: &str = "AGENTS.md";
const CLAUDE: &str = "CLAUDE.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub enum InstructionScope {
    /// The z-engine config directory (and `~/.claude` in compat mode).
    User,
    /// The project root and its ancestors up to the repository root.
    Project,
    /// Personal `AGENTS.local.md` in the project root.
    Local,
    /// Directories between the project root and a file being worked on.
    Nested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct InstructionFile {
    pub scope: InstructionScope,
    pub path: String,
    pub content: String,
}

/// User, then project directories from the outermost down to
/// `project_root`, then local files; lowest precedence first. The walk up
/// from `project_root` stops after the first directory containing `.git`,
/// and never includes the home directory or the filesystem root.
pub fn discover_instructions(
    paths: &Paths,
    project_root: &Path,
    compat_claude: bool,
) -> Vec<InstructionFile> {
    let mut list = List::default();
    if let Some(dir) = paths.claude_user_dir().filter(|_| compat_claude) {
        list.add(InstructionScope::User, &dir.join(CLAUDE), None);
    }
    list.add(InstructionScope::User, &paths.config_dir.join(NATIVE), None);
    let chain = project_chain(project_root, paths.home_dir.as_deref());
    let Some(boundary) = resolve_boundary(&chain[0]) else {
        return list.finish();
    };
    for dir in &chain {
        let names = [CLAUDE, NATIVE];
        list.pair(
            InstructionScope::Project,
            dir,
            names,
            compat_claude,
            &boundary,
        );
    }
    let local = ["CLAUDE.local.md", "AGENTS.local.md"];
    list.pair(
        InstructionScope::Local,
        project_root,
        local,
        compat_claude,
        &boundary,
    );
    list.finish()
}

/// Instruction files in directories strictly below `project_root`, down to
/// the directory containing `file` (absolute or relative to the root).
pub fn nested_instructions(
    project_root: &Path,
    file: &Path,
    compat_claude: bool,
) -> Vec<InstructionFile> {
    let file = if file.is_absolute() {
        file.to_path_buf()
    } else {
        project_root.join(file)
    };
    let Some(relative) = file
        .parent()
        .and_then(|dir| dir.strip_prefix(project_root).ok())
    else {
        return Vec::new();
    };
    let Some(boundary) = resolve_boundary(project_root) else {
        return Vec::new();
    };
    let mut list = List::default();
    let mut dir = project_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Vec::new();
        };
        dir.push(part);
        let names = [CLAUDE, NATIVE];
        list.pair(
            InstructionScope::Nested,
            &dir,
            names,
            compat_claude,
            &boundary,
        );
    }
    list.finish()
}

/// The canonical directory project files must resolve inside; when it
/// cannot be resolved, project files are not read at all.
fn resolve_boundary(dir: &Path) -> Option<PathBuf> {
    match fs::canonicalize(dir) {
        Ok(boundary) => Some(boundary),
        Err(error) => {
            if error.kind() != io::ErrorKind::NotFound {
                tracing::warn!(path = %dir.display(), %error, "cannot resolve the project root; project instructions skipped");
            }
            None
        }
    }
}

/// `project_root` and the ancestors that belong to its repository,
/// outermost first.
fn project_chain(project_root: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let mut chain = vec![project_root.to_path_buf()];
    let mut current = project_root;
    while !current.join(".git").exists() && Some(current) != home {
        let Some(parent) = current.parent() else {
            break;
        };
        if Some(parent) == home || parent.parent().is_none() {
            break;
        }
        chain.push(parent.to_path_buf());
        current = parent;
    }
    chain.reverse();
    chain
}

#[derive(Default)]
struct List {
    files: Vec<(Option<PathBuf>, InstructionFile)>,
}

impl List {
    /// A project-side pair: the Claude file (in compat mode), then the native one.
    fn pair(
        &mut self,
        scope: InstructionScope,
        dir: &Path,
        [claude, native]: [&str; 2],
        compat: bool,
        boundary: &Path,
    ) {
        if compat {
            self.add(scope, &dir.join(claude), Some(boundary));
        }
        self.add(scope, &dir.join(native), Some(boundary));
    }

    /// `boundary` is `None` only for user-owned files.
    fn add(&mut self, scope: InstructionScope, path: &Path, boundary: Option<&Path>) {
        let Some(content) = read_instructions(path) else {
            return;
        };
        let real = fs::canonicalize(path).ok();
        if let Some(boundary) = boundary {
            if !real.as_ref().is_some_and(|real| real.starts_with(boundary)) {
                tracing::warn!(path = %path.display(), "instruction file resolves outside the project; skipped");
                return;
            }
        }
        if real.is_some() {
            self.files.retain(|(seen, _)| seen != &real);
        }
        let path = path.to_string_lossy().into_owned();
        self.files.push((
            real,
            InstructionFile {
                scope,
                path,
                content,
            },
        ));
    }

    fn finish(self) -> Vec<InstructionFile> {
        self.files.into_iter().map(|(_, file)| file).collect()
    }
}

/// The file's text, `None` when missing, unreadable, or blank.
fn read_instructions(path: &Path) -> Option<String> {
    let (bytes, truncated) = match read_capped(path, INSTRUCTION_FILE_LIMIT) {
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "instruction file is unreadable; skipped");
            return None;
        }
    };
    let mut content = lossy_text(bytes);
    if content.trim().is_empty() {
        return None;
    }
    if truncated {
        content.push_str(TRUNCATION_NOTE);
    }
    Some(content)
}
