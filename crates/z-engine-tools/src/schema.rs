//! JSON Schema helpers for tool inputs.

use serde_json::{Value, json};

/// An object schema with `properties`, `required`, and no extra fields.
pub(crate) fn object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}
