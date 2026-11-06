//! Feature ids: the keys of `[experimental]`. Every new feature gets one.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum FeatureId {
    DecisionsCompaction,
    DecisionsTaskView,
    DecisionsSessionContext,
    DecisionsRouting,
    DecisionsLoopGuard,
    DecisionsCompletionCheck,
    DecisionsRisk,
    DecisionsOutputTrim,
    DecisionsSearchRank,
    DecisionsPrefetch,
    DecisionsPlanSuggest,
    DecisionsUserSignal,
    DecisionsHints,
    DecisionsMemorySuggest,
    DecisionsQuestionCheck,
    DecisionsReviewSuggest,
    DecisionsPetMood,
    DecisionsInboxPriority,
    DecisionsCheckSelect,
    DecisionsCustomRules,
    DecisionsSecretScreen,
    WorkPlans,
    WorkParallel,
}

impl FeatureId {
    /// Every feature, in the order the Experimental tab lists them.
    pub const ALL: [FeatureId; 23] = [
        Self::DecisionsCompaction,
        Self::DecisionsTaskView,
        Self::DecisionsSessionContext,
        Self::DecisionsRouting,
        Self::DecisionsLoopGuard,
        Self::DecisionsCompletionCheck,
        Self::DecisionsRisk,
        Self::DecisionsOutputTrim,
        Self::DecisionsSearchRank,
        Self::DecisionsPrefetch,
        Self::DecisionsPlanSuggest,
        Self::DecisionsUserSignal,
        Self::DecisionsHints,
        Self::DecisionsMemorySuggest,
        Self::DecisionsQuestionCheck,
        Self::DecisionsReviewSuggest,
        Self::DecisionsPetMood,
        Self::DecisionsInboxPriority,
        Self::DecisionsCheckSelect,
        Self::DecisionsCustomRules,
        Self::DecisionsSecretScreen,
        Self::WorkPlans,
        Self::WorkParallel,
    ];

    /// The `[experimental]` key.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DecisionsCompaction => "decisions_compaction",
            Self::DecisionsTaskView => "decisions_task_view",
            Self::DecisionsSessionContext => "decisions_session_context",
            Self::DecisionsRouting => "decisions_routing",
            Self::DecisionsLoopGuard => "decisions_loop_guard",
            Self::DecisionsCompletionCheck => "decisions_completion_check",
            Self::DecisionsRisk => "decisions_risk",
            Self::DecisionsOutputTrim => "decisions_output_trim",
            Self::DecisionsSearchRank => "decisions_search_rank",
            Self::DecisionsPrefetch => "decisions_prefetch",
            Self::DecisionsPlanSuggest => "decisions_plan_suggest",
            Self::DecisionsUserSignal => "decisions_user_signal",
            Self::DecisionsHints => "decisions_hints",
            Self::DecisionsMemorySuggest => "decisions_memory_suggest",
            Self::DecisionsQuestionCheck => "decisions_question_check",
            Self::DecisionsReviewSuggest => "decisions_review_suggest",
            Self::DecisionsPetMood => "decisions_pet_mood",
            Self::DecisionsInboxPriority => "decisions_inbox_priority",
            Self::DecisionsCheckSelect => "decisions_check_select",
            Self::DecisionsCustomRules => "decisions_custom_rules",
            Self::DecisionsSecretScreen => "decisions_secret_screen",
            Self::WorkPlans => "work_plans",
            Self::WorkParallel => "work_parallel",
        }
    }

    pub fn parse(key: &str) -> Option<FeatureId> {
        Self::ALL.into_iter().find(|id| id.as_str() == key)
    }
}

impl std::fmt::Display for FeatureId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_match_serde_and_round_trip() {
        for id in FeatureId::ALL {
            let json = serde_json::to_value(id).unwrap();
            assert_eq!(json, id.as_str(), "serde name of {id:?}");
            assert_eq!(FeatureId::parse(id.as_str()), Some(id));
        }
        assert_eq!(FeatureId::parse("decisions_nope"), None);
    }
}
