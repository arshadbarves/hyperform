//! The task and plan state machines: which status may follow which. Who
//! may make a move, and the gates some moves need, live in `authority` and
//! the kernel.

use hyperform_protocol::work::{PlanStatus, TaskStatus};

/// Same status, ignoring the details of `Blocked`, `Failed` and
/// `Superseded`.
pub fn same_kind(a: &TaskStatus, b: &TaskStatus) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

pub fn task_move_allowed(from: &TaskStatus, to: &TaskStatus) -> bool {
    use TaskStatus::*;
    matches!(
        (from, to),
        (Proposed, Pending | Cancelled | Superseded { .. })
            | (
                Pending,
                Active | Blocked { .. } | Done | Cancelled | Superseded { .. }
            )
            | (
                Active,
                Pending
                    | Blocked { .. }
                    | Verifying
                    | Done
                    | Failed { .. }
                    | Cancelled
                    | Superseded { .. }
            )
            | (
                Blocked { .. },
                Pending | Blocked { .. } | Failed { .. } | Cancelled | Superseded { .. }
            )
            | (
                Verifying,
                Pending | Active | Blocked { .. } | Done | Failed { .. } | Cancelled
            )
            | (Done, Pending | Superseded { .. })
            | (Failed { .. }, Pending | Cancelled | Superseded { .. })
            | (Cancelled, Pending)
    )
}

pub fn plan_move_allowed(from: PlanStatus, to: PlanStatus) -> bool {
    use PlanStatus::*;
    matches!(
        (from, to),
        (Draft, Active | Abandoned)
            | (Active, Paused | Replanning | Done | Abandoned)
            | (Paused, Active | Replanning | Abandoned)
            | (Replanning, Active | Paused | Abandoned)
            | (Done, Active | Replanning)
            | (Abandoned, Active | Draft)
    )
}

/// Plan moves only the user makes: approving, resuming or reopening
/// (anything to Active), pausing, abandoning, and back to Draft.
pub fn plan_move_needs_user(to: PlanStatus) -> bool {
    matches!(
        to,
        PlanStatus::Active | PlanStatus::Paused | PlanStatus::Abandoned | PlanStatus::Draft
    )
}

#[cfg(test)]
mod tests {
    use hyperform_protocol::TaskId;
    use hyperform_protocol::work::BlockReason;

    use super::*;

    fn blocked() -> TaskStatus {
        TaskStatus::Blocked {
            reason: BlockReason::Unreported,
        }
    }

    fn failed() -> TaskStatus {
        TaskStatus::Failed {
            reason: "tests".into(),
        }
    }

    #[test]
    fn task_moves_follow_the_lifecycle() {
        use TaskStatus::*;
        let allowed = [
            (Proposed, Pending),
            (Pending, Active),
            (Active, Verifying),
            (Verifying, Done),
            (Active, failed()),
            (failed(), Pending),
            (Pending, blocked()),
            (blocked(), Pending),
            (blocked(), blocked()),
            (Done, Pending),
            (Cancelled, Pending),
        ];
        for (from, to) in allowed {
            assert!(task_move_allowed(&from, &to), "{from:?} -> {to:?}");
        }
        let superseded = Superseded {
            by: TaskId::from("b"),
        };
        let refused = [
            (Proposed, Active),
            (Proposed, Done),
            (Pending, failed()),
            (Pending, Verifying),
            (blocked(), Active),
            (blocked(), Done),
            (Done, Active),
            (Done, failed()),
            (Cancelled, Done),
            (superseded.clone(), Pending),
            (failed(), Done),
        ];
        for (from, to) in refused {
            assert!(!task_move_allowed(&from, &to), "{from:?} -> {to:?}");
        }
        assert!(task_move_allowed(&Active, &superseded));
    }

    #[test]
    fn plan_moves_follow_the_lifecycle() {
        use PlanStatus::*;
        assert!(plan_move_allowed(Draft, Active));
        assert!(plan_move_allowed(Active, Replanning));
        assert!(plan_move_allowed(Replanning, Active));
        assert!(plan_move_allowed(Paused, Active));
        assert!(plan_move_allowed(Done, Active));
        assert!(!plan_move_allowed(Draft, Done));
        assert!(!plan_move_allowed(Draft, Replanning));
        assert!(!plan_move_allowed(Abandoned, Done));
        assert!(!plan_move_allowed(Paused, Done));
    }

    #[test]
    fn approval_pause_and_abandon_belong_to_the_user() {
        assert!(plan_move_needs_user(PlanStatus::Active));
        assert!(plan_move_needs_user(PlanStatus::Paused));
        assert!(plan_move_needs_user(PlanStatus::Abandoned));
        assert!(!plan_move_needs_user(PlanStatus::Replanning));
        assert!(!plan_move_needs_user(PlanStatus::Done));
    }

    #[test]
    fn same_kind_ignores_details() {
        assert!(same_kind(
            &blocked(),
            &TaskStatus::Blocked {
                reason: BlockReason::Other { text: "x".into() }
            }
        ));
        assert!(!same_kind(&TaskStatus::Pending, &TaskStatus::Active));
    }
}
