//! Copilot's `GET /models` list -> which API each model is served on.
//!
//! A model goes to the first of Anthropic Messages, Responses and Chat
//! Completions it lists. Models missing from the list (or every model,
//! when the list cannot be fetched) are routed by name instead.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::error::LlmError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    Chat,
    Responses,
    Messages,
}

/// Where one request goes, and the reasoning efforts the model takes
/// (`None`: unknown, the model was routed by name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved {
    pub route: Route,
    pub efforts: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    Disabled,
    Enabled {
        route: Option<Route>,
        efforts: Vec<String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RouteTable {
    models: HashMap<String, Entry>,
}

#[derive(Deserialize)]
struct RemoteModel {
    id: String,
    #[serde(default)]
    supported_endpoints: Vec<String>,
    #[serde(default)]
    policy: Policy,
    #[serde(default)]
    capabilities: Capabilities,
}

#[derive(Default, Deserialize)]
struct Policy {
    state: Option<String>,
}

#[derive(Default, Deserialize)]
struct Capabilities {
    #[serde(default)]
    limits: Limits,
    #[serde(default)]
    supports: Supports,
}

#[derive(Default, Deserialize)]
struct Limits {
    max_output_tokens: Option<u64>,
    max_prompt_tokens: Option<u64>,
}

#[derive(Default, Deserialize)]
struct Supports {
    #[serde(default)]
    reasoning_effort: Vec<String>,
    tool_calls: Option<bool>,
}

impl RouteTable {
    /// Parse a `/models` body. Malformed entries and models unfit for chat
    /// (no output or prompt limits, no tool-call capability) are skipped.
    pub(crate) fn parse(body: &Value) -> Self {
        let mut models = HashMap::new();
        let items = body.get("data").and_then(Value::as_array);
        for raw in items.into_iter().flatten() {
            let Ok(model) = RemoteModel::deserialize(raw) else {
                continue;
            };
            let entry = if model.policy.state.as_deref() == Some("disabled") {
                Entry::Disabled
            } else if usable(&model) {
                Entry::Enabled {
                    route: listed_route(&model.supported_endpoints),
                    efforts: model.capabilities.supports.reasoning_effort,
                }
            } else {
                continue;
            };
            let replace = match models.get(&model.id) {
                None | Some(Entry::Disabled) => true,
                Some(Entry::Enabled { .. }) => false,
            };
            if replace {
                models.insert(model.id, entry);
            }
        }
        Self { models }
    }

    pub(crate) fn resolve(&self, model: &str) -> Result<Resolved, LlmError> {
        match self.models.get(model) {
            Some(Entry::Disabled) => Err(LlmError::Config(format!(
                "GitHub Copilot model {model} is disabled for this account: \
                 enable it in your GitHub Copilot settings"
            ))),
            Some(Entry::Enabled { route, efforts }) => Ok(Resolved {
                route: route.unwrap_or_else(|| route_by_name(model)),
                efforts: Some(efforts.clone()),
            }),
            None => Ok(Resolved {
                route: route_by_name(model),
                efforts: None,
            }),
        }
    }
}

fn usable(model: &RemoteModel) -> bool {
    let capabilities = &model.capabilities;
    capabilities.limits.max_output_tokens.is_some()
        && capabilities.limits.max_prompt_tokens.is_some()
        && capabilities.supports.tool_calls.is_some()
}

fn listed_route(endpoints: &[String]) -> Option<Route> {
    let lists = |path: &str| endpoints.iter().any(|endpoint| endpoint == path);
    if lists("/v1/messages") {
        Some(Route::Messages)
    } else if lists("/responses") {
        Some(Route::Responses)
    } else if lists("/chat/completions") {
        Some(Route::Chat)
    } else {
        None
    }
}

/// GPT-5 class models (`gpt-N`, N >= 5) speak Responses, except the mini
/// variants that still need Chat Completions.
pub(crate) fn route_by_name(model: &str) -> Route {
    let major = model.strip_prefix("gpt-").and_then(|rest| {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse::<u32>().ok()
    });
    match major {
        Some(major) if major >= 5 && !model.starts_with("gpt-5-mini") => Route::Responses,
        _ => Route::Chat,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const MODELS: &str = include_str!("../../tests/fixtures/copilot_models.json");

    fn table() -> RouteTable {
        RouteTable::parse(&serde_json::from_str(MODELS).unwrap())
    }

    fn route(model: &str) -> Route {
        table().resolve(model).unwrap().route
    }

    #[test]
    fn listed_endpoints_pick_the_route() {
        assert_eq!(route("claude-sonnet-4.5"), Route::Messages);
        assert_eq!(route("gpt-5"), Route::Responses);
        assert_eq!(route("gpt-4.1"), Route::Chat);
        assert_eq!(route("gpt-5-mini"), Route::Chat);
        assert_eq!(route("gemini-2.5-pro"), Route::Chat, "no endpoints listed");
        assert_eq!(
            table().resolve("gpt-5").unwrap().efforts,
            Some(vec!["low".into(), "medium".into(), "high".into()])
        );
        assert_eq!(table().resolve("gpt-4.1").unwrap().efforts, Some(vec![]));
    }

    #[test]
    fn disabled_models_fail_clearly() {
        let error = table().resolve("o3-pro").unwrap_err();
        assert!(
            matches!(&error, LlmError::Config(message) if message.contains("enable it in your GitHub Copilot settings")),
            "{error}"
        );
    }

    #[test]
    fn unusable_and_unknown_models_are_routed_by_name() {
        assert_eq!(
            table().resolve("text-embedding-3-small").unwrap(),
            Resolved {
                route: Route::Chat,
                efforts: None
            }
        );
        assert_eq!(route("gpt-6-preview"), Route::Responses);
        assert!(
            RouteTable::parse(&json!({"data": "nope"}))
                .models
                .is_empty()
        );
        assert!(
            RouteTable::parse(&json!({"data": [{"name": "no id"}]}))
                .models
                .is_empty()
        );
    }

    #[test]
    fn an_enabled_duplicate_wins_over_a_disabled_one() {
        let usable = |state: &str| {
            json!({
                "id": "gpt-4o", "policy": {"state": state},
                "supported_endpoints": ["/chat/completions"],
                "capabilities": {
                    "limits": {"max_output_tokens": 1, "max_prompt_tokens": 1},
                    "supports": {"tool_calls": true},
                },
            })
        };
        let table = RouteTable::parse(&json!({"data": [usable("disabled"), usable("enabled")]}));
        assert_eq!(table.resolve("gpt-4o").unwrap().route, Route::Chat);
    }

    #[test]
    fn names_route_gpt5_class_models_to_responses() {
        for (model, expected) in [
            ("gpt-5", Route::Responses),
            ("gpt-5.1-codex", Route::Responses),
            ("gpt-10", Route::Responses),
            ("gpt-5-mini", Route::Chat),
            ("gpt-4.1", Route::Chat),
            ("gpt-4o", Route::Chat),
            ("claude-sonnet-4.5", Route::Chat),
            ("gpt-", Route::Chat),
            ("o3", Route::Chat),
        ] {
            assert_eq!(route_by_name(model), expected, "{model}");
        }
    }
}
