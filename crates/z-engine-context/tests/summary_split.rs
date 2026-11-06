//! Property-style check of summary compaction over generated conversations:
//! the chosen split always matches a brute-force oracle, lands on a real
//! user turn, and never separates a tool_use from its tool_result.

use std::collections::HashSet;

use serde_json::json;
use z_engine_context::{apply_summary, plan_summary, summary_message};
use z_engine_protocol::{CallId, ContentBlock, MediaSource, Message, Role};

/// xorshift64*: deterministic, dependency-free randomness.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

fn user_turn(rng: &mut Rng) -> Message {
    let mut content = vec![ContentBlock::text("please continue")];
    if rng.chance(20) {
        content.push(ContentBlock::Image {
            source: MediaSource::Url {
                url: "https://example.com/screenshot.png".into(),
            },
        });
    }
    Message::new(Role::User, content)
}

fn conversation(rng: &mut Rng) -> Vec<Message> {
    let mut messages = Vec::new();
    let mut next_call = 0;
    for _ in 0..1 + rng.below(5) {
        messages.push(user_turn(rng));
        let rounds = rng.below(4);
        for round in 0..=rounds {
            let calls = if round == rounds { 0 } else { 1 + rng.below(3) };
            let mut assistant = Vec::new();
            if rng.chance(40) {
                assistant.push(ContentBlock::Thinking {
                    text: "plan".into(),
                    signature: Some("sig".into()),
                });
            }
            assistant.push(ContentBlock::text("working"));
            let ids: Vec<CallId> = (0..calls)
                .map(|_| {
                    next_call += 1;
                    CallId::from(format!("call_{next_call}"))
                })
                .collect();
            for id in &ids {
                assistant.push(ContentBlock::ToolUse {
                    id: id.clone(),
                    name: "Bash".into(),
                    input: json!({"command": "cargo test"}),
                });
            }
            messages.push(Message::new(Role::Assistant, assistant));
            if calls == 0 {
                break;
            }
            // Rare anomalies: a user message arriving before the results,
            // or results that never arrive.
            if rng.chance(5) {
                messages.push(user_turn(rng));
            }
            if rng.chance(3) {
                break;
            }
            let mut results: Vec<ContentBlock> = ids
                .iter()
                .map(|id| ContentBlock::tool_result(id.clone(), "ok", rng.chance(10)))
                .collect();
            if rng.chance(30) {
                results.push(ContentBlock::text(
                    "<system-reminder>\nsteering\n</system-reminder>",
                ));
            }
            messages.push(Message::new(Role::User, results));
        }
    }
    messages
}

/// Every (tool_use index, tool_result index) pair.
fn pairs(messages: &[Message]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for (use_index, message) in messages.iter().enumerate() {
        for (id, _, _) in message.tool_uses() {
            for (result_index, other) in messages.iter().enumerate() {
                let answers = other.content.iter().any(|block| {
                    matches!(block, ContentBlock::ToolResult { tool_use_id, .. } if tool_use_id == id)
                });
                if answers {
                    found.push((use_index, result_index));
                }
            }
        }
    }
    found
}

fn has_tool_results(message: &Message) -> bool {
    message
        .content
        .iter()
        .any(|block| matches!(block, ContentBlock::ToolResult { .. }))
}

fn pairs_on_one_side(messages: &[Message], split: usize) -> bool {
    pairs(messages)
        .iter()
        .all(|&(a, b)| !(a.min(b) < split && split <= a.max(b)))
}

/// Brute-force definition of an acceptable split: at least two summarized
/// messages, the tail starts at an assistant message or a real user turn,
/// and every tool pair sits on one side.
fn acceptable(messages: &[Message], split: usize) -> bool {
    let message = &messages[split];
    let starts_tail = match message.role {
        Role::Assistant => true,
        Role::User => !message.content.is_empty() && !has_tool_results(message),
    };
    split >= 2 && starts_tail && pairs_on_one_side(messages, split)
}

/// Latest acceptable split keeping `keep` messages, else latest overall.
fn oracle(messages: &[Message], keep: usize) -> Option<usize> {
    let acceptable: Vec<usize> = (0..messages.len())
        .rev()
        .filter(|&split| acceptable(messages, split))
        .collect();
    acceptable
        .iter()
        .copied()
        .find(|&split| messages.len() - split >= keep)
        .or_else(|| acceptable.first().copied())
}

fn assert_tail_is_self_contained(tail: &[Message]) {
    let uses: HashSet<&CallId> = tail
        .iter()
        .flat_map(|message| message.tool_uses().map(|(id, _, _)| id))
        .collect();
    for message in tail {
        for block in &message.content {
            if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                assert!(uses.contains(tool_use_id), "orphaned result {tool_use_id}");
            }
        }
    }
}

#[test]
fn splits_match_the_oracle_and_never_break_tool_pairs() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut planned = 0;
    for _ in 0..400 {
        let messages = conversation(&mut rng);
        for keep in 0..=messages.len() + 1 {
            let expected = oracle(&messages, keep);
            let plan = plan_summary(&messages, keep);
            assert_eq!(
                plan.map(|plan| plan.split),
                expected,
                "keep {keep}: {messages:#?}"
            );
            let Some(plan) = plan else {
                continue;
            };
            planned += 1;
            let summary = summary_message("Earlier work summarized.");
            let compacted = apply_summary(&messages, &plan, &summary);
            assert_eq!(compacted[0], summary);
            assert_eq!(compacted[1..], messages[plan.split..]);
            assert!(plan.split >= 2);
            assert!(!has_tool_results(&compacted[1]));
            assert!(pairs_on_one_side(&messages, plan.split));
            assert_tail_is_self_contained(&compacted[1..]);
        }
    }
    assert!(planned > 1_000, "generator too narrow: {planned} plans");
}

/// One user prompt followed by 30 tool rounds and a final answer.
fn long_agentic_turn() -> Vec<Message> {
    let mut messages = vec![Message::user_text("refactor the parser")];
    for round in 0..30 {
        let ids = [
            CallId::from(format!("read_{round}")),
            CallId::from(format!("grep_{round}")),
        ];
        messages.push(Message::new(
            Role::Assistant,
            ids.iter()
                .map(|id| ContentBlock::ToolUse {
                    id: id.clone(),
                    name: "Read".into(),
                    input: json!({"file_path": "src/parser.rs"}),
                })
                .collect(),
        ));
        let mut results: Vec<ContentBlock> = ids
            .iter()
            .map(|id| ContentBlock::tool_result(id.clone(), "fn parse() {}", false))
            .collect();
        if round % 3 == 0 {
            results.push(ContentBlock::text(
                "<system-reminder>\nsteering\n</system-reminder>",
            ));
        }
        messages.push(Message::new(Role::User, results));
    }
    messages.push(Message::assistant_text("done"));
    messages
}

#[test]
fn a_single_long_agentic_turn_is_compacted_without_breaking_pairs() {
    let messages = long_agentic_turn();
    assert_eq!(messages.len(), 62);
    for keep in [0, 1, 4, 10, 20, 61, 62, 100] {
        let plan = plan_summary(&messages, keep).unwrap_or_else(|| panic!("keep {keep}: None"));
        assert_eq!(Some(plan.split), oracle(&messages, keep), "keep {keep}");
        assert!(plan.split >= 2, "keep {keep}");
        assert_eq!(messages[plan.split].role, Role::Assistant, "keep {keep}");
        if keep <= 59 {
            assert!(messages.len() - plan.split >= keep, "keep {keep}");
        }
        assert!(pairs_on_one_side(&messages, plan.split), "keep {keep}");
        let compacted = apply_summary(&messages, &plan, &summary_message("Read the parser."));
        assert_eq!(compacted[0].role, Role::User);
        assert_tail_is_self_contained(&compacted[1..]);
        for pair in compacted.windows(2) {
            assert_ne!(pair[0].role, pair[1].role, "roles must alternate");
        }
    }
    assert_eq!(plan_summary(&messages, 10).map(|plan| plan.split), Some(51));
}
