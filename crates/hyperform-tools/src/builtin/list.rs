//! Every built-in tool, in the order of `names::ALL`.

use std::sync::Arc;

use super::*;
use crate::ports::AgentCard;
use crate::tool::Tool;

pub(crate) fn builtin_tools(agent_catalog: Vec<AgentCard>) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ReadTool),
        Arc::new(WriteTool),
        Arc::new(EditTool),
        Arc::new(MultiEditTool),
        Arc::new(NotebookEditTool),
        Arc::new(GlobTool),
        Arc::new(GrepTool),
        Arc::new(BashTool),
        Arc::new(JobOutputTool),
        Arc::new(JobKillTool),
        Arc::new(WebFetchTool),
        Arc::new(WebSearchTool),
        Arc::new(TodoWriteTool),
        Arc::new(AskUserQuestionTool),
        Arc::new(ExitPlanModeTool::default()),
        Arc::new(TaskCreateTool),
        Arc::new(TaskUpdateTool),
        Arc::new(TaskGetTool),
        Arc::new(TaskListTool),
        Arc::new(PlanRecordTool),
        Arc::new(SkillTool),
        Arc::new(AgentTool::new(agent_catalog)),
        Arc::new(ApplyAgentChangesTool),
        Arc::new(VerifyTool),
        Arc::new(LspTool),
        Arc::new(ListMcpResourcesTool),
        Arc::new(ReadMcpResourceTool),
    ]
}
