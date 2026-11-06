//! .NET: a directory with a solution is one root covering the projects
//! below it; other project directories are roots of their own. Projects
//! get `dotnet test` only when they reference a test SDK or framework.

use z_engine_protocol::CheckKind;

use crate::discovery::collector::Collector;
use crate::discovery::rel;
use crate::discovery::walk::Found;

const SOLUTIONS: &[&str] = &["sln", "slnx"];
const PROJECTS: &[&str] = &["csproj", "fsproj", "vbproj"];
/// Lowercased markers of a test project.
const TEST_MARKERS: &[&str] = &[
    "microsoft.net.test.sdk",
    "<istestproject>true",
    "mstest.sdk",
    "xunit",
    "nunit",
];

pub(crate) fn is_manifest(name: &str) -> bool {
    extension(name).is_some_and(|ext| SOLUTIONS.contains(&ext) || PROJECTS.contains(&ext))
}

fn extension(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_, ext)| ext)
}

fn is_solution(name: &str) -> bool {
    extension(name).is_some_and(|ext| SOLUTIONS.contains(&ext))
}

pub(crate) fn discover(c: &mut Collector, found: &[Found]) {
    let solution_dirs: Vec<&str> = found
        .iter()
        .filter(|f| is_solution(&f.name))
        .map(|f| f.dir.as_str())
        .collect();
    let mut dirs: Vec<&str> = found.iter().map(|f| f.dir.as_str()).collect();
    dirs.dedup();
    for dir in dirs {
        if rel::nested_in(dir, solution_dirs.iter().copied()) {
            continue;
        }
        let mut files: Vec<&str> = found
            .iter()
            .filter(|f| f.dir == dir)
            .map(|f| f.name.as_str())
            .collect();
        files.sort_unstable();
        let target = files
            .iter()
            .find(|name| is_solution(name))
            .or_else(|| files.first())
            .copied();
        let Some(target) = target else {
            continue;
        };
        let manifest = rel::join(dir, target);
        let Some(text) = c.read(&manifest) else {
            continue;
        };
        let (valid, tests) = if target.ends_with(".sln") {
            (text.contains("Microsoft Visual Studio Solution File"), true)
        } else if target.ends_with(".slnx") {
            (text.contains("<Solution"), true)
        } else {
            let lower = text.to_ascii_lowercase();
            (
                text.contains("<Project"),
                TEST_MARKERS.iter().any(|m| lower.contains(m)),
            )
        };
        if !valid {
            c.note(format!(
                "{manifest}: not a solution or MSBuild project file"
            ));
            continue;
        }
        // `dotnet` refuses to guess when a directory has several candidates.
        let arg = if files.len() == 1 {
            String::new()
        } else {
            match quoted(target) {
                Some(arg) => format!(" {arg}"),
                None => {
                    c.note(format!(
                        "{manifest}: file name is not safe to pass to dotnet"
                    ));
                    continue;
                }
            }
        };
        c.add_root(dir, "dotnet", &manifest);
        if tests {
            c.check(
                dir,
                &manifest,
                "dotnet:test",
                CheckKind::Test,
                format!("dotnet test{arg}"),
            );
        }
        c.check(
            dir,
            &manifest,
            "dotnet:build",
            CheckKind::Build,
            format!("dotnet build{arg}"),
        );
    }
}

/// A file name as one shell word: plain names as they are, names with
/// spaces double-quoted, anything else refused.
fn quoted(name: &str) -> Option<String> {
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+');
    if name.chars().all(plain) {
        Some(name.to_string())
    } else if name.chars().all(|c| plain(c) || c == ' ') {
        Some(format!("\"{name}\""))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_quoted_or_refused() {
        assert_eq!(quoted("App.sln").as_deref(), Some("App.sln"));
        assert_eq!(quoted("My App.sln").as_deref(), Some("\"My App.sln\""));
        assert_eq!(quoted("x;rm -rf ~.sln"), None);
        assert_eq!(quoted("$(evil).csproj"), None);
    }
}
