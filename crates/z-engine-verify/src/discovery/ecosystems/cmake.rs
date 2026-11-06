//! CMake: one root per top-level `CMakeLists.txt` (nested lists are its
//! subdirectories). Checks need a configured `build/` directory: a build,
//! plus CTest when the project registers tests. Discovery never configures.

use z_engine_protocol::CheckKind;

use crate::discovery::collector::Collector;
use crate::discovery::rel;
use crate::discovery::walk::Found;

pub(crate) fn is_manifest(name: &str) -> bool {
    name == "CMakeLists.txt"
}

pub(crate) fn discover(c: &mut Collector, found: &[Found]) {
    let dirs: Vec<&str> = found.iter().map(|f| f.dir.as_str()).collect();
    for dir in &dirs {
        if rel::nested_in(dir, dirs.iter().copied()) {
            continue;
        }
        let manifest = rel::join(dir, "CMakeLists.txt");
        let Some(text) = c.read(&manifest) else {
            continue;
        };
        c.add_root(dir, "cmake", &manifest);
        let build = rel::join(dir, "build");
        if !c.is_dir(&build) {
            c.note(format!(
                "{manifest}: no build directory; configure one with `cmake -S . -B build` to enable build and test checks"
            ));
            continue;
        }
        c.check(
            dir,
            &manifest,
            "cmake:build",
            CheckKind::Build,
            "cmake --build build",
        );
        let lower = text.to_ascii_lowercase();
        let registers_tests = lower.contains("enable_testing")
            || lower.contains("add_test")
            || c.is_file(&rel::join(&build, "CTestTestfile.cmake"));
        if registers_tests {
            c.check(
                dir,
                &manifest,
                "cmake:test",
                CheckKind::Test,
                "ctest --test-dir build",
            );
        }
    }
}
