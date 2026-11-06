mod support;

use support::{Fixture, check, command, has_note, ids, roots};
use z_engine_protocol::CheckKind;

const PYTHON: &str = if cfg!(windows) { "python" } else { "python3" };

#[test]
fn pyproject_tools_become_checks() {
    let fixture = Fixture::new(&[(
        "pyproject.toml",
        "[project]\nname = \"api\"\n\n[tool.pytest.ini_options]\ntestpaths = [\"tests\"]\n\n\
         [tool.ruff]\nline-length = 100\n\n[tool.mypy]\nstrict = true\n\n[tool.pyright]\n",
    )]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [(".", "python")]);
    assert_eq!(
        ids(&profile),
        [
            "python:pytest",
            "python:mypy",
            "python:pyright",
            "python:ruff"
        ]
    );
    assert_eq!(
        command(&profile, "python:pytest"),
        format!("{PYTHON} -m pytest")
    );
    assert_eq!(
        command(&profile, "python:mypy"),
        format!("{PYTHON} -m mypy .")
    );
    assert_eq!(
        command(&profile, "python:ruff"),
        format!("{PYTHON} -m ruff check .")
    );
    assert_eq!(command(&profile, "python:pyright"), "pyright");
    assert_eq!(check(&profile, "python:pytest").kind, CheckKind::Test);
    assert_eq!(check(&profile, "python:mypy").kind, CheckKind::Typecheck);
    assert_eq!(check(&profile, "python:ruff").kind, CheckKind::Lint);
}

#[test]
fn pytest_is_found_through_every_kind_of_evidence() {
    let cases: [&[(&str, &str)]; 6] = [
        &[("pytest.ini", "[pytest]\naddopts = -q\n")],
        &[(
            "tox.ini",
            "[tox]\nenvlist = py312\n\n[pytest]\ntestpaths = tests\n",
        )],
        &[(
            "setup.cfg",
            "[metadata]\nname = lib\n\n[tool:pytest]\naddopts = -q\n",
        )],
        &[
            ("setup.py", "from setuptools import setup\nsetup()\n"),
            ("conftest.py", ""),
        ],
        &[
            ("requirements.txt", "requests\n"),
            ("tests/test_api.py", ""),
        ],
        &[("pyproject.toml", "[tool.pytest]\nminversion = \"8.0\"\n")],
    ];
    for files in cases {
        let profile = Fixture::new(files).discover();
        assert_eq!(ids(&profile), ["python:pytest"], "{files:?}");
    }
}

#[test]
fn flake8_and_mypy_from_ini_files_and_markers() {
    let fixture = Fixture::new(&[
        (
            "setup.cfg",
            "[flake8]\nmax-line-length = 100\n\n[mypy]\nstrict = True\n",
        ),
        ("svc/pyproject.toml", "[project]\nname = \"svc\"\n"),
        ("svc/.flake8", "[flake8]\n"),
        ("svc/pyrightconfig.json", "{}"),
    ]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [(".", "python"), ("svc", "python")]);
    assert_eq!(
        ids(&profile),
        [
            "python:mypy",
            "python:flake8",
            "svc/python:pyright",
            "svc/python:flake8"
        ]
    );
    assert_eq!(
        command(&profile, "python:flake8"),
        format!("{PYTHON} -m flake8")
    );
    assert_eq!(
        check(&profile, "svc/python:flake8").label,
        format!("{PYTHON} -m flake8 (svc)")
    );
}

#[test]
fn uv_poetry_and_venv_runners() {
    let uv = Fixture::new(&[
        ("uv.lock", ""),
        (
            "packages/core/pyproject.toml",
            "[tool.pytest.ini_options]\n[tool.ruff]\n",
        ),
    ]);
    let profile = uv.discover();
    assert_eq!(
        command(&profile, "packages/core/python:pytest"),
        "uv run pytest"
    );
    assert_eq!(
        command(&profile, "packages/core/python:ruff"),
        "uv run ruff check ."
    );

    let poetry = Fixture::new(&[
        (
            "pyproject.toml",
            "[tool.poetry]\nname = \"app\"\n[tool.mypy]\n",
        ),
        ("poetry.lock", ""),
        ("tests/test_app.py", ""),
    ]);
    let profile = poetry.discover();
    assert_eq!(command(&profile, "python:pytest"), "poetry run pytest");
    assert_eq!(command(&profile, "python:mypy"), "poetry run mypy .");

    let venv_python = if cfg!(windows) {
        r".venv\Scripts\python.exe"
    } else {
        ".venv/bin/python"
    };
    let venv_file = venv_python.replace('\\', "/");
    let venv = Fixture::new(&[("pytest.ini", "[pytest]\n"), (venv_file.as_str(), "")]);
    assert_eq!(
        command(&venv.discover(), "python:pytest"),
        format!("{venv_python} -m pytest")
    );
}

#[test]
fn unconfigured_projects_are_noted_and_invalid_pyproject_is_skipped() {
    let bare = Fixture::new(&[("pyproject.toml", "[project]\nname = \"bare\"\n")]);
    let profile = bare.discover();
    assert_eq!(roots(&profile), [(".", "python")]);
    assert!(profile.checks.is_empty());
    assert!(has_note(
        &profile,
        "pyproject.toml: no pytest, mypy, pyright, ruff or flake8"
    ));

    let docs = Fixture::new(&[("docs/requirements.txt", "sphinx\n")]);
    let profile = docs.discover();
    assert!(profile.roots.is_empty() && profile.notes.is_empty());

    let broken = Fixture::new(&[
        ("pyproject.toml", "[tool.pytest.ini_options"),
        ("conftest.py", ""),
    ]);
    let profile = broken.discover();
    assert!(profile.roots.is_empty());
    assert!(has_note(&profile, "pyproject.toml: invalid TOML ("));
}
