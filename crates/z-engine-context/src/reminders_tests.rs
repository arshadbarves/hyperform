use z_engine_protocol::{AgentId, JobId};

use super::*;

fn job(kind: JobKind, status: JobStatus, exit_code: Option<i32>) -> JobInfo {
    JobInfo {
        job_id: JobId::from("job_42"),
        kind,
        label: "cargo test\n--workspace".into(),
        owner: AgentId::main(),
        agent_id: None,
        status,
        exit_code,
        started_at: 1,
        finished_at: Some(2),
        output_tail: "SECRET TAIL".into(),
    }
}

fn todo(content: &str, status: TodoStatus) -> TodoItem {
    TodoItem {
        content: content.into(),
        active_form: String::new(),
        status,
    }
}

fn body(reminder: &str) -> &str {
    reminder
        .strip_prefix("<system-reminder>\n")
        .and_then(|rest| rest.strip_suffix("\n</system-reminder>"))
        .unwrap_or_else(|| panic!("not wrapped: {reminder}"))
}

#[test]
fn every_reminder_is_wrapped_rendered_and_non_empty() {
    let doc = InstructionDoc {
        label: "Directory instructions (AGENTS.md)".into(),
        path: "/p/ui/AGENTS.md".into(),
        content: "Use Svelte 5 runes.".into(),
    };
    let reminders = [
        todo_nudge(),
        todo_state(&[todo("Write tests", TodoStatus::Pending)]),
        plan_mode_active(),
        plan_approved("1. Do it"),
        files_changed_externally(&["src/lib.rs".into()]),
        nested_instructions(&[doc]),
        job_finished(&job(JobKind::Shell, JobStatus::Completed, Some(0))),
        steering(&["also update docs".into()]),
        verification_required(
            &VerificationOutcome::Unverified {
                reason: "no checks ran".into(),
            },
            VerificationMode::Strict,
        ),
        auto_check_failed("test parser::nested ... FAILED"),
        interrupted(),
    ];
    for reminder in &reminders {
        let text = body(reminder);
        assert!(!text.trim().is_empty(), "{reminder}");
        assert!(!text.contains("{{"), "{reminder}");
        assert_eq!(text, text.trim(), "{reminder}");
    }
}

#[test]
fn wrap_trims_the_body() {
    assert_eq!(
        wrap_reminder("\n hi \n"),
        "<system-reminder>\nhi\n</system-reminder>"
    );
}

#[test]
fn todo_state_marks_each_status() {
    let out = todo_state(&[
        todo("Read code", TodoStatus::Completed),
        todo("Fix bug\nquickly", TodoStatus::InProgress),
        todo("Run tests", TodoStatus::Pending),
    ]);
    assert!(
        out.contains("[x] Read code\n[~] Fix bug quickly\n[ ] Run tests"),
        "{out}"
    );
}

#[test]
fn job_finished_names_the_job_and_omits_output() {
    let out = job_finished(&job(JobKind::Shell, JobStatus::Failed, Some(101)));
    for expected in [
        "shell command",
        "job_42",
        "cargo test --workspace",
        "failed",
        "Exit code: 101",
        "JobOutput",
    ] {
        assert!(out.contains(expected), "missing {expected:?}: {out}");
    }
    assert!(!out.contains("SECRET TAIL"));
    let agent = job_finished(&job(JobKind::Agent, JobStatus::Completed, None));
    assert!(agent.contains("background agent"), "{agent}");
    assert!(!agent.contains("Exit code"), "{agent}");
}

#[test]
fn steering_quotes_every_message() {
    let out = steering(&["stop using unwrap".into(), "line one\n\nline two".into()]);
    assert!(
        out.contains("> stop using unwrap\n\n> line one\n>\n> line two"),
        "{out}"
    );
}

#[test]
fn verification_required_reports_status_reason_and_mode() {
    let out = verification_required(
        &VerificationOutcome::Failed {
            reason: "cargo test failed".into(),
        },
        VerificationMode::Auto,
    );
    assert!(out.contains("Verification status: failed"), "{out}");
    assert!(out.contains("Reason: cargo test failed"), "{out}");
    assert!(out.contains("Verification mode: auto"), "{out}");
    assert!(out.contains("Verify"), "{out}");
    let no_reason = verification_required(
        &VerificationOutcome::NotApplicable,
        VerificationMode::Strict,
    );
    assert!(!no_reason.contains("Reason:"), "{no_reason}");
}

#[test]
fn content_reminders_embed_their_payload() {
    assert!(plan_approved("## Plan\n1. Edit").contains("## Plan\n1. Edit"));
    assert!(files_changed_externally(&["a.rs".into(), "b.rs".into()]).contains("- a.rs\n- b.rs"));
    assert!(auto_check_failed("\nerror[E0308]\n").contains("error[E0308]"));
    let nested = nested_instructions(&[InstructionDoc {
        label: "Directory instructions".into(),
        path: "/p/ui/AGENTS.md".into(),
        content: "Use runes.".into(),
    }]);
    assert!(
        nested.contains("# Directory instructions — /p/ui/AGENTS.md\n\nUse runes."),
        "{nested}"
    );
    assert!(plan_mode_active().contains("ExitPlanMode"));
    assert!(todo_nudge().contains("TodoWrite"));
}
