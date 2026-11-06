//! What a downloaded checkpoint folder says about itself: token budgets and
//! temperatures from `config.json`'s `laya` section (Laya's
//! `rl_agent_config.json`), and the special tokens from
//! `tokenizer_config.json`. The tokenizer's own ids are authoritative: the
//! multilingual encoder's config names a different `cls_token_id` than the
//! `<bos>` its tokenizer (and so Laya) uses.

use serde::Deserialize;
use serde_json::Value;

use super::decode::Temperatures;

#[derive(Debug, Deserialize)]
struct ConfigFile {
    laya: LayaSection,
}

#[derive(Debug, Deserialize)]
struct LayaSection {
    max_len: usize,
    head_max_len: usize,
    #[serde(default)]
    temperature: Option<Value>,
    #[serde(default)]
    temperature_by_options: Option<Value>,
}

/// The parts of `config.json` the runtime uses.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct CheckpointConfig {
    /// The length the checkpoint was trained with.
    pub max_len: usize,
    pub head_max_len: usize,
    pub temperatures: Temperatures,
}

impl CheckpointConfig {
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let file: ConfigFile = serde_json::from_str(text)
            .map_err(|error| format!("config.json has no usable `laya` section: {error}"))?;
        let laya = file.laya;
        if laya.head_max_len == 0 || laya.head_max_len >= laya.max_len {
            return Err("config.json: head_max_len must be below max_len".into());
        }
        Ok(Self {
            max_len: laya.max_len,
            head_max_len: laya.head_max_len,
            temperatures: Temperatures::from_config(
                laya.temperature.as_ref(),
                laya.temperature_by_options.as_ref(),
            ),
        })
    }
}

/// The special tokens' texts, resolved to ids against the tokenizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SpecialTexts {
    pub cls: String,
    pub sep: String,
    pub mask: String,
    pub pad: String,
}

impl SpecialTexts {
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let config: Value = serde_json::from_str(text)
            .map_err(|error| format!("tokenizer_config.json is not JSON: {error}"))?;
        let token = |name: &str| {
            let value = config.get(name);
            // A token is either its text or an `{"content": text, ...}` object.
            let text = value
                .and_then(Value::as_str)
                .or_else(|| value?.get("content")?.as_str());
            text.map(str::to_string)
                .filter(|text| !text.is_empty())
                .ok_or_else(|| format!("tokenizer_config.json names no {name}"))
        };
        Ok(Self {
            cls: token("cls_token")?,
            sep: token("sep_token")?,
            mask: token("mask_token")?,
            pad: token("pad_token")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_laya_section_and_special_tokens_parse() {
        let config = CheckpointConfig::parse(
            r#"{ "hidden_size": 768, "laya": { "max_len": 1024, "head_max_len": 256,
                 "temperature": [1.0, 1.0, 1.0], "temperature_by_options": {} } }"#,
        )
        .unwrap();
        assert_eq!((config.max_len, config.head_max_len), (1024, 256));
        assert!(CheckpointConfig::parse(r#"{ "hidden_size": 768 }"#).is_err());
        let special = SpecialTexts::parse(
            r#"{ "cls_token": "<bos>", "sep_token": "<eos>", "pad_token": "<pad>",
                 "mask_token": { "content": "<mask>", "lstrip": true } }"#,
        )
        .unwrap();
        assert_eq!(special.cls, "<bos>");
        assert_eq!(special.mask, "<mask>");
        assert!(SpecialTexts::parse(r#"{ "cls_token": "[CLS]" }"#).is_err());
    }
}
