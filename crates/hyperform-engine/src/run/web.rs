//! Whether a run has read web content. It is set from the first WebFetch
//! or WebSearch result, a subagent report (its `Agent` result or
//! `JobOutput`) from a run that read the web, a spawn by a run that
//! already had, or a resume of an agent whose earlier run had
//! (`AgentInfo::read_web`), and stays set until the run ends; every work
//! proposal of such a run reaches the plan as untrusted input.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use hyperform_tools::names;

#[derive(Debug, Default)]
pub(crate) struct WebTaint(AtomicBool);

impl WebTaint {
    /// A child's taint: it starts set when the run that spawned it read
    /// the web, since its prompt may carry that content.
    pub(crate) fn inherit(parent: &Self) -> Arc<Self> {
        Arc::new(Self(AtomicBool::new(parent.is_set())))
    }

    pub(crate) fn mark(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub(crate) fn is_set(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// The tools whose results carry web content.
pub(crate) fn reads_web(tool: &str) -> bool {
    tool == names::WEB_FETCH || tool == names::WEB_SEARCH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_child_inherits_what_its_parent_read() {
        let parent = WebTaint::default();
        assert!(!WebTaint::inherit(&parent).is_set());
        parent.mark();
        assert!(WebTaint::inherit(&parent).is_set());
        assert!(reads_web(names::WEB_FETCH) && reads_web(names::WEB_SEARCH));
        assert!(!reads_web(names::BASH));
    }
}
