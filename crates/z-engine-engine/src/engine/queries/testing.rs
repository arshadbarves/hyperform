//! An engine over temporary directories for the query tests.

use std::path::Path;
use std::sync::Arc;

use z_engine_config::{EnvOverrides, Paths};

use crate::engine::Engine;
use crate::options::EngineOptions;

pub(super) fn engine(dir: &Path) -> Engine {
    Engine::new(EngineOptions {
        paths: Paths::with_roots(dir.join("config"), dir.join("data")),
        event_sink: Arc::new(|_| {}),
        client_factory: None,
        env: EnvOverrides::default(),
    })
    .unwrap()
}

/// Runs `git` in `dir` with a fixed identity; panics on failure.
pub(super) fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(status.status.success(), "git {args:?}: {status:?}");
}
