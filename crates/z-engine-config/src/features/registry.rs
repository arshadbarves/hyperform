//! The feature registry: what every feature is, how mature it is, who owns
//! it, and when it may graduate. The Experimental tab renders this table.

use serde::Serialize;
use ts_rs::TS;

use super::FeatureId;

const OWNER: &str = "z-engine core";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum FeatureStage {
    /// Off unless the user turns it on in `[experimental]`.
    Experimental,
    /// Always on; its `[experimental]` key is ignored with a warning.
    Stable,
}

/// Features that share settings; the Experimental tab shows a group's
/// settings card while any of its features runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum FeatureGroup {
    /// Uses of the decision model (`[decisions]`).
    Decisions,
    /// Work plans: tasks, ledgers and task packets (`[work]`).
    Work,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct FeatureSpec {
    pub id: FeatureId,
    pub title: &'static str,
    pub summary: &'static str,
    pub stage: FeatureStage,
    pub group: FeatureGroup,
    /// `shadow` is allowed: the feature runs and records, never acts.
    pub supports_shadow: bool,
    /// Built in this version. A registered feature that is not built yet
    /// stays off and is not listed.
    pub available: bool,
    /// Another feature this one builds on: it stays off unless that one runs.
    pub requires: Option<FeatureId>,
    pub owner: &'static str,
    /// The measurable bar for becoming Stable (results go in docs/status.md).
    pub graduation_criteria: &'static str,
}

impl FeatureSpec {
    /// Marks the feature as built in this version.
    pub const fn available(self) -> Self {
        Self {
            available: true,
            ..self
        }
    }

    pub const fn requires(self, id: FeatureId) -> Self {
        Self {
            requires: Some(id),
            ..self
        }
    }
}

const fn decision(
    id: FeatureId,
    title: &'static str,
    summary: &'static str,
    graduation_criteria: &'static str,
) -> FeatureSpec {
    FeatureSpec {
        id,
        title,
        summary,
        stage: FeatureStage::Experimental,
        group: FeatureGroup::Decisions,
        supports_shadow: true,
        available: false,
        requires: None,
        owner: OWNER,
        graduation_criteria,
    }
}

const fn work(
    id: FeatureId,
    title: &'static str,
    summary: &'static str,
    graduation_criteria: &'static str,
) -> FeatureSpec {
    FeatureSpec {
        id,
        title,
        summary,
        stage: FeatureStage::Experimental,
        group: FeatureGroup::Work,
        supports_shadow: false,
        available: false,
        requires: None,
        owner: OWNER,
        graduation_criteria,
    }
}

/// Every feature, in [`FeatureId::ALL`] order.
pub const FEATURES: [FeatureSpec; 23] = [
    decision(
        FeatureId::DecisionsCompaction,
        "Relevance-aware compaction",
        "When older tool results are cleared to free context, clear the ones the current task no longer needs first. Results the task depends on are always kept.",
        "At least 15% fewer context tokens at equal task success, zero loss of mandatory context, p95 at or under 250 ms, fallbacks at or under 5%.",
    )
    .available(),
    decision(
        FeatureId::DecisionsTaskView,
        "Task-scoped history",
        "When a new task starts in a long chat, send only the earlier exchanges it needs; the rest is set aside behind a one-line index the model can read back.",
        "At least 30% fewer input tokens per turn on chats of 5 or more tasks, no drop in task success, readbacks at or under 5%, and cost per task no higher once prompt caching is counted.",
    )
    .available(),
    decision(
        FeatureId::DecisionsSessionContext,
        "First-request context",
        "On a chat's first request, order the repository map by relevance and preload the MCP tools that fit the request.",
        "Fewer tool rounds before the first useful read at equal task success, with no rise in first-request tokens.",
    )
    .available(),
    decision(
        FeatureId::DecisionsRouting,
        "Per-task routing",
        "Choose a reasoning effort when a new task starts (only when you have not set one) and keep it through follow-ups. With model switching allowed, also choose the main or fast model, for subagents that inherit their model too.",
        "Lower cost per task at equal task success, with the share of input read from the prompt cache not lower than with the flag off.",
    )
    .available(),
    decision(
        FeatureId::DecisionsLoopGuard,
        "Loop guard",
        "Notice repeated calls or the same failure after an edit and remind the model to change approach; after repeated reminders, ask you.",
        "Catches at least 80% of the loop fixtures with at most one needless reminder per 50 turns.",
    )
    .available(),
    decision(
        FeatureId::DecisionsCompletionCheck,
        "Completion check",
        "Spot final messages that claim success or passing tests while nothing was checked.",
        "At least 90% precision on claim fixtures; a turn is never marked Verified by this feature.",
    )
    .available(),
    decision(
        FeatureId::DecisionsRisk,
        "Risk review",
        "Ask before allowed calls that look risky and flag likely prompt injection in web and MCP results. It never loosens a permission.",
        "No decision ever loosened (enforced by structure) and at most one needless escalation per 50 calls.",
    )
    .available(),
    decision(
        FeatureId::DecisionsOutputTrim,
        "Relevant output trimming",
        "Keep the parts of long command output that matter to the task beside its head and tail; the full output is still saved.",
        "At least 20% fewer tool-output tokens with no rise in re-runs of the same command.",
    )
    .available(),
    decision(
        FeatureId::DecisionsSearchRank,
        "Search ranking",
        "When a search hits its result cap, order matches by relevance before the cut.",
        "At least 25% fewer follow-up searches after a capped result.",
    )
    .available(),
    decision(
        FeatureId::DecisionsPrefetch,
        "File prefetch",
        "When you send a request, attach files the model will very likely read, within a token budget.",
        "Fewer tool rounds before the first edit, with at least 60% of prefetched files read or edited in the task.",
    )
    .available(),
    decision(
        FeatureId::DecisionsPlanSuggest,
        "Plan suggestion",
        "Offer to plan first when a request looks large or risky; plan mode starts only if you accept.",
        "At least half of the suggestions accepted, and at most one suggestion per 20 small requests.",
    )
    .available(),
    decision(
        FeatureId::DecisionsUserSignal,
        "Correction signal",
        "When you seem to be correcting the agent, remind it to restate its plan and ask before large edits.",
        "At least 85% precision on labeled correction messages.",
    )
    .available(),
    decision(
        FeatureId::DecisionsHints,
        "Skill and agent hints",
        "When you send a request, remind the model of an installed skill or agent that looks relevant; the model still decides.",
        "Hinted skills or agents used in at least 40% of hinted tasks.",
    )
    .available(),
    decision(
        FeatureId::DecisionsMemorySuggest,
        "Standing-rule suggestions",
        "When a message sets a rule for the rest of the chat, offer to save it to memory or project rules.",
        "At least half of the offers accepted.",
    )
    .available(),
    decision(
        FeatureId::DecisionsQuestionCheck,
        "Repeated-question check",
        "Before a question reaches you, check whether the chat already answered it and point the model there first.",
        "At least 90% precision on repeated-question fixtures.",
    )
    .available(),
    decision(
        FeatureId::DecisionsReviewSuggest,
        "Review suggestion",
        "When a turn touched risky areas (auth, migrations, CI, deletions), suggest running the review agent.",
        "At least 30% of suggestions accepted, and at most one suggestion per 10 turns.",
    )
    .available(),
    decision(
        FeatureId::DecisionsPetMood,
        "Pet mood from turn tone",
        "At the end of a turn, let the pet show how the turn went; the event-based mood stays the fallback.",
        "At least 80% agreement with hand labels of turn tone.",
    )
    .available(),
    decision(
        FeatureId::DecisionsInboxPriority,
        "Inbox priority",
        "Order Activity inbox items by urgency, and run your Notification hooks only for urgent ones.",
        "At least 80% agreement with hand-ranked urgency, with no urgent item left without a notification.",
    )
    .available(),
    decision(
        FeatureId::DecisionsCheckSelect,
        "Check selection",
        "In auto verification, choose which configured checks matter for the changed files.",
        "Check time cut by at least 30% with no failing check skipped.",
    )
    .available(),
    decision(
        FeatureId::DecisionsCustomRules,
        "Custom decision rules",
        "Your own typed questions before tool calls or prompts; they can ask, notify or remind, never allow.",
        "p95 at or under 250 ms, and no rule ever changes a permission (enforced by structure).",
    )
    .available(),
    decision(
        FeatureId::DecisionsSecretScreen,
        "Secret screening",
        "Flag further likely credentials in tool output after the pattern detectors, and ask before the next request sends them.",
        "At least 95% recall on credential fixtures with at most one false alarm per 100 tool results.",
    )
    .available(),
    work(
        FeatureId::WorkPlans,
        "Work plans",
        "Keep a chat's work as a plan of tasks with acceptance criteria, decisions and evidence. A task is done only when its checks pass on the current code, and the model gets the task's context back after compaction.",
        "On the long-task fixtures: fewer tasks wrongly claimed done, no rise in tokens per task, the full packet re-sent only on a new revision, and requests identical to the flag off when no plan is attached.",
    )
    .available(),
    work(
        FeatureId::WorkParallel,
        "Parallel tasks",
        "Let independent tasks of an approved plan run at the same time in helpers, each in its own worktree, and bring their changes back one at a time. Needs Work plans.",
        "On the parallel fixtures: less wall time than serial with no rise in failed or reopened tasks, no change applied without your approval or a passing integration check, and every write outside a task's scope asked first.",
    )
    .requires(FeatureId::WorkPlans)
    .available(),
];

impl FeatureId {
    pub fn spec(self) -> &'static FeatureSpec {
        let index = Self::ALL
            .iter()
            .position(|id| *id == self)
            .unwrap_or_default();
        &FEATURES[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_lists_every_id_once_in_order() {
        let ids: Vec<FeatureId> = FEATURES.iter().map(|spec| spec.id).collect();
        assert_eq!(ids, FeatureId::ALL);
        for id in FeatureId::ALL {
            assert_eq!(id.spec().id, id);
        }
    }

    #[test]
    fn every_spec_is_complete() {
        for spec in FEATURES {
            let texts = [
                spec.title,
                spec.summary,
                spec.owner,
                spec.graduation_criteria,
            ];
            assert!(texts.iter().all(|text| !text.trim().is_empty()), "{spec:?}");
        }
    }

    #[test]
    fn work_plans_has_no_shadow_mode() {
        let spec = FeatureId::WorkPlans.spec();
        assert_eq!(spec.group, FeatureGroup::Work);
        assert_eq!(spec.stage, FeatureStage::Experimental);
        assert!(spec.available && !spec.supports_shadow);
    }

    #[test]
    fn parallel_tasks_build_on_work_plans() {
        let spec = FeatureId::WorkParallel.spec();
        assert_eq!(spec.group, FeatureGroup::Work);
        assert_eq!(spec.requires, Some(FeatureId::WorkPlans));
        assert!(spec.available && !spec.supports_shadow);
        assert!(FEATURES.iter().all(|spec| spec.requires != Some(spec.id)));
        assert_eq!(
            serde_json::to_value(spec).unwrap()["requires"],
            "work_plans"
        );
    }

    #[test]
    fn specs_serialize_camel_case() {
        let json = serde_json::to_value(FEATURES[0].available()).unwrap();
        assert_eq!(json["id"], "decisions_compaction");
        assert_eq!(json["stage"], "experimental");
        assert_eq!(json["supportsShadow"], true);
        assert_eq!(json["available"], true);
        assert!(json["graduationCriteria"].is_string());
    }
}
