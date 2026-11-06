//! A small least-recently-used cache of model answers, keyed by a hash of
//! (schema version, question, state, provider revision). Answers are
//! cached before calibration, so a changed calibration applies at once.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Mutex, PoisonError};

use crate::answer::Answer;
use crate::question::Question;

/// Bump when the question encoding or answer decoding changes.
pub const CACHE_SCHEMA_VERSION: u32 = 1;
const DEFAULT_CAPACITY: usize = 512;

pub fn cache_key(question: &Question, state: &str, revision: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    CACHE_SCHEMA_VERSION.hash(&mut hasher);
    question.instructions.hash(&mut hasher);
    question.form.hash(&mut hasher);
    state.hash(&mut hasher);
    revision.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug)]
pub struct DecisionCache {
    capacity: usize,
    inner: Mutex<Entries>,
}

#[derive(Debug, Default)]
struct Entries {
    map: HashMap<u64, (Answer, u64)>,
    tick: u64,
}

impl Default for DecisionCache {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl DecisionCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            inner: Mutex::default(),
        }
    }

    pub fn get(&self, key: u64) -> Option<Answer> {
        let mut entries = self.lock();
        entries.tick += 1;
        let tick = entries.tick;
        let (answer, used) = entries.map.get_mut(&key)?;
        *used = tick;
        Some(answer.clone())
    }

    pub fn put(&self, key: u64, answer: Answer) {
        let mut entries = self.lock();
        entries.tick += 1;
        let tick = entries.tick;
        entries.map.insert(key, (answer, tick));
        if entries.map.len() > self.capacity {
            let oldest = entries
                .map
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| *key);
            if let Some(oldest) = oldest {
                entries.map.remove(&oldest);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.lock().map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Entries are replaced whole, so a poisoned map is still consistent.
    fn lock(&self) -> std::sync::MutexGuard<'_, Entries> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::AbstainReason;

    fn answer(name: &str) -> Answer {
        Answer::abstained(name, AbstainReason::Rules, "test")
    }

    #[test]
    fn evicts_the_least_recently_used() {
        let cache = DecisionCache::new(2);
        cache.put(1, answer("one"));
        cache.put(2, answer("two"));
        assert!(cache.get(1).is_some());
        cache.put(3, answer("three"));
        assert!(cache.get(2).is_none(), "two was used least recently");
        assert!(cache.get(1).is_some() && cache.get(3).is_some());
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn keys_depend_on_state_and_revision() {
        let template = "Q?\n- yes: y\n- no: n\n";
        let question = Question::yes_no("q", template).unwrap();
        let key = cache_key(&question, "{}", "r1");
        assert_eq!(key, cache_key(&question, "{}", "r1"));
        assert_ne!(key, cache_key(&question, "{\"a\":1}", "r1"));
        assert_ne!(key, cache_key(&question, "{}", "r2"));
    }
}
