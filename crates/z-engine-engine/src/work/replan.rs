//! `/replan`: reconcile now. With findings, or with the user's reasons
//! when there are none, the plan goes to Replanning and a turn revises it;
//! otherwise the command answers that there is nothing to replan.

use std::path::Path;

use z_engine_config::{CommandDef, ExtensionScope, ExtensionSource, parse_command};
use z_engine_prompts::commands::REPLAN;
use z_engine_protocol::work::{Actor, ChangeCause, PlanStatus, WorkCause};
use z_engine_work::Submission;

use super::facts::instant;
use super::ids::Ids;
use super::lifecycle::{attached, replan as replan_proposals};
use super::plan::Submitted;
use super::reconcile::{Trigger, reconcile};
use crate::session::SessionCore;

const NAME: &str = "replan";

pub(crate) enum Replan {
    /// Run this prompt as a turn.
    Turn(Box<CommandDef>),
    /// Nothing to replan; show this.
    Nothing(String),
}

pub(crate) async fn replan(core: &SessionCore, args: &str) -> Replan {
    let Some(plan) = attached(core) else {
        return Replan::Nothing("No work plan is attached to this chat.".into());
    };
    let found = reconcile(core, &plan, Trigger::Replan).await;
    if found.is_empty() {
        if args.is_empty() {
            return Replan::Nothing(
                "The plan matches the workspace, so there is nothing to replan. Add what to reconsider after `/replan` to replan anyway.".into(),
            );
        }
        let result = plan.submit(&instant(core), |state| {
            let proposals = if state.header.status == PlanStatus::Replanning {
                Vec::new()
            } else {
                replan_proposals(
                    &mut Ids::new(state),
                    ChangeCause::Replan,
                    args,
                    &[],
                    Vec::new(),
                    Vec::new(),
                )
            };
            Ok(Submission::new(Actor::User, WorkCause::Edit, proposals))
        });
        match result {
            Ok(Submitted::Rejected(rejection)) => {
                return Replan::Nothing(format!(
                    "The plan cannot be replanned now: {}",
                    rejection.messages().join("; ")
                ));
            }
            Err(error) => {
                return Replan::Nothing(format!("The plan cannot be replanned now: {error}"));
            }
            Ok(_) => {}
        }
    }
    let source = ExtensionSource::new(ExtensionScope::User, Path::new(NAME));
    match parse_command(NAME, REPLAN, source) {
        Ok(def) => Replan::Turn(Box::new(def)),
        Err(error) => Replan::Nothing(format!("/replan is unavailable: {error}")),
    }
}
