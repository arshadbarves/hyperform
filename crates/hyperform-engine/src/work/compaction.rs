//! State-aware compaction while a plan is attached to the main agent: its
//! task steps are recorded first, the summarizer gets an addendum asking
//! for candidate discoveries in a trailing JSON block, those are recorded
//! unconfirmed with compaction provenance, and the focus packet follows
//! the summary. Without a plan the summary request is unchanged.

use std::path::Path;
use std::sync::Arc;

use hyperform_llm::SystemBlock;
use hyperform_prompts::auxiliary::COMPACT_WORK;
use hyperform_protocol::AgentId;
use hyperform_protocol::work::{Actor, Discovery, EntryBody, WorkCause};
use hyperform_work::{EntryDraft, Proposal, Submission};
use serde::Deserialize;

use super::evidence::record_steps;
use super::facts::instant;
use super::ids::Ids;
use super::inject::packet_now;
use super::lifecycle::{attached, report};
use super::plan::PlanHandle;
use crate::session::SessionCore;

/// Discoveries taken from one summary at most.
const MAX_DISCOVERIES: usize = 10;
const FENCE: &str = "```json";

pub(crate) struct CompactWork {
    plan: Arc<PlanHandle>,
}

impl CompactWork {
    /// For a main-agent compaction while a plan is attached.
    pub(crate) fn begin(core: &SessionCore, agent_id: &AgentId) -> Option<Self> {
        if !agent_id.is_main() {
            return None;
        }
        let plan = attached(core)?;
        let todos = core.with_state(|state| state.todos_of(agent_id).to_vec());
        if !todos.is_empty() {
            record_steps(core, agent_id, &todos, WorkCause::Compaction, false);
        }
        Some(Self { plan })
    }

    pub(crate) fn system(&self) -> SystemBlock {
        SystemBlock::new(COMPACT_WORK)
    }

    /// Records the discoveries; returns the packet for the summary message.
    pub(crate) async fn finish(
        self,
        core: &SessionCore,
        root: &Path,
        discoveries: Vec<Discovery>,
    ) -> Option<String> {
        if !discoveries.is_empty() {
            let result = self.plan.submit(&instant(core), |state| {
                let mut ids = Ids::new(state);
                let proposals = discoveries
                    .into_iter()
                    .map(|discovery| {
                        Proposal::Record(Box::new(EntryDraft {
                            id: Some(ids.entry()),
                            proposed: true,
                            ..EntryDraft::new(EntryBody::Discovery(discovery))
                        }))
                    })
                    .collect();
                Ok(Submission::new(
                    Actor::Compaction,
                    WorkCause::Compaction,
                    proposals,
                ))
            });
            report(
                core,
                result,
                "Discoveries from the summary were not recorded",
            );
        }
        core.work.packet_due("compaction");
        packet_now(core, root, "compaction").await
    }
}

#[derive(Deserialize)]
struct Block {
    #[serde(default)]
    discoveries: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    text: String,
    #[serde(default)]
    source: Option<String>,
}

/// The summary without its trailing JSON block, and the block's
/// discoveries (none when it is missing or malformed).
pub(crate) fn split_discoveries(text: &str) -> (String, Vec<Discovery>) {
    let whole = || (text.trim().to_string(), Vec::new());
    let Some(start) = text.rfind(FENCE) else {
        return whole();
    };
    let rest = &text[start + FENCE.len()..];
    let Some(end) = rest.find("```") else {
        return whole();
    };
    let summary = format!("{}{}", &text[..start], &rest[end + 3..]);
    let discoveries = serde_json::from_str::<Block>(rest[..end].trim())
        .map(|block| block.discoveries)
        .unwrap_or_default()
        .into_iter()
        .filter(|item| !item.text.trim().is_empty())
        .take(MAX_DISCOVERIES)
        .map(|item| Discovery {
            text: item.text.trim().to_string(),
            source: item.source.filter(|source| !source.trim().is_empty()),
        })
        .collect();
    (summary.trim().to_string(), discoveries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_json_block_is_split_from_the_summary() {
        let text = "Summary line.\n\n```json\n{\"discoveries\": [{\"text\": \"Retries ignore 429\", \"source\": \"src/client.rs\"}, {\"text\": \" \"}]}\n```\n";
        let (summary, found) = split_discoveries(text);
        assert_eq!(summary, "Summary line.");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source.as_deref(), Some("src/client.rs"));
        let (summary, found) = split_discoveries("Only prose.");
        assert_eq!((summary.as_str(), found.len()), ("Only prose.", 0));
        let (summary, found) = split_discoveries("Prose\n```json\nnot json\n```");
        assert_eq!((summary.as_str(), found.len()), ("Prose", 0));
    }
}
