//! Python: one root per directory with a Python manifest. pytest runs when
//! configured (pyproject `[tool.pytest*]`, `pytest.ini`, `[pytest]` in
//! tox.ini, `[tool:pytest]` in setup.cfg) or implied (`conftest.py`, a
//! `tests/` directory); ruff/flake8 lint and mypy/pyright typecheck when
//! configured. Tools run through `uv run` next to `uv.lock`, `poetry run`
//! next to `poetry.lock`, else the project's `.venv` or system Python.

use std::collections::BTreeSet;

use z_engine_protocol::CheckKind;

use crate::discovery::collector::{Collector, brief};
use crate::discovery::rel;
use crate::discovery::walk::Found;

/// In order of preference as the root's defining manifest.
const MANIFESTS: &[&str] = &[
    "pyproject.toml",
    "setup.cfg",
    "setup.py",
    "pytest.ini",
    "tox.ini",
    "requirements.txt",
];

#[cfg(windows)]
const VENV_PYTHON: &str = r".venv\Scripts\python.exe";
#[cfg(not(windows))]
const VENV_PYTHON: &str = ".venv/bin/python";
const SYSTEM_PYTHON: &str = if cfg!(windows) { "python" } else { "python3" };

pub(crate) fn is_manifest(name: &str) -> bool {
    MANIFESTS.contains(&name)
}

/// Declaration order is check order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tool {
    Pytest,
    Mypy,
    Pyright,
    Ruff,
    Flake8,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Pytest => "pytest",
            Self::Mypy => "mypy",
            Self::Pyright => "pyright",
            Self::Ruff => "ruff",
            Self::Flake8 => "flake8",
        }
    }

    fn kind(self) -> CheckKind {
        match self {
            Self::Pytest => CheckKind::Test,
            Self::Mypy | Self::Pyright => CheckKind::Typecheck,
            Self::Ruff | Self::Flake8 => CheckKind::Lint,
        }
    }

    /// The tool's arguments; `pyright` is not a Python module.
    fn invocation(self) -> (&'static str, bool) {
        match self {
            Self::Pytest => ("pytest", true),
            Self::Mypy => ("mypy .", true),
            Self::Pyright => ("pyright", false),
            Self::Ruff => ("ruff check .", true),
            Self::Flake8 => ("flake8", true),
        }
    }
}

enum Runner {
    Uv,
    Poetry,
    Python(&'static str),
}

impl Runner {
    fn command(&self, tool: Tool) -> String {
        let (args, module) = tool.invocation();
        match self {
            Self::Uv => format!("uv run {args}"),
            Self::Poetry => format!("poetry run {args}"),
            Self::Python(python) if module => format!("{python} -m {args}"),
            Self::Python(_) => args.to_string(),
        }
    }
}

pub(crate) fn discover(c: &mut Collector, found: &[Found]) {
    let mut dirs: Vec<&str> = found.iter().map(|f| f.dir.as_str()).collect();
    dirs.dedup();
    for dir in dirs {
        let names: Vec<&str> = found
            .iter()
            .filter(|f| f.dir == dir)
            .map(|f| f.name.as_str())
            .collect();
        let Some(defining) = MANIFESTS.iter().find(|m| names.contains(m)) else {
            continue;
        };
        let manifest = rel::join(dir, defining);
        let Some(tools) = tools(c, dir, &names) else {
            continue;
        };
        if tools.is_empty() && names == ["requirements.txt"] {
            continue;
        }
        c.add_root(dir, "python", &manifest);
        if tools.is_empty() {
            c.note(format!(
                "{manifest}: no pytest, mypy, pyright, ruff or flake8 configuration"
            ));
            continue;
        }
        let runner = runner(c, dir);
        for tool in tools {
            let id = format!("python:{}", tool.name());
            c.check(dir, &manifest, &id, tool.kind(), runner.command(tool));
        }
    }
}

/// The tools configured for `dir`; `None` when its pyproject.toml is not
/// valid TOML (every tool would fail to start).
fn tools(c: &mut Collector, dir: &str, names: &[&str]) -> Option<BTreeSet<Tool>> {
    let mut tools = BTreeSet::new();
    if names.contains(&"pyproject.toml") {
        let path = rel::join(dir, "pyproject.toml");
        let text = c.read(&path)?;
        let table = match text.parse::<toml::Table>() {
            Ok(table) => table,
            Err(error) => {
                c.note(format!("{path}: invalid TOML ({})", brief(&error)));
                return None;
            }
        };
        let tool = table.get("tool").and_then(toml::Value::as_table);
        for (key, found) in [
            ("pytest", Tool::Pytest),
            ("mypy", Tool::Mypy),
            ("pyright", Tool::Pyright),
            ("ruff", Tool::Ruff),
            ("flake8", Tool::Flake8),
        ] {
            if tool.is_some_and(|t| t.contains_key(key)) {
                tools.insert(found);
            }
        }
    }
    if names.contains(&"pytest.ini") {
        tools.insert(Tool::Pytest);
    }
    for (file, sections) in [
        (
            "tox.ini",
            &[("pytest", Tool::Pytest), ("flake8", Tool::Flake8)][..],
        ),
        (
            "setup.cfg",
            &[
                ("tool:pytest", Tool::Pytest),
                ("flake8", Tool::Flake8),
                ("mypy", Tool::Mypy),
            ][..],
        ),
    ] {
        if !names.contains(&file) {
            continue;
        }
        let Some(text) = c.read(&rel::join(dir, file)) else {
            continue;
        };
        let present = ini_sections(&text);
        for (section, tool) in sections {
            if present.contains(*section) {
                tools.insert(*tool);
            }
        }
    }
    for (marker, tool) in [
        ("conftest.py", Tool::Pytest),
        ("mypy.ini", Tool::Mypy),
        (".mypy.ini", Tool::Mypy),
        ("pyrightconfig.json", Tool::Pyright),
        ("ruff.toml", Tool::Ruff),
        (".ruff.toml", Tool::Ruff),
        (".flake8", Tool::Flake8),
    ] {
        if c.is_file(&rel::join(dir, marker)) {
            tools.insert(tool);
        }
    }
    if c.is_dir(&rel::join(dir, "tests")) {
        tools.insert(Tool::Pytest);
    }
    Some(tools)
}

fn ini_sections(text: &str) -> BTreeSet<&str> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix('['))
        .filter_map(|rest| rest.split_once(']'))
        .map(|(name, _)| name.trim())
        .collect()
}

fn runner(c: &Collector, dir: &str) -> Runner {
    for ancestor in rel::ancestors(dir) {
        if c.is_file(&rel::join(ancestor, "uv.lock")) {
            return Runner::Uv;
        }
        if c.is_file(&rel::join(ancestor, "poetry.lock")) {
            return Runner::Poetry;
        }
    }
    if c.is_file(&rel::join(dir, VENV_PYTHON)) {
        Runner::Python(VENV_PYTHON)
    } else {
        Runner::Python(SYSTEM_PYTHON)
    }
}
