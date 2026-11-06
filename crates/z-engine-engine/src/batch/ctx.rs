//! The per-call capability bundle handed to tools: identity, directories,
//! mode, the agent's tracker and shell cwd, session locks, shell and web
//! settings, progress and spill hooks, and the engine's ports.

use std::sync::Arc;

use z_engine_host::OutputSink;
use z_engine_protocol::CallId;
use z_engine_tools::{ShellConfig, SpillFn, ToolCtx, ToolLimits, WebOptions};

use crate::ports::run_ports;
use crate::run::RunContext;
use crate::session::SessionCore;
use crate::settings::{models, sandbox};

pub(crate) fn tool_ctx(
    ctx: &RunContext,
    call_id: &CallId,
    progress: Option<OutputSink>,
) -> ToolCtx {
    let core = &ctx.core;
    let settings = core.settings();
    let catalog = core.catalog();
    let web = &settings.settings.web;
    let mut additional_dirs = core.additional_dirs();
    let sandbox = sandbox::profile_for(&settings, &ctx.spec.root, &additional_dirs);
    additional_dirs.extend(
        ctx.spec
            .worktree
            .as_ref()
            .map(|scope| scope.project.clone()),
    );
    ToolCtx {
        session_id: core.id.clone(),
        agent_id: ctx.spec.agent_id.clone(),
        call_id: call_id.clone(),
        root: ctx.spec.root.clone(),
        additional_dirs,
        mode: ctx.mode(),
        planning: ctx.planning(),
        cwd: Arc::clone(&ctx.resources.cwd),
        cancel: ctx.cancel.clone(),
        files: Arc::new(ctx.resources.files.clone()),
        locks: Arc::new(core.locks.clone()),
        shell: Arc::new(ShellConfig {
            spec: settings.shell.clone(),
            env: settings.env.clone(),
            sandbox,
        }),
        web: core.shared.web.clone(),
        web_options: WebOptions {
            allow_private_network: web.allow_private_network,
            fetch_extract: web.fetch_extract,
            search: settings.web_search.clone(),
        },
        progress,
        spill: Some(spill(core)),
        limits: ToolLimits::default(),
        vision: models::supports_vision(catalog.as_deref(), &ctx.model()),
        ports: Arc::new(run_ports(ctx)),
    }
}

/// Full outputs go to the session's artifacts; a failed write is logged
/// and reported to the tool as "not stored".
fn spill(core: &SessionCore) -> SpillFn {
    let artifacts = core.shared.store.artifacts(&core.id);
    Arc::new(
        move |hint: &str, content: &str| match artifacts.write_text(hint, "txt", content) {
            Ok(path) => Some(path),
            Err(error) => {
                tracing::warn!(%error, hint, "could not store a full tool output");
                None
            }
        },
    )
}
