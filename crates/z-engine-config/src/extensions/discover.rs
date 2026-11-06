//! Finds user-authored extensions. Roots are scanned from lowest to highest
//! precedence (`~/.claude`, user config dir, `<project>/.claude`,
//! `<project>/.z-engine`), so a later definition replaces a same-named one.
//! Project files that resolve outside the project (via symlinks) are
//! skipped, so a repository cannot pull arbitrary local files into prompts.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::agent::parse_agent;
use super::command::parse_command;
use super::output_style::parse_output_style;
use super::rule::parse_rule;
use super::skill::{SKILL_FILE, parse_skill};
use super::{
    AgentDef, CommandDef, ExtensionError, ExtensionScope, ExtensionSource, Extensions,
    OutputStyleDef, RuleDef, SkillDef,
};
use crate::files::{lossy_text, read_capped};
use crate::paths::{Paths, project_dir};

/// Extension files larger than this are skipped with an error.
pub const EXTENSION_FILE_LIMIT: usize = 256 * 1024;
const MAX_DEPTH: usize = 8;
const MAX_FILES: usize = 1000;

struct Root {
    scope: ExtensionScope,
    base: PathBuf,
    /// Canonical directory that project files must resolve inside.
    boundary: Option<PathBuf>,
}

pub fn discover_extensions(paths: &Paths, project_root: &Path, compat_claude: bool) -> Extensions {
    let mut found = Found::default();
    let mut roots = Vec::new();
    let root = |scope, base, boundary| Root {
        scope,
        base,
        boundary,
    };
    if let Some(base) = paths.claude_user_dir().filter(|_| compat_claude) {
        roots.push(root(ExtensionScope::ClaudeUser, base, None));
    }
    roots.push(root(ExtensionScope::User, paths.config_dir.clone(), None));
    match fs::canonicalize(project_root) {
        Ok(boundary) => {
            if compat_claude {
                let base = project_root.join(".claude");
                roots.push(root(
                    ExtensionScope::ClaudeProject,
                    base,
                    Some(boundary.clone()),
                ));
            }
            let base = project_dir(project_root);
            roots.push(root(ExtensionScope::Project, base, Some(boundary)));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            let message = format!("cannot resolve the project root: {error}");
            found
                .errors
                .push(ExtensionError::new(project_root, message));
        }
    }
    for root in &roots {
        found.scan(root);
    }
    found.into_extensions()
}

type Parser<T> = fn(&str, &str, ExtensionSource) -> Result<T, String>;

#[derive(Default)]
struct Found {
    agents: BTreeMap<String, AgentDef>,
    commands: BTreeMap<String, CommandDef>,
    skills: BTreeMap<String, SkillDef>,
    rules: BTreeMap<String, RuleDef>,
    output_styles: BTreeMap<String, OutputStyleDef>,
    errors: Vec<ExtensionError>,
}

impl Found {
    /// Rules and output styles exist only in the native folders.
    fn scan(&mut self, root: &Root) {
        self.scan_markdown(
            root,
            "agents",
            |_, text, source| parse_agent(text, source),
            |f| &mut f.agents,
        );
        self.scan_markdown(root, "commands", parse_command, |f| &mut f.commands);
        self.scan_skills(root);
        if matches!(root.scope, ExtensionScope::User | ExtensionScope::Project) {
            self.scan_markdown(root, "rules", parse_rule, |f| &mut f.rules);
            let parse = |_: &str, text: &str, source| parse_output_style(text, source);
            self.scan_markdown(root, "output-styles", parse, |f| &mut f.output_styles);
        }
    }

    /// Parses every `*.md` below `root.base/dir`; the parser receives the
    /// path-derived name (`sub/name.md` -> `sub:name`).
    fn scan_markdown<T: Definition>(
        &mut self,
        root: &Root,
        dir: &str,
        parse: Parser<T>,
        map: fn(&mut Self) -> &mut BTreeMap<String, T>,
    ) {
        for (path, name) in self.markdown_files(&root.base.join(dir)) {
            if let Some(text) = self.read(root, &path) {
                let parsed = parse(&name, &text, ExtensionSource::new(root.scope, &path));
                self.add(parsed, &path, map);
            }
        }
    }

    fn scan_skills(&mut self, root: &Root) {
        for dir in self.entries(&root.base.join("skills")) {
            let path = dir.join(SKILL_FILE);
            if !dir.is_dir() || !path.is_file() {
                continue;
            }
            if let Some(text) = self.read(root, &path) {
                let parsed = parse_skill(&dir, &text, ExtensionSource::new(root.scope, &path));
                self.add(parsed, &path, |f| &mut f.skills);
            }
        }
    }

    /// Inserts a parsed definition; a duplicate name within one scope keeps
    /// the first file and reports the second.
    fn add<T: Definition>(
        &mut self,
        parsed: Result<T, String>,
        path: &Path,
        map: fn(&mut Self) -> &mut BTreeMap<String, T>,
    ) {
        let def = match parsed {
            Ok(def) => def,
            Err(message) => return self.errors.push(ExtensionError::new(path, message)),
        };
        let name = def.name().to_string();
        let existing = map(self).get(&name).map(|old| old.source().clone());
        match existing {
            Some(old) if old.scope == def.source().scope => {
                let message = format!("duplicate name `{name}`; {} is used", old.path);
                self.errors.push(ExtensionError::new(path, message));
            }
            _ => {
                map(self).insert(name, def);
            }
        }
    }

    fn read(&mut self, root: &Root, path: &Path) -> Option<String> {
        if let Some(boundary) = &root.boundary {
            if !fs::canonicalize(path).is_ok_and(|real| real.starts_with(boundary)) {
                let message = "resolves outside the project; skipped";
                self.errors.push(ExtensionError::new(path, message));
                return None;
            }
        }
        let message = match read_capped(path, EXTENSION_FILE_LIMIT) {
            Ok((bytes, false)) => return Some(lossy_text(bytes)),
            Ok((_, true)) => format!("larger than {} KiB; skipped", EXTENSION_FILE_LIMIT / 1024),
            Err(error) => format!("cannot read: {error}"),
        };
        self.errors.push(ExtensionError::new(path, message));
        None
    }

    /// Sorted entries of a directory without hidden names; missing is empty.
    fn entries(&mut self, dir: &Path) -> Vec<PathBuf> {
        let listing = match fs::read_dir(dir) {
            Ok(listing) => listing,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
            Err(error) => {
                self.errors
                    .push(ExtensionError::new(dir, format!("cannot list: {error}")));
                return Vec::new();
            }
        };
        let mut paths = Vec::new();
        for entry in listing {
            match entry {
                Ok(entry) if entry.file_name().to_string_lossy().starts_with('.') => {}
                Ok(entry) => paths.push(entry.path()),
                Err(error) => self
                    .errors
                    .push(ExtensionError::new(dir, format!("cannot list: {error}"))),
            }
        }
        paths.sort();
        paths
    }

    fn markdown_files(&mut self, dir: &Path) -> Vec<(PathBuf, String)> {
        let mut files = Vec::new();
        if !self.walk(dir, "", 0, &mut files) {
            let message = format!("more than {MAX_FILES} files; the rest were skipped");
            self.errors.push(ExtensionError::new(dir, message));
        }
        files
    }

    /// Returns false once [`MAX_FILES`] files were collected.
    fn walk(
        &mut self,
        dir: &Path,
        prefix: &str,
        depth: usize,
        files: &mut Vec<(PathBuf, String)>,
    ) -> bool {
        let named = |part: &std::ffi::OsStr| match prefix {
            "" => part.to_string_lossy().into_owned(),
            prefix => format!("{prefix}:{}", part.to_string_lossy()),
        };
        for path in self.entries(dir) {
            if path.is_dir() {
                let Some(dir_name) = path.file_name().map(named) else {
                    continue;
                };
                if depth < MAX_DEPTH && !self.walk(&path, &dir_name, depth + 1, files) {
                    return false;
                }
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            {
                let Some(name) = path.file_stem().map(named) else {
                    continue;
                };
                if files.len() >= MAX_FILES {
                    return false;
                }
                files.push((path, name));
            }
        }
        true
    }

    fn into_extensions(self) -> Extensions {
        Extensions {
            agents: self.agents.into_values().collect(),
            commands: self.commands.into_values().collect(),
            skills: self.skills.into_values().collect(),
            rules: self.rules.into_values().collect(),
            output_styles: self.output_styles.into_values().collect(),
            errors: self.errors,
        }
    }
}

trait Definition {
    fn name(&self) -> &str;
    fn source(&self) -> &ExtensionSource;
}

macro_rules! definition {
    ($($def:ty),*) => {
        $(impl Definition for $def {
            fn name(&self) -> &str {
                &self.name
            }

            fn source(&self) -> &ExtensionSource {
                &self.source
            }
        })*
    };
}

definition!(AgentDef, CommandDef, SkillDef, RuleDef, OutputStyleDef);
