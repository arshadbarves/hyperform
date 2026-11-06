//! Engine services behind the tools crate's port traits.

mod agents;
mod checks;
mod interaction;
mod jobs;
mod lsp;
mod mcp;
mod relevance;
mod side_model;
mod skills;
mod work;

use std::sync::Arc;

use z_engine_tools::{LspPort, Ports, RelevancePort, WorkPort};

use crate::run::RunContext;

/// The ports of one run's tool calls; subagents spawn as its children.
pub(crate) fn run_ports(ctx: &RunContext) -> Ports {
    let core = &ctx.core;
    let lsp = core
        .lsp
        .worker()
        .map(|worker| Arc::new(lsp::Lsp::new(worker)) as Arc<dyn LspPort>);
    Ports {
        agents: Some(Arc::new(agents::Agents::new(ctx.clone()))),
        interaction: Some(Arc::new(interaction::Interaction::new(
            Arc::clone(core),
            Arc::clone(&ctx.web),
        ))),
        jobs: Some(Arc::new(jobs::Jobs::new(
            Arc::clone(core),
            Arc::clone(&ctx.web),
        ))),
        skills: Some(Arc::new(skills::Skills::new(Arc::clone(core)))),
        checks: Some(Arc::new(checks::Checks::new(Arc::clone(core)))),
        lsp,
        mcp: Some(Arc::new(mcp::Mcp::new(
            Arc::clone(core),
            Arc::clone(&ctx.mcp),
        ))),
        side_model: Some(Arc::new(side_model::SideModel::new(Arc::clone(core)))),
        relevance: relevance::Relevance::for_run(ctx)
            .map(|port| Arc::new(port) as Arc<dyn RelevancePort>),
        work: crate::work::enabled(core).then(|| {
            Arc::new(work::Work::new(Arc::clone(core), Arc::clone(&ctx.web))) as Arc<dyn WorkPort>
        }),
    }
}
