//! [`ModelRequest`] -> Anthropic Messages API request body.

use serde_json::{Value, json};
use z_engine_protocol::Effort;

use super::messages;
use crate::types::{ModelRequest, SystemBlock, ThinkingConfig, ToolChoice};
use crate::wire::{ephemeral, float};

/// Anthropic rejects requests with more `cache_control` blocks than this.
pub(crate) const MAX_CACHE_BREAKPOINTS: usize = 4;
const MIN_THINKING_BUDGET: u32 = 1_024;
/// Output tokens always left for the answer after the thinking budget.
const ANSWER_HEADROOM: u32 = 1_024;
const RAISED_ANSWER_ROOM: u32 = 4_096;

/// Extended thinking is sent unless the tool choice forces a tool call,
/// which the API rejects in combination with thinking.
pub(crate) fn thinking_enabled(request: &ModelRequest) -> bool {
    let forced = !request.tools.is_empty()
        && matches!(request.tool_choice, ToolChoice::Any | ToolChoice::Tool(_));
    request.thinking.is_some() && !forced
}

pub(crate) fn build_body(request: &ModelRequest, cache_control: bool) -> Value {
    let thinking = request.thinking.filter(|_| thinking_enabled(request));
    let system: Vec<&SystemBlock> = request
        .system
        .iter()
        .filter(|block| !block.text.trim().is_empty())
        .collect();
    let mut system_marks: Vec<bool> = system
        .iter()
        .map(|block| cache_control && block.cache)
        .collect();
    let tools_mark = cache_control && request.cache_tools && !request.tools.is_empty();
    let message_budget = fit_fixed_breakpoints(&mut system_marks, tools_mark);
    let mut body = json!({
        "model": request.model,
        "stream": true,
        "messages": messages::build(request, message_budget, thinking.is_some()),
    });
    if !system.is_empty() {
        body["system"] = system
            .iter()
            .zip(&system_marks)
            .map(|(block, mark)| cacheable(json!({"type": "text", "text": block.text}), *mark))
            .collect();
    }
    if let Some(last) = request.tools.len().checked_sub(1) {
        body["tools"] = request
            .tools
            .iter()
            .enumerate()
            .map(|(index, tool)| {
                let definition = json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.input_schema,
                });
                cacheable(definition, tools_mark && index == last)
            })
            .collect();
        body["tool_choice"] = tool_choice(&request.tool_choice);
    }
    let mut max_tokens = request.max_tokens;
    match thinking {
        Some(config) => {
            let (budget, adjusted) = thinking_budget(config, max_tokens);
            max_tokens = adjusted;
            body["thinking"] = json!({"type": "enabled", "budget_tokens": budget});
        }
        None => {
            if let Some(temperature) = request.temperature {
                body["temperature"] = float(temperature);
            }
        }
    }
    body["max_tokens"] = json!(max_tokens);
    if !request.stop_sequences.is_empty() {
        body["stop_sequences"] = json!(request.stop_sequences);
    }
    body
}

/// `(budget_tokens, max_tokens)`: the budget is the explicit or effort-based
/// value, at least [`MIN_THINKING_BUDGET`] and leaving [`ANSWER_HEADROOM`];
/// when `max_tokens` cannot fit the minimum, it is raised instead.
pub(crate) fn thinking_budget(config: ThinkingConfig, max_tokens: u32) -> (u32, u32) {
    let desired = config
        .budget_tokens
        .unwrap_or_else(|| effort_budget(config.effort));
    let budget = desired
        .min(max_tokens.saturating_sub(ANSWER_HEADROOM))
        .max(MIN_THINKING_BUDGET);
    if budget.saturating_add(ANSWER_HEADROOM) > max_tokens {
        (budget, budget.saturating_add(RAISED_ANSWER_ROOM))
    } else {
        (budget, max_tokens)
    }
}

fn effort_budget(effort: Effort) -> u32 {
    match effort {
        Effort::Low => 2_048,
        Effort::Medium => 8_192,
        Effort::High => 16_384,
        Effort::Max => 32_000,
    }
}

/// Keep system and tool breakpoints within the cap by dropping the earliest
/// system ones; returns how many message breakpoints remain allowed.
fn fit_fixed_breakpoints(system: &mut [bool], tools: bool) -> usize {
    let mut used = system.iter().filter(|mark| **mark).count() + usize::from(tools);
    for mark in system.iter_mut() {
        if used <= MAX_CACHE_BREAKPOINTS {
            break;
        }
        if *mark {
            *mark = false;
            used -= 1;
        }
    }
    MAX_CACHE_BREAKPOINTS.saturating_sub(used)
}

fn cacheable(mut block: Value, mark: bool) -> Value {
    if mark {
        block["cache_control"] = ephemeral();
    }
    block
}

fn tool_choice(choice: &ToolChoice) -> Value {
    match choice {
        ToolChoice::Auto => json!({"type": "auto"}),
        ToolChoice::None => json!({"type": "none"}),
        ToolChoice::Any => json!({"type": "any"}),
        ToolChoice::Tool(name) => json!({"type": "tool", "name": name}),
    }
}

#[cfg(test)]
mod tests;
