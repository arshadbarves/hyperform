//! The structural check every call passes before hooks and policy: the
//! input is an object carrying every field the schema requires. Tools
//! validate types and values themselves.

use serde_json::Value;

pub(super) fn check(schema: &Value, input: &Value) -> Result<(), String> {
    let Some(object) = input.as_object() else {
        return Err("The tool input must be a JSON object.".to_string());
    };
    let missing: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|key| object.get(*key).is_none_or(Value::is_null))
        .collect();
    match missing.as_slice() {
        [] => Ok(()),
        [one] => Err(format!("Missing required parameter `{one}`.")),
        many => Err(format!(
            "Missing required parameters: {}.",
            many.iter()
                .map(|key| format!("`{key}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn requires_an_object_with_required_fields() {
        let schema = json!({"type": "object", "required": ["file_path", "old_string"]});
        assert!(check(&schema, &json!({"file_path": "a", "old_string": "b"})).is_ok());
        let error = check(&schema, &json!({"file_path": "a", "old_string": null})).unwrap_err();
        assert!(error.contains("`old_string`"));
        assert!(
            check(&schema, &json!({}))
                .unwrap_err()
                .contains("parameters")
        );
        assert!(check(&schema, &json!([1])).is_err());
        assert!(check(&json!({"type": "object"}), &json!({})).is_ok());
    }
}
