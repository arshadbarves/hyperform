//! Which server instance serves a file: the first spec claiming the file's
//! extension whose binary is on PATH and whose root markers (if any) exist
//! in an ancestor directory inside the project. The workspace root is the
//! outermost such directory, so one server covers a whole Cargo or Go
//! workspace; specs without markers use the project root.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::spec::LspServerSpec;
use crate::error::IntegrationError;
use crate::process::find_program;
use crate::sync::lock;

#[derive(Debug, Clone)]
pub(crate) struct Route {
    pub spec: LspServerSpec,
    pub program: PathBuf,
    pub root: PathBuf,
}

#[derive(Debug)]
pub(crate) struct Router {
    project_root: PathBuf,
    specs: Vec<LspServerSpec>,
    /// PATH lookups per spec name, including misses.
    programs: Mutex<HashMap<String, Option<PathBuf>>>,
}

impl Router {
    pub(crate) fn new(project_root: PathBuf, specs: Vec<LspServerSpec>) -> Self {
        Self {
            project_root,
            specs,
            programs: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn route(&self, file: &Path) -> Result<Route, IntegrationError> {
        let extension = file
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| {
                IntegrationError::Unsupported(format!(
                    "{} has no extension, so no language server applies",
                    file.display()
                ))
            })?;
        let candidates: Vec<&LspServerSpec> = self
            .specs
            .iter()
            .filter(|spec| spec.handles(&extension))
            .collect();
        if candidates.is_empty() {
            return Err(IntegrationError::Unsupported(format!(
                "no language server is configured for `.{extension}` files"
            )));
        }
        let mut reasons = Vec::new();
        for spec in candidates {
            let Some(program) = self.program(spec) else {
                reasons.push(format!("`{}` is not installed (not on PATH)", spec.command));
                continue;
            };
            match self.workspace_root(spec, file).await {
                Some(root) => {
                    return Ok(Route {
                        spec: spec.clone(),
                        program,
                        root,
                    });
                }
                None => reasons.push(format!(
                    "{} needs {} in a directory above the file",
                    spec.name,
                    spec.root_markers.join(" or ")
                )),
            }
        }
        Err(IntegrationError::Unsupported(format!(
            "no language server available for `.{extension}` files: {}",
            reasons.join("; ")
        )))
    }

    /// Servers whose root markers sit in the project root, for
    /// workspace-wide queries before any file started a server. One per
    /// extension; specs without markers are skipped (no evidence the
    /// project uses their language).
    pub(crate) async fn project_routes(&self) -> Vec<Route> {
        let mut routes: Vec<Route> = Vec::new();
        for spec in &self.specs {
            let claimed = routes
                .iter()
                .any(|route| spec.extensions.iter().any(|ext| route.spec.handles(ext)));
            if spec.root_markers.is_empty()
                || claimed
                || !self.marked(spec, &self.project_root).await
            {
                continue;
            }
            if let Some(program) = self.program(spec) {
                routes.push(Route {
                    spec: spec.clone(),
                    program,
                    root: self.project_root.clone(),
                });
            }
        }
        routes
    }

    fn program(&self, spec: &LspServerSpec) -> Option<PathBuf> {
        lock(&self.programs)
            .entry(spec.name.clone())
            .or_insert_with(|| find_program(&spec.command, Some(&self.project_root)))
            .clone()
    }

    async fn workspace_root(&self, spec: &LspServerSpec, file: &Path) -> Option<PathBuf> {
        if spec.root_markers.is_empty() {
            return Some(self.project_root.clone());
        }
        let mut outermost = None;
        for dir in file.ancestors().skip(1) {
            if !dir.starts_with(&self.project_root) {
                break;
            }
            if self.marked(spec, dir).await {
                outermost = Some(dir.to_path_buf());
            }
        }
        outermost
    }

    async fn marked(&self, spec: &LspServerSpec, dir: &Path) -> bool {
        for marker in &spec.root_markers {
            if tokio::fs::try_exists(dir.join(marker))
                .await
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn fake(name: &str, command: &str, markers: &[&str]) -> LspServerSpec {
        LspServerSpec::new(name, command, &[], &["fk"], markers)
    }

    #[tokio::test]
    async fn picks_the_first_installed_spec_and_the_outermost_marker() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("ws/member/src")).unwrap();
        std::fs::write(root.join("ws/Mark"), "").unwrap();
        std::fs::write(root.join("ws/member/Mark"), "").unwrap();
        let file = root.join("ws/member/src/a.fk");
        let sh = find_program("sh", None).unwrap();
        let router = Router::new(
            root.clone(),
            vec![
                fake("missing", "zengine-no-such-server", &[]),
                fake("found", sh.to_str().unwrap(), &["Mark"]),
            ],
        );
        let route = router.route(&file).await.unwrap();
        assert_eq!(route.spec.name, "found");
        assert_eq!(route.root, root.join("ws"));
    }

    #[tokio::test]
    async fn explains_why_nothing_applies() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let sh = find_program("sh", None).unwrap();
        let router = Router::new(
            root.clone(),
            vec![
                fake("missing", "zengine-no-such-server", &[]),
                fake("marked", sh.to_str().unwrap(), &["Mark"]),
            ],
        );
        let error = router
            .route(&root.join("a.fk"))
            .await
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("not on PATH") && error.contains("needs Mark"),
            "{error}"
        );
        let other = router
            .route(&root.join("a.zz"))
            .await
            .unwrap_err()
            .to_string();
        assert!(
            other.contains("no language server is configured for `.zz`"),
            "{other}"
        );
        assert!(router.route(&root.join("Makefile")).await.is_err());
        assert!(router.project_routes().await.is_empty());
        std::fs::write(root.join("Mark"), "").unwrap();
        assert_eq!(router.project_routes().await.len(), 1);
    }
}
