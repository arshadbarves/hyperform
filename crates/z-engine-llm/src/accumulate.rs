//! Assembles streaming events into content blocks.

use std::collections::HashMap;

use serde_json::Value;
use z_engine_protocol::{CallId, ContentBlock, Usage};

use crate::types::{ModelEvent, StopReason};

/// A tool call whose JSON input did not parse into an object. It is still
/// present in `content` (with `{}` input) so a paired error result can be
/// returned to the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedToolUse {
    pub id: String,
    pub name: String,
    pub raw: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssistantTurn {
    pub content: Vec<ContentBlock>,
    pub usage: Usage,
    pub stop: Option<StopReason>,
    pub malformed: Vec<MalformedToolUse>,
}

impl AssistantTurn {
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn has_tool_use(&self) -> bool {
        self.content
            .iter()
            .any(|block| matches!(block, ContentBlock::ToolUse { .. }))
    }
}

#[derive(Debug)]
enum Building {
    Text(String),
    Thinking {
        text: String,
        signature: Option<String>,
    },
    Redacted(String),
    Tool {
        id: String,
        name: String,
        json: String,
    },
}

#[derive(Debug, Default)]
pub struct ResponseAccumulator {
    blocks: Vec<Building>,
    tools: HashMap<usize, usize>,
    usage: Usage,
    stop: Option<StopReason>,
}

impl ResponseAccumulator {
    pub fn absorb(&mut self, event: &ModelEvent) {
        match event {
            ModelEvent::TextDelta(delta) => match self.blocks.last_mut() {
                Some(Building::Text(text)) => text.push_str(delta),
                _ => self.blocks.push(Building::Text(delta.clone())),
            },
            ModelEvent::ThinkingDelta(delta) => match self.blocks.last_mut() {
                Some(Building::Thinking {
                    text,
                    signature: None,
                }) => text.push_str(delta),
                _ => self.blocks.push(Building::Thinking {
                    text: delta.clone(),
                    signature: None,
                }),
            },
            ModelEvent::ThinkingSignature(sig) => match self.blocks.last_mut() {
                Some(Building::Thinking { signature, .. }) => *signature = Some(sig.clone()),
                _ => self.blocks.push(Building::Thinking {
                    text: String::new(),
                    signature: Some(sig.clone()),
                }),
            },
            ModelEvent::RedactedThinking(data) => {
                self.blocks.push(Building::Redacted(data.clone()))
            }
            ModelEvent::ToolUseStart { index, id, name } => {
                self.tools.insert(*index, self.blocks.len());
                self.blocks.push(Building::Tool {
                    id: id.clone(),
                    name: name.clone(),
                    json: String::new(),
                });
            }
            ModelEvent::ToolUseDelta {
                index,
                partial_json,
            } => {
                let slot = match self.tools.get(index) {
                    Some(slot) => *slot,
                    None => {
                        self.tools.insert(*index, self.blocks.len());
                        self.blocks.push(Building::Tool {
                            id: CallId::new().0,
                            name: String::new(),
                            json: String::new(),
                        });
                        self.blocks.len() - 1
                    }
                };
                if let Some(Building::Tool { json, .. }) = self.blocks.get_mut(slot) {
                    json.push_str(partial_json);
                }
            }
            ModelEvent::ToolUseEnd { .. } | ModelEvent::Retrying { .. } => {}
            ModelEvent::Usage(usage) => self.usage = *usage,
            ModelEvent::Stop(reason) => self.stop = Some(reason.clone()),
        }
    }

    /// Text streamed so far (for partial display after an interruption).
    pub fn text_so_far(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                Building::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    pub fn finish(self) -> AssistantTurn {
        let mut content = Vec::new();
        let mut malformed = Vec::new();
        for block in self.blocks {
            match block {
                Building::Text(text) => {
                    if !text.is_empty() {
                        content.push(ContentBlock::Text { text });
                    }
                }
                Building::Thinking { text, signature } => {
                    if !text.is_empty() || signature.is_some() {
                        content.push(ContentBlock::Thinking { text, signature });
                    }
                }
                Building::Redacted(data) => content.push(ContentBlock::RedactedThinking { data }),
                Building::Tool { id, name, json } => {
                    let input = match parse_input(&json) {
                        Ok(input) => input,
                        Err(error) => {
                            malformed.push(MalformedToolUse {
                                id: id.clone(),
                                name: name.clone(),
                                raw: json,
                                error,
                            });
                            Value::Object(Default::default())
                        }
                    };
                    content.push(ContentBlock::ToolUse {
                        id: CallId(id),
                        name,
                        input,
                    });
                }
            }
        }
        AssistantTurn {
            content,
            usage: self.usage,
            stop: self.stop,
            malformed,
        }
    }
}

fn parse_input(json: &str) -> Result<Value, String> {
    if json.trim().is_empty() {
        return Ok(Value::Object(Default::default()));
    }
    match serde_json::from_str::<Value>(json) {
        Ok(value @ Value::Object(_)) => Ok(value),
        Ok(other) => Err(format!("expected a JSON object, got {other}")),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(events: &[ModelEvent]) -> AssistantTurn {
        let mut acc = ResponseAccumulator::default();
        for event in events {
            acc.absorb(event);
        }
        acc.finish()
    }

    #[test]
    fn assembles_text_thinking_and_tools_in_order() {
        let turn = run(&[
            ModelEvent::ThinkingDelta("hm".into()),
            ModelEvent::ThinkingSignature("sig".into()),
            ModelEvent::TextDelta("Hel".into()),
            ModelEvent::TextDelta("lo".into()),
            ModelEvent::ToolUseStart {
                index: 0,
                id: "c1".into(),
                name: "Read".into(),
            },
            ModelEvent::ToolUseDelta {
                index: 0,
                partial_json: "{\"file_path\":".into(),
            },
            ModelEvent::ToolUseDelta {
                index: 0,
                partial_json: "\"a.rs\"}".into(),
            },
            ModelEvent::ToolUseEnd { index: 0 },
            ModelEvent::Stop(StopReason::ToolUse),
        ]);
        assert_eq!(turn.content.len(), 3);
        assert!(matches!(
            &turn.content[0],
            ContentBlock::Thinking { signature: Some(s), .. } if s == "sig"
        ));
        assert_eq!(turn.text(), "Hello");
        assert!(turn.has_tool_use());
        assert!(turn.malformed.is_empty());
        assert_eq!(turn.stop, Some(StopReason::ToolUse));
    }

    #[test]
    fn malformed_and_empty_tool_input() {
        let turn = run(&[
            ModelEvent::ToolUseStart {
                index: 0,
                id: "a".into(),
                name: "Bash".into(),
            },
            ModelEvent::ToolUseDelta {
                index: 0,
                partial_json: "{bad".into(),
            },
            ModelEvent::ToolUseStart {
                index: 1,
                id: "b".into(),
                name: "TodoWrite".into(),
            },
        ]);
        assert_eq!(turn.malformed.len(), 1);
        assert_eq!(turn.malformed[0].id, "a");
        let inputs: Vec<_> = turn
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolUse { input, .. } => Some(input.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(inputs, vec![serde_json::json!({}), serde_json::json!({})]);
    }
}
