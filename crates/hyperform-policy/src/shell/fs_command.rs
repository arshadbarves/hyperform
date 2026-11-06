//! Filesystem commands that `acceptEdits` mode runs without asking.

use std::path::{Path, PathBuf};

use super::analysis::parse;
use super::location::{Location, locate};
use super::operands::operands;
use super::read_only::segment_is_read_only;
use super::sed;
use super::syntax::{Redirect, Word};
use crate::paths::{is_protected, normalize};

const MUTATORS: &[&str] = &["mkdir", "touch", "rm", "rmdir", "mv", "cp"];

/// True when every segment is a common filesystem change (`mkdir`, `touch`,
/// `rm`, `rmdir`, `mv`, `cp`, `sed -i`) whose operands stay inside
/// `project_root`, a `cd` that stays inside it, or a read-only command reading
/// inside it. Relative operands resolve against the project root, or the
/// directory an earlier `cd` in the same line moved to, so the caller must
/// run the command from inside the project. Operands that expand at run time
/// (`*`, `{a,b}`, `$x`, `~`), climb with `..`, name the project root itself
/// for `rm`/`rmdir`/`mv`, or touch `.git` and harness settings never qualify,
/// and neither do file-writing redirects.
pub fn is_common_fs_command(command: &str, project_root: &Path) -> bool {
    let parsed = parse(command);
    if !parsed.ok || parsed.dynamic || parsed.segments.is_empty() {
        return false;
    }
    let root = normalize(project_root);
    let mut cwd = root.clone();
    parsed.segments.iter().all(|segment| {
        if segment.redirects.iter().any(Redirect::writes_file) {
            return false;
        }
        let words = segment.command();
        let Some((name, args)) = words.split_first() else {
            return true;
        };
        if name.assignment || name.expands() {
            return false;
        }
        match name.text.as_str() {
            "cd" => change_directory(&mut cwd, args, &root),
            "sed" => match sed::parse_call(args) {
                Some(call) if call.in_place => {
                    call.files.iter().all(|file| writable(file, &cwd, &root))
                }
                Some(_) => reads_inside(words, &cwd, &root),
                None => false,
            },
            name if MUTATORS.contains(&name) => mutation_contained(name, args, &cwd, &root),
            _ => segment_is_read_only(segment) && reads_inside(words, &cwd, &root),
        }
    })
}

fn mutation_contained(name: &str, args: &[Word], cwd: &Path, root: &Path) -> bool {
    let mut options = true;
    let mut targets = Vec::new();
    for word in args {
        if word.expands() {
            return false;
        }
        let text = word.text.as_str();
        if options && text == "--" {
            options = false;
        } else if options && text.starts_with('-') && text.len() > 1 {
            if let Some((_, value)) = text.split_once('=').filter(|(_, v)| v.contains('/')) {
                targets.push(Word {
                    text: value.to_string(),
                    ..word.clone()
                });
            }
        } else {
            targets.push(word.clone());
        }
    }
    let removes_root = matches!(name, "rm" | "rmdir" | "mv");
    targets.iter().all(|target| {
        contained(target, cwd, root).is_some_and(|path| {
            let refused = is_protected(&path, [root]) || (removes_root && path == root);
            !refused
        })
    })
}

fn change_directory(cwd: &mut PathBuf, args: &[Word], root: &Path) -> bool {
    let targets: Vec<&Word> = args
        .iter()
        .filter(|word| !matches!(word.text.as_str(), "-L" | "-P" | "-e" | "-@"))
        .collect();
    let [target] = targets.as_slice() else {
        return false;
    };
    if target.text == "-" {
        return false;
    }
    match contained(target, cwd, root) {
        Some(path) => {
            *cwd = path;
            true
        }
        None => false,
    }
}

fn reads_inside(words: &[Word], cwd: &Path, root: &Path) -> bool {
    operands(words)
        .reads
        .iter()
        .all(|word| contained(word, cwd, root).is_some())
}

fn writable(word: &Word, cwd: &Path, root: &Path) -> bool {
    contained(word, cwd, root).is_some_and(|path| !is_protected(&path, [root]))
}

/// The operand's path when it provably stays inside `root`.
fn contained(word: &Word, cwd: &Path, root: &Path) -> Option<PathBuf> {
    let path = match locate(word, None) {
        Location::Relative {
            path,
            escapes: false,
        } => normalize(&cwd.join(path)),
        Location::Absolute(path) => path,
        _ => return None,
    };
    path.starts_with(root).then_some(path)
}
