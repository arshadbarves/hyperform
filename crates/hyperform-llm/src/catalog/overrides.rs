//! Local model overrides in the `~/.config/hyperform/models.json` shape:
//! `{"<provider>": {"name": "...", "models": {"<id>": {"name", "reasoning",
//! "attachment", "context", "output"}}}}`. models.dev field names such as
//! `tool_call`, `limit` and `cost` are accepted too, and so is the
//! `{"providers": {...}}` wrapper the v1 documentation described.

use serde_json::Value;

use super::models_dev::{apply_fields, unknown_model};
use super::types::ModelCatalog;
use crate::error::LlmError;

impl ModelCatalog {
    /// Merge overrides field by field into the matching `(provider, id)`
    /// entries. Models the catalog does not know are added in front, so
    /// they win lookups, and are assumed to support tools (as v1 did).
    pub fn merge_overrides(&mut self, json: &str) -> Result<(), LlmError> {
        let root: Value = serde_json::from_str(json)
            .map_err(|error| LlmError::Decode(format!("model overrides: {error}")))?;
        let providers = root
            .get("providers")
            .filter(|providers| providers.is_object())
            .unwrap_or(&root)
            .as_object()
            .ok_or_else(|| LlmError::Decode("model overrides must be a JSON object".into()))?;
        let mut added = Vec::new();
        for (provider_id, provider) in providers {
            let Some(models) = provider.get("models").and_then(Value::as_object) else {
                continue;
            };
            for (model_id, entry) in models {
                if !entry.is_object() {
                    tracing::warn!(provider = %provider_id, model = %model_id, "ignoring non-object model override");
                    continue;
                }
                let existing = self
                    .models
                    .iter_mut()
                    .find(|model| model.provider == *provider_id && model.id == *model_id);
                match existing {
                    Some(model) => apply_fields(model, entry),
                    None => {
                        let mut model = unknown_model(provider_id, model_id, true);
                        apply_fields(&mut model, entry);
                        added.push(model);
                    }
                }
            }
        }
        added.append(&mut self.models);
        self.models = added;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Pricing;

    fn catalog() -> ModelCatalog {
        ModelCatalog::from_models_dev_json(include_str!("../../tests/fixtures/models_dev.json"))
            .unwrap()
    }

    #[test]
    fn v1_overrides_update_fields_individually_and_add_models() {
        let mut catalog = catalog();
        let before = catalog.models.len();
        catalog
            .merge_overrides(
                r#"{"openrouter": {"name": "OpenRouter", "models": {
                    "anthropic/claude-sonnet-4.5": {"context": 500000},
                    "acme/private": {"name": "Private", "reasoning": true, "attachment": true, "context": 32000, "output": 4096}
                }}}"#,
            )
            .unwrap();
        assert_eq!(catalog.models.len(), before + 1);
        let sonnet = catalog.lookup("anthropic/claude-sonnet-4.5").unwrap();
        assert_eq!(sonnet.context_window, 500_000);
        assert_eq!(sonnet.max_output, 64_000);
        assert_eq!(sonnet.name, "Claude Sonnet 4.5");
        assert!(sonnet.pricing.is_some());
        let private = &catalog.models[0];
        assert_eq!(
            (private.id.as_str(), private.provider.as_str()),
            ("acme/private", "openrouter")
        );
        assert_eq!(
            (private.context_window, private.max_output),
            (32_000, 4_096)
        );
        assert!(private.tools && private.vision && private.reasoning);
        assert_eq!(catalog.lookup("private").unwrap().name, "Private");
    }

    #[test]
    fn wrapped_overrides_with_models_dev_fields_are_accepted() {
        let mut catalog = catalog();
        catalog
            .merge_overrides(
                r#"{"providers": {"opencode": {"models": {"big-pickle": {
                    "tool_call": false, "limit": {"output": 1000}, "status": "deprecated",
                    "cost": {"input": 1, "output": 2, "cache_read": 0.1}
                }}}}}"#,
            )
            .unwrap();
        let pickle = catalog.lookup("big-pickle").unwrap();
        assert!(!pickle.tools && pickle.deprecated);
        assert_eq!((pickle.context_window, pickle.max_output), (200_000, 1_000));
        assert_eq!(
            pickle.pricing,
            Some(Pricing {
                input: 1.0,
                output: 2.0,
                cache_read: 0.1,
                cache_write: 0.0
            })
        );
    }

    #[test]
    fn malformed_overrides_are_errors_and_leave_the_catalog_alone() {
        let mut catalog = catalog();
        let before = catalog.clone();
        for bad in ["{", "[]", "\"text\""] {
            assert!(
                matches!(catalog.merge_overrides(bad), Err(LlmError::Decode(_))),
                "{bad}"
            );
        }
        catalog
            .merge_overrides(r#"{"x": {"models": {"m": 5}}, "y": 3}"#)
            .unwrap();
        assert_eq!(catalog, before);
    }
}
