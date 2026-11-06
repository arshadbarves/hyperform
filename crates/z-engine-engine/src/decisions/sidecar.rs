//! The engine-wide decision sidecar: `decisions.sidecar.command` started
//! through host background shells on a free loopback port, with a fresh
//! random `LAYA_API_KEY` per launch. One sidecar at a time: a launch with
//! other settings replaces it, and engine shutdown (or dropping the last
//! handle) kills it. The handle also carries the native runtime, the
//! in-process alternative to the sidecar.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use z_engine_host::{BackgroundShells, BackgroundSpec, EnvPolicy, ShellSpec, free_loopback_port};
use z_engine_protocol::{JobId, JobStatus};

use super::native::NativeRuntime;

/// After a launch the model is still loading; the connection test says so.
pub(crate) const WARM_UP: Duration = Duration::from_secs(60);
const LABEL: &str = "decision model sidecar";
const OWNER: &str = "decisions";

/// What a session asks to run; equal launches share one process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SidecarLaunch {
    pub command: String,
    pub checkpoint: String,
    pub max_len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SidecarEndpoint {
    pub endpoint: String,
    pub api_key: String,
    pub started: Instant,
}

impl SidecarEndpoint {
    pub(crate) fn warming_up(&self) -> bool {
        self.started.elapsed() < WARM_UP
    }
}

#[derive(Debug)]
struct Running {
    launch: SidecarLaunch,
    job: JobId,
    endpoint: SidecarEndpoint,
}

/// Cheap to clone; clones share the sidecar and the native runtime.
#[derive(Debug, Clone)]
pub(crate) struct Sidecars {
    shells: BackgroundShells,
    running: Arc<Mutex<Option<Running>>>,
    native: NativeRuntime,
}

impl Default for Sidecars {
    fn default() -> Self {
        Self {
            shells: BackgroundShells::new(None),
            running: Arc::default(),
            native: NativeRuntime::default(),
        }
    }
}

impl Sidecars {
    pub(crate) fn native(&self) -> &NativeRuntime {
        &self.native
    }

    /// The running sidecar for `launch`, starting (or replacing) it when
    /// needed. `cwd`, `shell` and `env` come from the asking session.
    pub(crate) async fn ensure(
        &self,
        launch: &SidecarLaunch,
        cwd: &Path,
        shell: &ShellSpec,
        env: &EnvPolicy,
    ) -> Result<SidecarEndpoint, String> {
        let mut running = self.running.lock().await;
        if let Some(current) = running.as_ref() {
            let alive = self
                .shells
                .snapshot(&current.job)
                .is_some_and(|job| job.status == JobStatus::Running);
            if alive && current.launch == *launch {
                return Ok(current.endpoint.clone());
            }
        }
        if let Some(old) = running.take() {
            self.kill(&old.job).await;
        }
        let port = free_loopback_port().map_err(|error| error.to_string())?;
        let api_key = random_key();
        let mut env = env.clone();
        let vars = [
            ("LAYA_API_KEY", api_key.clone()),
            ("LAYA_HOST", "127.0.0.1".to_string()),
            ("LAYA_PORT", port.to_string()),
            ("LAYA_CHECKPOINT", launch.checkpoint.clone()),
            ("LAYA_MAX_LEN", launch.max_len.to_string()),
        ];
        env.extra
            .extend(vars.map(|(name, value)| (name.to_string(), value)));
        let spec = BackgroundSpec {
            command: launch.command.clone(),
            cwd: cwd.to_path_buf(),
            shell: shell.clone(),
            env,
            label: LABEL.to_string(),
            owner: OWNER.to_string(),
        };
        let job = self
            .shells
            .spawn(spec)
            .await
            .map_err(|error| error.to_string())?;
        let endpoint = SidecarEndpoint {
            endpoint: format!("http://127.0.0.1:{port}"),
            api_key,
            started: Instant::now(),
        };
        *running = Some(Running {
            launch: launch.clone(),
            job,
            endpoint: endpoint.clone(),
        });
        Ok(endpoint)
    }

    pub(crate) async fn shutdown(&self) {
        self.native.cancel_download();
        if let Some(old) = self.running.lock().await.take() {
            self.kill(&old.job).await;
        }
    }

    async fn kill(&self, job: &JobId) {
        if let Err(error) = self.shells.kill(job).await {
            tracing::warn!(%error, "could not stop the decision sidecar");
        }
    }
}

/// 160 random bits from two ULIDs' random parts, as hex.
fn random_key() -> String {
    let random = |id: ulid::Ulid| id.random() & ((1u128 << 80) - 1);
    format!(
        "{:020x}{:020x}",
        random(ulid::Ulid::new()),
        random(ulid::Ulid::new())
    )
}

#[cfg(test)]
mod tests {
    use z_engine_host::resolve_shell;

    use super::*;

    fn launch(command: &str) -> SidecarLaunch {
        SidecarLaunch {
            command: command.to_string(),
            checkpoint: "multilingual".into(),
            max_len: 1_024,
        }
    }

    #[test]
    fn keys_are_long_and_fresh() {
        let (a, b) = (random_key(), random_key());
        assert_eq!(a.len(), 40);
        assert_ne!(a, b);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn one_sidecar_is_shared_replaced_and_stopped() {
        let tmp = tempfile::tempdir().unwrap();
        let sidecars = Sidecars::default();
        let (shell, env) = (resolve_shell(None), EnvPolicy::default());
        let sleep = launch("sleep 30");
        let first = sidecars
            .ensure(&sleep, tmp.path(), &shell, &env)
            .await
            .unwrap();
        assert!(first.endpoint.starts_with("http://127.0.0.1:"));
        assert!(first.warming_up());
        let again = sidecars
            .ensure(&sleep, tmp.path(), &shell, &env)
            .await
            .unwrap();
        assert_eq!(first, again, "equal launches share the process");
        let other = sidecars
            .ensure(&launch("sleep 31"), tmp.path(), &shell, &env)
            .await
            .unwrap();
        assert_ne!(other.api_key, first.api_key);
        let jobs = sidecars.shells.list();
        assert_eq!(
            jobs.iter()
                .filter(|job| job.status == JobStatus::Running)
                .count(),
            1
        );
        sidecars.shutdown().await;
        assert!(
            sidecars
                .shells
                .list()
                .iter()
                .all(|job| job.status != JobStatus::Running)
        );
    }
}
