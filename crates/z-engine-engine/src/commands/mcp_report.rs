//! `/mcp`: every server's state, what it offers, and why it failed (with
//! the tail of a stdio server's stderr).

use z_engine_integrations::{McpServerState, McpServerStatus};

use crate::session::SessionCore;

const STDERR_LINES: usize = 10;

pub(crate) fn mcp_report(core: &SessionCore) -> String {
    let statuses: Vec<McpServerStatus> = core
        .mcp
        .managers()
        .iter()
        .flat_map(|manager| manager.status())
        .collect();
    let withheld = core.settings().withheld.clone();
    let mut out = if statuses.is_empty() {
        String::from("No MCP servers are configured for this chat.")
    } else {
        render(&statuses)
    };
    if withheld.iter().any(|what| what == "MCP servers") {
        out.push_str(
            "\n\nThis project defines MCP servers that stay off until you trust the workspace.",
        );
    }
    out
}

fn render(statuses: &[McpServerStatus]) -> String {
    let mut out = String::from(
        "| Server | State | Tools | Resources | Prompts |\n|---|---|---:|---:|---:|\n",
    );
    for status in statuses {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            status.name,
            state_label(&status.state),
            status.tool_count,
            status.resource_count,
            status.prompt_count
        ));
    }
    for status in statuses {
        let error = match &status.state {
            McpServerState::Failed(reason) => Some(reason.as_str()),
            _ => status.error.as_deref(),
        };
        if error.is_none() && status.stderr_tail.is_empty() {
            continue;
        }
        out.push_str(&format!("\n**{}**", status.name));
        if let Some(error) = error {
            out.push_str(&format!(": {error}"));
        }
        out.push('\n');
        let skip = status.stderr_tail.len().saturating_sub(STDERR_LINES);
        let tail: Vec<&str> = status.stderr_tail[skip..]
            .iter()
            .map(String::as_str)
            .collect();
        if !tail.is_empty() {
            out.push_str(&format!("```text\n{}\n```\n", tail.join("\n")));
        }
    }
    out.trim_end().to_string()
}

pub(crate) fn state_label(state: &McpServerState) -> &'static str {
    match state {
        McpServerState::Connecting => "connecting",
        McpServerState::Ready => "ready",
        McpServerState::Failed(_) => "failed",
        McpServerState::Disabled => "disabled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_show_their_reason_and_stderr() {
        let statuses = [
            McpServerStatus {
                name: "ok".into(),
                state: McpServerState::Ready,
                tool_count: 3,
                resource_count: 1,
                prompt_count: 2,
                error: None,
                stderr_tail: Vec::new(),
            },
            McpServerStatus {
                name: "bad".into(),
                state: McpServerState::Failed("exited with 3".into()),
                tool_count: 0,
                resource_count: 0,
                prompt_count: 0,
                error: Some("exited with 3".into()),
                stderr_tail: vec!["boom".into()],
            },
        ];
        let out = render(&statuses);
        assert!(out.contains("| ok | ready | 3 | 1 | 2 |"), "{out}");
        assert!(
            out.contains("**bad**: exited with 3") && out.contains("boom"),
            "{out}"
        );
    }
}
