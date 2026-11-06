//! What the `Agent` tool returns for a finished foreground agent: the
//! report text and a footer with duration, tool calls, tokens, cost and
//! changed files (or the pending worktree the parent may apply).

use std::path::Path;
use std::time::Duration;

use z_engine_host::relative_display;
use z_engine_protocol::{AgentStatus, TurnOutcome, WorktreeState};
use z_engine_tools::SpawnOutcome;

use super::child::ChildReport;

/// Changed files listed in the footer before summarizing the rest.
const LISTED_FILES: usize = 10;

/// A failed agent without any report is a tool error.
pub(crate) fn foreground_outcome(
    report: &ChildReport,
    root: &Path,
) -> Result<SpawnOutcome, String> {
    let agent = &report.info.agent_id;
    let text = match (&report.outcome, report.text.trim()) {
        (TurnOutcome::Failed { message }, "") => {
            return Err(format!("agent {agent} failed: {message}"));
        }
        (TurnOutcome::Cancelled | TurnOutcome::Interrupted, "") => {
            format!("Agent {agent} was cancelled before it reported.")
        }
        (_, "") => format!("Agent {agent} finished without a report."),
        (_, text) => text.to_string(),
    };
    Ok(SpawnOutcome {
        agent_id: agent.clone(),
        job_id: None,
        text,
        footer: footer(report, root),
    })
}

pub(crate) fn footer(report: &ChildReport, root: &Path) -> String {
    let info = &report.info;
    let mut lines = vec![format!(
        "[{} agent {} in {}; {} tool call(s), {} tokens, ${:.4}]",
        info.agent_type,
        status_label(info.status),
        duration(report.duration),
        report.tool_calls,
        report.usage.total_tokens(),
        report.cost_usd
    )];
    if let Some(error) = &info.error {
        lines.push(format!("Error: {error}"));
    }
    match &info.worktree {
        Some(worktree) if worktree.state == WorktreeState::Pending => {
            lines.push(format!(
                "Worktree changes pending: {} file(s) on branch {}.\n{}\nReview the report and \
                 call ApplyAgentChanges with agent_id {} to merge them into the project.",
                worktree.files_changed, worktree.branch, worktree.diffstat, info.agent_id
            ));
        }
        Some(worktree) if worktree.state == WorktreeState::Empty => {
            lines.push("The agent changed no files in its worktree.".to_string());
        }
        Some(_) => {}
        None if report.written.is_empty() => lines.push("Files changed: none".to_string()),
        None => {
            let mut shown: Vec<String> = report
                .written
                .iter()
                .take(LISTED_FILES)
                .map(|path| relative_display(root, path))
                .collect();
            if report.written.len() > LISTED_FILES {
                shown.push(format!("and {} more", report.written.len() - LISTED_FILES));
            }
            lines.push(format!("Files changed: {}", shown.join(", ")));
        }
    }
    lines.join("\n")
}

fn status_label(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Running | AgentStatus::Waiting => "running",
        AgentStatus::Completed => "completed",
        AgentStatus::Failed => "failed",
        AgentStatus::Cancelled => "cancelled",
    }
}

fn duration(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("{:.1}s", elapsed.as_secs_f64())
    } else {
        format!("{}m{:02}s", secs / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_are_short() {
        assert_eq!(duration(Duration::from_millis(1500)), "1.5s");
        assert_eq!(duration(Duration::from_secs(125)), "2m05s");
    }
}
