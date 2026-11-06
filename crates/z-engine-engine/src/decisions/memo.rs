//! `UseMemo`: small facts a session's decision uses keep between seams,
//! such as the fingerprints of things they already offered, so a hint,
//! card or redirect is not repeated. Bounded; the oldest entries go first.

use std::collections::VecDeque;
use std::sync::Mutex;

use z_engine_config::FeatureId;

use crate::sync::lock;

const CAPACITY: usize = 512;

#[derive(Debug, Default)]
pub(crate) struct UseMemo {
    entries: Mutex<VecDeque<(FeatureId, String)>>,
}

impl UseMemo {
    /// Remembers `key` for `feature`; false when it was already known.
    pub(crate) fn insert(&self, feature: FeatureId, key: &str) -> bool {
        let mut entries = lock(&self.entries);
        if entries.iter().any(|(f, k)| *f == feature && k == key) {
            return false;
        }
        if entries.len() == CAPACITY {
            entries.pop_front();
        }
        entries.push_back((feature, key.to_string()));
        true
    }

    pub(crate) fn contains(&self, feature: FeatureId, key: &str) -> bool {
        let entries = lock(&self.entries);
        entries.iter().any(|(f, k)| *f == feature && k == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_per_feature_and_the_oldest_go_first() {
        let memo = UseMemo::default();
        assert!(memo.insert(FeatureId::DecisionsHints, "pdf"));
        assert!(!memo.insert(FeatureId::DecisionsHints, "pdf"));
        assert!(!memo.contains(FeatureId::DecisionsReviewSuggest, "pdf"));
        for index in 0..CAPACITY {
            memo.insert(FeatureId::DecisionsReviewSuggest, &index.to_string());
        }
        assert!(!memo.contains(FeatureId::DecisionsHints, "pdf"));
        assert!(memo.contains(FeatureId::DecisionsReviewSuggest, "0"));
    }
}
