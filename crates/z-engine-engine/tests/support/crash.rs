//! Simulated app crashes: the runtime running a live engine is torn down
//! (no close, no shutdown: every task is dropped where it stands), and a
//! fresh engine later reopens the session from what reached the disk.

use z_engine_config::Paths;
use z_engine_protocol::{Event, SessionId};
use z_engine_testkit::{FixtureRepo, ScriptedModel};

use super::{Harness, boot};

/// What survives a crash: the data directories and the project.
pub struct Remains {
    pub paths: Paths,
    pub repo: FixtureRepo,
    pub session: SessionId,
    dirs: tempfile::TempDir,
}

impl Harness {
    /// Drops `runtime` under the live engine, as a crash would.
    pub fn crash(self, runtime: tokio::runtime::Runtime) -> Remains {
        let Harness {
            engine,
            repo,
            session,
            paths,
            _dirs,
            ..
        } = self;
        drop(runtime);
        drop(engine);
        Remains {
            paths,
            repo,
            session,
            dirs: _dirs,
        }
    }

    /// Shuts the engine down cleanly, as quitting the app would.
    pub async fn shut_down(self) -> Remains {
        let Harness {
            engine,
            repo,
            session,
            paths,
            _dirs,
            ..
        } = self;
        engine.shutdown().await;
        drop(engine);
        Remains {
            paths,
            repo,
            session,
            dirs: _dirs,
        }
    }
}

impl Remains {
    /// A new engine (a restarted app) reopening the crashed session; the
    /// harness has seen its `Snapshot`.
    pub async fn restart(self) -> Harness {
        let model = ScriptedModel::new();
        let (engine, mut events) = boot(&self.paths, &model);
        let session = engine
            .open_session(self.repo.path(), Some(self.session.clone()))
            .await
            .unwrap();
        assert_eq!(session, self.session);
        events
            .wait_for(|event| matches!(event, Event::Snapshot { .. }))
            .await;
        Harness {
            engine,
            model,
            events,
            repo: self.repo,
            session,
            paths: self.paths,
            _dirs: self.dirs,
        }
    }
}
