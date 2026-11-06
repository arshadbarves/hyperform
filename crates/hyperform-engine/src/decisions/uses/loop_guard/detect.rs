//! Findings over one agent's steps in this turn, newest last. A clear
//! finding (the same call and result again and again, the same failure
//! after an edit) reminds at once; a suspect one (the same failure without
//! edits, or mostly failures lately) only after the decision model says the
//! steps made no progress. Polled tools never count.

use super::steps::{FailureClass, Step};

/// The same call with the same result, since the last edit: remind at this
/// count and at each multiple of it.
pub(super) const REPEAT_LIMIT: usize = 3;
/// The same failure this often since the last edit is suspect.
const STUCK_FAILURES: usize = 3;
/// Failures among the last `RECENT` steps that are suspect.
const RECENT: usize = 6;
const RECENT_FAILURES: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Finding {
    Repeat {
        tool: String,
        count: usize,
        call: u64,
    },
    FailedAfterEdit {
        tool: String,
        count: usize,
        failure: u64,
        class: Option<FailureClass>,
    },
    Suspect {
        tool: String,
        count: usize,
        /// The newest failure repeats; otherwise failures merely pile up.
        same_failure: bool,
        class: Option<FailureClass>,
    },
}

impl Finding {
    pub(super) fn is_clear(&self) -> bool {
        !matches!(self, Self::Suspect { .. })
    }
}

pub(super) fn detect(steps: &[Step]) -> Option<Finding> {
    let last = steps.last()?;
    if last.polled {
        return None;
    }
    let tool = last.tool.clone();
    let since_edit = steps
        .iter()
        .rposition(|step| step.edit)
        .map_or(0, |index| index + 1);
    if let Some(failure) = &last.failure {
        let same = |step: &Step| {
            step.failure
                .as_ref()
                .is_some_and(|f| f.hash == failure.hash)
        };
        let before = &steps[..steps.len() - 1];
        if let Some(index) = before.iter().rposition(same)
            && steps[index + 1..].iter().any(|step| step.edit)
        {
            return Some(Finding::FailedAfterEdit {
                tool,
                count: steps.iter().filter(|step| same(step)).count(),
                failure: failure.hash,
                class: failure.class,
            });
        }
    }
    let repeats = steps[since_edit..]
        .iter()
        .filter(|step| step.call == last.call && step.result == last.result)
        .count();
    if repeats >= REPEAT_LIMIT && repeats % REPEAT_LIMIT == 0 {
        return Some(Finding::Repeat {
            tool,
            count: repeats,
            call: last.call,
        });
    }
    let failure = last.failure.as_ref()?;
    let stuck = steps[since_edit..]
        .iter()
        .filter(|step| {
            step.failure
                .as_ref()
                .is_some_and(|f| f.hash == failure.hash)
        })
        .count();
    let recent = steps
        .iter()
        .rev()
        .take(RECENT)
        .filter(|step| step.failure.is_some())
        .count();
    let (count, same_failure) = if stuck >= STUCK_FAILURES {
        (stuck, true)
    } else if recent >= RECENT_FAILURES {
        (recent, false)
    } else {
        return None;
    };
    Some(Finding::Suspect {
        tool,
        count,
        same_failure,
        class: failure.class,
    })
}
