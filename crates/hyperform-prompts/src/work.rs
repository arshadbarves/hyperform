//! Layouts of work-plan text the model reads: the task packet, the task
//! list, a dispatched helper's first message and the audit request. A list
//! placeholder (`Label:{{name?}}`) takes a value that starts with a newline
//! and holds one `- ...` line per item, so the label line disappears with
//! an empty list.

/// One task packet (`TaskGet`, and inside [`crate::reminders::WORK_PACKET`]).
pub const PACKET: &str = include_str!("../prompts/work/packet.md");

/// The `TaskList` result: tasks grouped by state, the next ready task and a
/// health line.
pub const TASK_LIST: &str = include_str!("../prompts/work/task-list.md");

/// A helper dispatched with `Agent { task_id }`: `{{task}}` is the task id,
/// `{{packet}}` its packet from [`PACKET`], `{{prompt}}` the caller's
/// instructions. It carries the result contract (`TaskUpdate` `report`).
pub const HELPER: &str = include_str!("../prompts/work/helper.md");

/// The audit agent's one request; `{{plan}}` is the plan as JSON.
pub const AUDIT_REQUEST: &str = include_str!("../prompts/work/audit-request.md");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_hold_their_required_placeholders() {
        for key in ["{{goal}}", "{{task}}", "{{title}}", "{{status}}"] {
            assert!(PACKET.contains(key), "packet lacks {key}");
        }
        for key in ["{{goal}}", "{{status}}", "{{revision}}", "{{health}}"] {
            assert!(TASK_LIST.contains(key), "task list lacks {key}");
        }
        for key in ["{{task}}", "{{packet}}", "{{prompt}}", "report"] {
            assert!(HELPER.contains(key), "helper brief lacks {key}");
        }
        assert!(AUDIT_REQUEST.contains("{{plan}}"));
    }
}
