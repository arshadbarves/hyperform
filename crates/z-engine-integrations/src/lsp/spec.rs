//! Which language server handles which files: built-in presets for common
//! languages, replaced by name by user configuration.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerSpec {
    /// Unique name; configuration overrides replace presets by name.
    pub name: String,
    /// Executable, looked up on PATH unless it is a path.
    pub command: String,
    pub args: Vec<String>,
    /// File extensions without the dot, lowercase.
    pub extensions: Vec<String>,
    /// Files marking a workspace root; empty means "any file qualifies and
    /// the project root is the workspace".
    pub root_markers: Vec<String>,
    /// `languageId` per extension; others use [`default_language_id`].
    pub language_ids: BTreeMap<String, String>,
}

impl LspServerSpec {
    pub fn new(
        name: &str,
        command: &str,
        args: &[&str],
        extensions: &[&str],
        markers: &[&str],
    ) -> Self {
        let owned = |items: &[&str]| items.iter().map(|item| (*item).to_string()).collect();
        Self {
            name: name.to_string(),
            command: command.to_string(),
            args: owned(args),
            extensions: owned(extensions),
            root_markers: owned(markers),
            language_ids: BTreeMap::new(),
        }
    }

    pub fn handles(&self, extension: &str) -> bool {
        self.extensions
            .iter()
            .any(|ext| ext.eq_ignore_ascii_case(extension))
    }

    /// The `languageId` sent with `didOpen` for files with `extension`.
    pub fn language_id(&self, extension: &str) -> String {
        let extension = extension.to_ascii_lowercase();
        self.language_ids
            .get(&extension)
            .cloned()
            .unwrap_or_else(|| default_language_id(&extension).to_string())
    }
}

/// Built-in servers, in preference order: for an extension claimed by
/// several (pyright, basedpyright) the first one installed wins.
pub fn presets() -> Vec<LspServerSpec> {
    vec![
        LspServerSpec::new(
            "rust-analyzer",
            "rust-analyzer",
            &[],
            &["rs"],
            &["Cargo.toml", "rust-project.json"],
        ),
        LspServerSpec::new(
            "typescript-language-server",
            "typescript-language-server",
            &["--stdio"],
            &["ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts"],
            &[],
        ),
        LspServerSpec::new("pyright", "pyright-langserver", &["--stdio"], &["py"], &[]),
        LspServerSpec::new(
            "basedpyright",
            "basedpyright-langserver",
            &["--stdio"],
            &["py"],
            &[],
        ),
        LspServerSpec::new("gopls", "gopls", &[], &["go"], &["go.mod", "go.work"]),
        LspServerSpec::new(
            "clangd",
            "clangd",
            &[],
            &["c", "h", "cc", "cpp", "cxx", "hpp"],
            &[],
        ),
    ]
}

/// Overrides replace presets with the same name in place; new servers come
/// first so user servers win the extensions they claim.
pub fn merge_specs(
    presets: Vec<LspServerSpec>,
    overrides: Vec<LspServerSpec>,
) -> Vec<LspServerSpec> {
    let (replacing, added): (Vec<_>, Vec<_>) = overrides
        .into_iter()
        .partition(|spec| presets.iter().any(|preset| preset.name == spec.name));
    let mut merged = added;
    merged.extend(presets.into_iter().map(|preset| {
        replacing
            .iter()
            .find(|spec| spec.name == preset.name)
            .cloned()
            .unwrap_or(preset)
    }));
    merged
}

/// The conventional `languageId` of a lowercase extension.
pub fn default_language_id(extension: &str) -> &str {
    match extension {
        "rs" => "rust",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "py" => "python",
        "go" => "go",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" => "cpp",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_cover_the_documented_languages() {
        let specs = presets();
        let names: Vec<_> = specs.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "rust-analyzer",
                "typescript-language-server",
                "pyright",
                "basedpyright",
                "gopls",
                "clangd"
            ]
        );
        let ts = &specs[1];
        assert!(ts.handles("tsx") && ts.handles("CTS") && ts.args == ["--stdio"]);
        assert_eq!(ts.language_id("tsx"), "typescriptreact");
        assert_eq!(specs[5].language_id("hpp"), "cpp");
        assert_eq!(specs[0].root_markers[0], "Cargo.toml");
    }

    #[test]
    fn overrides_replace_by_name_and_new_servers_come_first() {
        let mut custom_rust = LspServerSpec::new("rust-analyzer", "/opt/ra", &[], &["rs"], &[]);
        custom_rust.language_ids.insert("rs".into(), "rust2".into());
        let ruff = LspServerSpec::new("ruff", "ruff", &["server"], &["py"], &[]);
        let merged = merge_specs(presets(), vec![custom_rust, ruff]);
        assert_eq!(merged.len(), presets().len() + 1);
        assert_eq!(merged[0].name, "ruff");
        assert_eq!(merged[1].command, "/opt/ra");
        assert!(merged[1].root_markers.is_empty());
        assert_eq!(merged[1].language_id("rs"), "rust2");
    }
}
