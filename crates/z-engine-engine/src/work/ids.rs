//! Short ids the model can write back: `t3` for tasks, `e2` for ledger
//! entries, `r1` / `i1` for requirements and invariants. They are picked
//! under the plan lock, so they are unique within the plan.

use std::collections::BTreeSet;

use z_engine_protocol::work::PlanItemKind;
use z_engine_protocol::{EntryId, TaskId};
use z_engine_work::PlanState;

pub(crate) struct Ids<'a> {
    state: &'a PlanState,
    used: BTreeSet<String>,
}

impl<'a> Ids<'a> {
    pub(crate) fn new(state: &'a PlanState) -> Self {
        Self {
            state,
            used: BTreeSet::new(),
        }
    }

    pub(crate) fn state(&self) -> &'a PlanState {
        self.state
    }

    pub(crate) fn task(&mut self) -> TaskId {
        let start = self.state.tasks.len() + 1;
        TaskId(self.next("t", start, |state, id| {
            state.task(&TaskId(id.to_string())).is_some()
        }))
    }

    pub(crate) fn entry(&mut self) -> EntryId {
        self.ledger("e")
    }

    pub(crate) fn item(&mut self, kind: PlanItemKind) -> EntryId {
        self.ledger(match kind {
            PlanItemKind::Requirement => "r",
            PlanItemKind::Invariant => "i",
        })
    }

    fn ledger(&mut self, prefix: &str) -> EntryId {
        EntryId(self.next(prefix, 1, |state, id| {
            let id = EntryId(id.to_string());
            state.entry(&id).is_some() || state.item(&id).is_some()
        }))
    }

    fn next(
        &mut self,
        prefix: &str,
        start: usize,
        taken: impl Fn(&PlanState, &str) -> bool,
    ) -> String {
        let mut n = start;
        loop {
            let id = format!("{prefix}{n}");
            if !taken(self.state, &id) && self.used.insert(id.clone()) {
                return id;
            }
            n += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use z_engine_protocol::work::{Actor, PlanDepth, PlanStatus, WorkCause, WorkEvent, WorkRecord};

    use super::*;

    #[test]
    fn ids_are_short_and_unique_within_a_submission() {
        let record = WorkRecord {
            seq: 1,
            revision: 1,
            at: 1,
            actor: Actor::User,
            cause: WorkCause::Edit,
            event: WorkEvent::PlanCreated {
                plan_id: z_engine_protocol::PlanId::new(),
                project_root: "/p".into(),
                goal: "g".into(),
                document: String::new(),
                depth: PlanDepth::Light,
                status: PlanStatus::Draft,
            },
        };
        let state = PlanState::created(&record).unwrap();
        let mut ids = Ids::new(&state);
        assert_eq!(ids.task().0, "t1");
        assert_eq!(ids.task().0, "t2");
        assert_eq!(ids.entry().0, "e1");
        assert_eq!(ids.item(PlanItemKind::Requirement).0, "r1");
        assert_eq!(ids.item(PlanItemKind::Invariant).0, "i1");
        assert_eq!(ids.entry().0, "e2");
    }
}
