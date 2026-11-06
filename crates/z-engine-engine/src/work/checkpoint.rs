//! The plan's own record of the workspace: after each main-agent turn and
//! each helper change that reached the project tree, the kernel links the
//! tree's fingerprint and a shadow checkpoint of it (`CheckpointLinked`),
//! so resuming the plan later tells files changed outside the plan from
//! its own work.

use z_engine_host::workspace_fingerprint;
use z_engine_protocol::work::{Actor, EvidenceRef, PlanStatus, WorkCause, WorkEvent, WorkRecord};
use z_engine_work::{PlanState, Proposal, Submission};

use super::facts::instant;
use super::lifecycle::attached;
use super::plan::PlanHandle;
use crate::session::SessionCore;

/// Links the project tree as it is now to the attached plan, when it
/// changed since the plan last saw it.
pub(crate) async fn link(core: &SessionCore) {
    let Some(plan) = attached(core) else {
        return;
    };
    let closed = plan.read(|state| {
        matches!(
            state.header.status,
            PlanStatus::Done | PlanStatus::Abandoned
        )
    });
    if closed {
        return;
    }
    let Ok(fingerprint) = workspace_fingerprint(&core.root).await else {
        return;
    };
    if plan.read(|state| state.fingerprint.as_deref() == Some(fingerprint.as_str())) {
        return;
    }
    let checkpoint = snapshot(core).await;
    let proposals = vec![Proposal::LinkCheckpoint {
        fingerprint,
        checkpoint,
    }];
    let result = plan.submit(&instant(core), |_| {
        Ok(Submission::new(
            Actor::Kernel,
            WorkCause::Reconciliation,
            proposals,
        ))
    });
    if let Err(error) = result {
        tracing::debug!(%error, "the plan's checkpoint was not linked");
    }
}

/// A shadow checkpoint of the project tree, when checkpoints work.
pub(crate) async fn snapshot(core: &SessionCore) -> Option<String> {
    if !core.checkpoints.available() {
        return None;
    }
    let repo = core.checkpoints.repo(core).await.ok()?;
    match repo.snapshot("work plan checkpoint").await {
        Ok(commit) => Some(commit),
        Err(error) => {
            tracing::debug!(%error, "no checkpoint for the plan");
            None
        }
    }
}

/// The fingerprint the plan saw last: its latest linked one, else the
/// tree of its latest check evidence.
pub(crate) fn recorded_fingerprint(state: &PlanState) -> Option<String> {
    state.fingerprint.clone().or_else(|| {
        state
            .tasks
            .values()
            .flat_map(|task| &task.acceptance)
            .flat_map(|criterion| &criterion.evidence)
            .filter_map(|evidence| match evidence {
                EvidenceRef::Check {
                    fingerprint, at, ..
                } => Some((*at, fingerprint.clone())),
                _ => None,
            })
            .max()
            .map(|(_, fingerprint)| fingerprint)
    })
}

/// The latest linked checkpoint and when it was linked.
pub(crate) fn last_checkpoint(plan: &PlanHandle) -> Option<(String, u64)> {
    plan.read_log(|records, _| latest_link(records))
}

fn latest_link(records: &[WorkRecord]) -> Option<(String, u64)> {
    records.iter().rev().find_map(|record| match &record.event {
        WorkEvent::CheckpointLinked {
            checkpoint: Some(commit),
            ..
        } => Some((commit.clone(), record.at)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use z_engine_protocol::work::{Actor, WorkCause};

    use super::*;

    fn linked(seq: u64, at: u64, checkpoint: Option<&str>) -> WorkRecord {
        WorkRecord {
            seq,
            revision: seq,
            at,
            actor: Actor::Kernel,
            cause: WorkCause::Reconciliation,
            event: WorkEvent::CheckpointLinked {
                fingerprint: format!("fp{seq}"),
                checkpoint: checkpoint.map(String::from),
            },
        }
    }

    #[test]
    fn the_latest_link_with_a_checkpoint_is_the_diff_base() {
        let records = [
            linked(1, 10, Some("c1")),
            linked(2, 20, Some("c2")),
            linked(3, 30, None),
        ];
        assert_eq!(latest_link(&records), Some(("c2".into(), 20)));
        assert_eq!(latest_link(&records[2..]), None);
    }
}
