//! Input decoding with model-friendly errors. Every message names the field
//! at fault; scalars are lenient (numbers and booleans sent as strings are
//! accepted) because not every model types JSON arguments precisely.

use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::context::ToolCtx;
use crate::error::ToolError;

/// Typed access to the fields of one tool input object.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fields<'a> {
    map: &'a Map<String, Value>,
}

impl<'a> Fields<'a> {
    pub(crate) fn new(input: &'a Value) -> Result<Self, ToolError> {
        input
            .as_object()
            .map(|map| Self { map })
            .ok_or_else(|| ToolError::invalid("the input must be a JSON object"))
    }

    /// The value of `key`; JSON `null` counts as absent.
    pub(crate) fn value(&self, key: &str) -> Option<&'a Value> {
        self.map.get(key).filter(|value| !value.is_null())
    }

    pub(crate) fn str(&self, key: &str) -> Result<Option<&'a str>, ToolError> {
        match self.value(key) {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text)),
            Some(other) => Err(type_error(key, "a string", other)),
        }
    }

    pub(crate) fn required_str(&self, key: &str) -> Result<&'a str, ToolError> {
        self.str(key)?.ok_or_else(|| missing(key))
    }

    /// A required string that is not blank.
    pub(crate) fn non_empty(&self, key: &str) -> Result<&'a str, ToolError> {
        let text = self.required_str(key)?;
        if text.trim().is_empty() {
            return Err(ToolError::invalid(format!("`{key}` must not be empty")));
        }
        Ok(text)
    }

    /// An optional string; blank values and the placeholders some models
    /// send for "no value" (`undefined`, `null`) count as absent.
    pub(crate) fn optional_text(&self, key: &str) -> Result<Option<&'a str>, ToolError> {
        Ok(self
            .str(key)?
            .filter(|text| !matches!(text.trim(), "" | "undefined" | "null")))
    }

    pub(crate) fn u64(&self, key: &str) -> Result<Option<u64>, ToolError> {
        match self.value(key) {
            None => Ok(None),
            Some(value) => integer(value)
                .map(Some)
                .ok_or_else(|| type_error(key, "a non-negative integer", value)),
        }
    }

    pub(crate) fn usize(&self, key: &str) -> Result<Option<usize>, ToolError> {
        self.u64(key)?
            .map(|value| {
                usize::try_from(value)
                    .map_err(|_| ToolError::invalid(format!("`{key}` is too large")))
            })
            .transpose()
    }

    pub(crate) fn u32(&self, key: &str) -> Result<Option<u32>, ToolError> {
        self.u64(key)?
            .map(|value| {
                u32::try_from(value)
                    .map_err(|_| ToolError::invalid(format!("`{key}` is too large")))
            })
            .transpose()
    }

    pub(crate) fn bool(&self, key: &str) -> Result<Option<bool>, ToolError> {
        match self.value(key) {
            None => Ok(None),
            Some(Value::Bool(flag)) => Ok(Some(*flag)),
            Some(Value::String(text)) => match text.trim().to_ascii_lowercase().as_str() {
                "true" => Ok(Some(true)),
                "false" => Ok(Some(false)),
                _ => Err(type_error(key, "a boolean", &Value::String(text.clone()))),
            },
            Some(other) => Err(type_error(key, "a boolean", other)),
        }
    }

    /// Decodes a structured field; serde's message is prefixed with the key.
    pub(crate) fn parse<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, ToolError> {
        self.value(key)
            .map(|value| {
                T::deserialize(value).map_err(|e| ToolError::invalid(format!("`{key}`: {e}")))
            })
            .transpose()
    }

    pub(crate) fn required<T: DeserializeOwned>(&self, key: &str) -> Result<T, ToolError> {
        self.parse(key)?.ok_or_else(|| missing(key))
    }
}

/// The whole input as one structure (JSON `null` fields count as absent
/// through `#[serde(default)]` on the target).
pub(crate) fn whole<T: DeserializeOwned>(input: &Value) -> Result<T, ToolError> {
    Fields::new(input)?;
    let cleaned: Map<String, Value> = input
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, value)| !value.is_null())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    T::deserialize(Value::Object(cleaned)).map_err(|e| ToolError::invalid(e.to_string()))
}

/// A string field for infallible uses (`title`, `action`).
pub(crate) fn str_field<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(Value::as_str)
}

/// A non-blank path field resolved against the context's root.
pub(crate) fn path_field(input: &Value, key: &str, ctx: &ToolCtx) -> Option<PathBuf> {
    str_field(input, key)
        .filter(|path| !matches!(path.trim(), "" | "undefined" | "null"))
        .map(|path| ctx.resolve(path))
}

fn integer(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64().or_else(|| {
            number
                .as_f64()
                .filter(|float| float.fract() == 0.0 && *float >= 0.0 && *float < 1e15)
                .map(|float| float as u64)
        }),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn missing(key: &str) -> ToolError {
    ToolError::invalid(format!("missing required field `{key}`"))
}

fn type_error(key: &str, expected: &str, found: &Value) -> ToolError {
    let mut shown = found.to_string();
    if shown.len() > 60 {
        let mut end = 60;
        while !shown.is_char_boundary(end) {
            end -= 1;
        }
        shown.truncate(end);
        shown.push_str("...");
    }
    ToolError::invalid(format!("`{key}` must be {expected}, not {shown}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn scalars_are_lenient_and_errors_name_the_field() {
        let input = json!({"a": "12", "b": 3.0, "c": "true", "d": null, "e": [1]});
        let fields = Fields::new(&input).unwrap();
        assert_eq!(fields.usize("a").unwrap(), Some(12));
        assert_eq!(fields.u64("b").unwrap(), Some(3));
        assert_eq!(fields.bool("c").unwrap(), Some(true));
        assert_eq!(fields.str("d").unwrap(), None);
        let err = fields.usize("e").unwrap_err().to_string();
        assert_eq!(
            err,
            "invalid input: `e` must be a non-negative integer, not [1]"
        );
        let err = fields.required_str("missing").unwrap_err().to_string();
        assert!(err.contains("missing required field `missing`"), "{err}");
    }

    #[test]
    fn placeholders_count_as_absent_text() {
        let input = json!({"path": "undefined", "blank": "  ", "real": "src"});
        let fields = Fields::new(&input).unwrap();
        assert_eq!(fields.optional_text("path").unwrap(), None);
        assert_eq!(fields.optional_text("blank").unwrap(), None);
        assert_eq!(fields.optional_text("real").unwrap(), Some("src"));
        assert!(Fields::new(&json!([1])).is_err());
    }
}
