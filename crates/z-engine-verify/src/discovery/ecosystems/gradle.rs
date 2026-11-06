//! Gradle: one root per build — a directory with a settings script, or a
//! build script outside any multi-project build (whose subprojects are
//! covered by the build's own `test` and `build` tasks). The Gradle build
//! scripts are executable configuration and are not read.

use z_engine_protocol::CheckKind;

use crate::discovery::collector::Collector;
use crate::discovery::rel;
use crate::discovery::walk::Found;

/// In order of preference as the root's defining manifest.
const MANIFESTS: &[&str] = &[
    "settings.gradle.kts",
    "settings.gradle",
    "build.gradle.kts",
    "build.gradle",
];

#[cfg(windows)]
const WRAPPER: (&str, &str) = ("gradlew.bat", r".\gradlew.bat");
#[cfg(not(windows))]
const WRAPPER: (&str, &str) = ("gradlew", "./gradlew");

pub(crate) fn is_manifest(name: &str) -> bool {
    MANIFESTS.contains(&name)
}

pub(crate) fn discover(c: &mut Collector, found: &[Found]) {
    let settings: Vec<&str> = found
        .iter()
        .filter(|f| f.name.starts_with("settings."))
        .map(|f| f.dir.as_str())
        .collect();
    let mut dirs: Vec<&str> = found.iter().map(|f| f.dir.as_str()).collect();
    dirs.dedup();
    for dir in dirs {
        if rel::nested_in(dir, settings.iter().copied()) {
            continue;
        }
        let Some(defining) = MANIFESTS
            .iter()
            .find(|m| found.iter().any(|f| f.dir == dir && f.name == **m))
        else {
            continue;
        };
        let manifest = rel::join(dir, defining);
        let (wrapper, invoke) = WRAPPER;
        let gradle = if c.is_file(&rel::join(dir, wrapper)) {
            invoke
        } else {
            "gradle"
        };
        c.add_root(dir, "gradle", &manifest);
        c.check(
            dir,
            &manifest,
            "gradle:test",
            CheckKind::Test,
            format!("{gradle} test"),
        );
        c.check(
            dir,
            &manifest,
            "gradle:build",
            CheckKind::Build,
            format!("{gradle} build"),
        );
    }
}
