//! Narrowing an automatic check selection to the checks a change can
//! affect (`decisions_check_select`). Path mapping is deterministic: a check
//! whose ecosystem owns a changed file always runs. Only the rest may be
//! skipped, and never when a changed file is itself a test.

use std::path::{Path, PathBuf};

use z_engine_protocol::CheckKind;

use crate::CheckSpec;

const TEST_DIRS: &[&str] = &["test", "tests", "__tests__", "spec", "specs", "testdata"];
const NODE: &[&str] = &[
    "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "svelte", "vue", "astro", "json",
    "jsonc", "css", "scss", "html",
];
const PYTHON: &[&str] = &["py", "pyi", "toml", "cfg", "ini"];
const JVM: &[&str] = &[
    "java",
    "kt",
    "kts",
    "groovy",
    "scala",
    "gradle",
    "xml",
    "properties",
];
const DOTNET: &[&str] = &[
    "cs", "fs", "vb", "csproj", "fsproj", "vbproj", "sln", "props", "targets",
];
const NATIVE: &[&str] = &["c", "h", "cc", "cpp", "cxx", "hh", "hpp", "cmake"];

/// Whether a changed path looks like a test: inside a test folder, or
/// named like one (`test_x.py`, `x_test.go`, `x.test.ts`, `XTest.java`).
pub fn is_test_path(path: &Path) -> bool {
    let in_test_dir = path.parent().is_some_and(|dir| {
        dir.components().any(|part| {
            let part = part.as_os_str().to_string_lossy().to_ascii_lowercase();
            TEST_DIRS.contains(&part.as_str())
        })
    });
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned());
    let (Some(name), Some(stem)) = (name, stem) else {
        return in_test_dir;
    };
    let lower = name.to_ascii_lowercase();
    in_test_dir
        || lower.starts_with("test_")
        || lower.contains(".test.")
        || lower.contains(".spec.")
        || lower == "conftest.py"
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || stem.ends_with("Test")
        || stem.ends_with("Tests")
}

/// Whether any changed path is a test; then every selected check runs.
pub fn touches_tests(changed: &[PathBuf]) -> bool {
    changed.iter().any(|path| is_test_path(path))
}

/// Whether `check`'s ecosystem owns one of the changed paths (a `.rs` file
/// for a `cargo:` check, a `.ts` file for an `npm:` check, ...). Generic
/// runners (`make`, `just`, custom checks) own nothing.
pub fn owns_change(check: &CheckSpec, changed: &[PathBuf]) -> bool {
    let local = check
        .id
        .rsplit_once('/')
        .map_or(check.id.as_str(), |(_, id)| id);
    let tool = local.split_once(':').map_or(local, |(tool, _)| tool);
    changed.iter().any(|path| owned_by(tool, path))
}

fn owned_by(tool: &str, path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let ext = extension.as_str();
    match tool {
        "cargo" => ext == "rs" || name.starts_with("Cargo.") || name.starts_with("rust-toolchain"),
        "npm" | "pnpm" | "yarn" | "bun" | "deno" => NODE.contains(&ext) || name.ends_with(".lock"),
        "go" => ext == "go" || name == "go.mod" || name == "go.sum",
        "python" => PYTHON.contains(&ext) || name.starts_with("requirements"),
        "gradle" | "maven" => JVM.contains(&ext),
        "dotnet" => DOTNET.contains(&ext),
        "cmake" => NATIVE.contains(&ext) || name == "CMakeLists.txt",
        _ => false,
    }
}

/// `selected` without the `skip` ids, in order. In `strict` mode the Test,
/// Build and Typecheck checks stay when skipping would leave none of them.
pub fn skip_checks<'a>(
    selected: Vec<&'a CheckSpec>,
    skip: &[String],
    strict: bool,
) -> Vec<&'a CheckSpec> {
    if skip.is_empty() {
        return selected;
    }
    let kept: Vec<&CheckSpec> = selected
        .iter()
        .copied()
        .filter(|check| !skip.contains(&check.id))
        .collect();
    if !strict || kept.iter().any(|check| proves(check)) {
        return kept;
    }
    selected
        .into_iter()
        .filter(|check| !skip.contains(&check.id) || proves(check))
        .collect()
}

/// The kinds `strict` verification accepts as evidence.
fn proves(check: &CheckSpec) -> bool {
    matches!(
        check.kind,
        CheckKind::Test | CheckKind::Build | CheckKind::Typecheck
    )
}

#[cfg(test)]
#[path = "narrow_tests.rs"]
mod tests;
