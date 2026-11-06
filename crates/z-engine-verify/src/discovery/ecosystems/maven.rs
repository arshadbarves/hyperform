//! Maven: one root per top-level `pom.xml`; a `pom.xml` below another is a
//! module of that build and is covered by its checks. The Maven wrapper is
//! used when present.

use z_engine_protocol::CheckKind;

use crate::discovery::collector::Collector;
use crate::discovery::rel;
use crate::discovery::walk::Found;

#[cfg(windows)]
const WRAPPER: (&str, &str) = ("mvnw.cmd", r".\mvnw.cmd");
#[cfg(not(windows))]
const WRAPPER: (&str, &str) = ("mvnw", "./mvnw");

pub(crate) fn is_manifest(name: &str) -> bool {
    name == "pom.xml"
}

pub(crate) fn discover(c: &mut Collector, found: &[Found]) {
    let mut builds: Vec<(&str, String)> = Vec::new();
    for f in found {
        let path = rel::join(&f.dir, &f.name);
        let Some(text) = c.read(&path) else {
            continue;
        };
        if !text.contains("<project") {
            c.note(format!("{path}: no <project> element"));
            continue;
        }
        builds.push((f.dir.as_str(), path));
    }
    let dirs: Vec<&str> = builds.iter().map(|(dir, _)| *dir).collect();
    for (dir, manifest) in &builds {
        if rel::nested_in(dir, dirs.iter().copied()) {
            continue;
        }
        let (wrapper, invoke) = WRAPPER;
        let mvn = if c.is_file(&rel::join(dir, wrapper)) {
            invoke
        } else {
            "mvn"
        };
        c.add_root(dir, "maven", manifest);
        c.check(
            dir,
            manifest,
            "maven:test",
            CheckKind::Test,
            format!("{mvn} -q test"),
        );
        c.check(
            dir,
            manifest,
            "maven:package",
            CheckKind::Build,
            format!("{mvn} -q -DskipTests package"),
        );
    }
}
