use serde_json::json;

use super::*;
use crate::ids::{AgentId, SessionId, TaskId};

#[test]
fn pause_hold_and_release_edits_use_camel_case() {
    let edits = [
        (json!({"type": "pause"}), WorkEdit::Pause),
        (json!({"type": "resume"}), WorkEdit::Resume),
        (json!({"type": "stop"}), WorkEdit::Stop),
        (json!({"type": "reopen"}), WorkEdit::Reopen),
        (json!({"type": "abandon"}), WorkEdit::Abandon),
        (
            json!({"type": "holdTask", "taskId": "tsk_a", "held": true}),
            WorkEdit::HoldTask {
                task_id: TaskId::from("tsk_a"),
                held: true,
            },
        ),
        (
            json!({"type": "releaseTask", "taskId": "tsk_a"}),
            WorkEdit::ReleaseTask {
                task_id: TaskId::from("tsk_a"),
            },
        ),
    ];
    for (json, edit) in edits {
        assert_eq!(serde_json::from_value::<WorkEdit>(json).unwrap(), edit);
    }
}

#[test]
fn an_interruption_carries_its_resume_pointer() {
    let event = WorkEvent::TaskInterrupted {
        task_id: TaskId::from("tsk_a"),
        resume: ResumePointer {
            session_id: SessionId::from("ses_1"),
            agent_id: AgentId::from("agt_1"),
            worktree: None,
            note: "the chat closed".into(),
        },
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "taskInterrupted");
    assert_eq!(json["resume"]["agentId"], "agt_1");
    assert_eq!(serde_json::from_value::<WorkEvent>(json).unwrap(), event);
    assert_eq!(
        serde_json::to_value(ParallelBlock::Replanning).unwrap(),
        json!({"type": "replanning"})
    );
    assert_eq!(
        serde_json::to_value(ChangeCause::Resume).unwrap(),
        json!("resume")
    );
}

#[test]
fn an_older_plan_summary_reads_with_empty_counts() {
    let older = json!({
        "planId": "pln_1", "projectRoot": "/p", "goal": "Ship", "status": "active",
        "depth": "light", "revision": 3, "tasksTotal": 2, "tasksDone": 1,
        "createdAt": 1, "updatedAt": 2
    });
    let summary: PlanSummary = serde_json::from_value(older).unwrap();
    assert_eq!(
        (summary.blocked, summary.needs_you, summary.running),
        (0, 0, 0)
    );
    assert_eq!(
        (&summary.last_session_id, &summary.attached_to),
        (&None, &None)
    );
    let json = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["needsYou"], 0);
    assert_eq!(json["lastSessionId"], json!(null));
    assert_eq!(json["attachedTo"], json!(null));
}

#[test]
fn a_discarded_resume_worktree_names_its_task() {
    let event = WorkEvent::ResumeWorktreeDiscarded {
        task_id: TaskId::from("tsk_a"),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(
        json,
        json!({"type": "resumeWorktreeDiscarded", "taskId": "tsk_a"})
    );
    assert_eq!(serde_json::from_value::<WorkEvent>(json).unwrap(), event);
}
