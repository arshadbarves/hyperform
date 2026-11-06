//! Context occupancy: the provider-reported prompt size of the last request
//! plus local estimates of the messages added since. Without a report (a
//! new or just-compacted working set) the whole request is estimated.

use z_engine_context::estimate_messages;
use z_engine_protocol::Message;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ContextMeter {
    reported: Option<Reported>,
    /// Estimated system prompt and tool definitions of the latest request.
    overhead: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reported {
    prompt_tokens: u64,
    /// Working-set length the report covers.
    messages: usize,
}

impl ContextMeter {
    pub(crate) fn set_overhead(&mut self, overhead: u64) {
        self.overhead = overhead;
    }

    /// The provider saw `prompt_tokens` for a request of `messages` messages.
    pub(crate) fn observe(&mut self, prompt_tokens: u64, messages: usize) {
        if prompt_tokens > 0 {
            self.reported = Some(Reported {
                prompt_tokens,
                messages,
            });
        }
    }

    pub(crate) fn measure(&self, working: &[Message]) -> u64 {
        match self.reported {
            Some(report) if report.messages <= working.len() => {
                report.prompt_tokens + estimate_messages(&working[report.messages..])
            }
            _ => self.overhead + estimate_messages(working),
        }
    }

    /// Messages covered by the last report were rewritten in place
    /// (microcompaction): lower the report by what they shrank.
    pub(crate) fn rebase(&mut self, before: &[Message], after: &[Message]) {
        let Some(report) = &mut self.reported else {
            return;
        };
        let covered = report.messages.min(before.len()).min(after.len());
        let shrunk = estimate_messages(&before[..covered])
            .saturating_sub(estimate_messages(&after[..covered]));
        report.prompt_tokens = report.prompt_tokens.saturating_sub(shrunk);
    }

    /// The working set was replaced (summary compaction, rewind).
    pub(crate) fn reset(&mut self) {
        self.reported = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reported_size_plus_newer_messages() {
        let mut meter = ContextMeter::default();
        meter.set_overhead(50);
        let working = vec![Message::user_text("a".repeat(40))];
        assert_eq!(meter.measure(&working), 60);
        meter.observe(1_000, 1);
        let mut longer = working.clone();
        longer.push(Message::assistant_text("b".repeat(8)));
        assert_eq!(meter.measure(&longer), 1_002);
        let shorter = vec![Message::user_text("a".repeat(4))];
        meter.rebase(&working, &shorter);
        assert_eq!(meter.measure(&shorter), 991);
        meter.reset();
        assert_eq!(meter.measure(&shorter), 51);
    }
}
