//! Typed questions and the request that batches every question about one
//! state. Question wording comes from `z_engine_prompts::decisions`.

use std::collections::HashSet;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::DecisionError;

/// Laya accepts up to 255 options but degrades above 20; prefer two steps.
pub const MAX_OPTIONS: usize = 255;

/// One option of a choice, or one level of a score.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Criterion {
    pub key: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Form {
    /// One of several options, answered by key.
    Choice(Vec<Criterion>),
    /// Sent as a two-option choice with neutral keys: Laya's `noul`
    /// answers can follow the option labels instead of the input.
    YesNo { yes: String, no: String },
    /// A level from an ordered list, lowest first; Laya's weakest type.
    Score(Vec<Criterion>),
}

/// `name` is unique within a request and names the question's calibration
/// (`[decisions.calibration.<name>]`) and its trace entries.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Question {
    pub name: String,
    pub instructions: String,
    pub form: Form,
}

impl Question {
    /// A choice from a template whose `- key: description` lines are the options.
    pub fn choice(name: &str, template: &str) -> Result<Self, DecisionError> {
        let (instructions, options) = parse_template(name, template)?;
        Ok(Self::new(name, instructions, Form::Choice(options)))
    }

    /// A yes/no question from a template with `- yes:` and `- no:` lines.
    pub fn yes_no(name: &str, template: &str) -> Result<Self, DecisionError> {
        let (instructions, options) = parse_template(name, template)?;
        let find = |key: &str| {
            let option = options.iter().find(|option| option.key == key);
            option
                .map(|option| option.description.clone())
                .ok_or_else(|| {
                    DecisionError::InvalidQuestion(format!("{name}: missing the `- {key}:` line"))
                })
        };
        let form = Form::YesNo {
            yes: find("yes")?,
            no: find("no")?,
        };
        Ok(Self::new(name, instructions, form))
    }

    /// A score whose `- key: description` lines are the levels, lowest first.
    pub fn score(name: &str, template: &str) -> Result<Self, DecisionError> {
        let (instructions, levels) = parse_template(name, template)?;
        Ok(Self::new(name, instructions, Form::Score(levels)))
    }

    fn new(name: &str, instructions: String, form: Form) -> Self {
        Self {
            name: name.to_string(),
            instructions,
            form,
        }
    }
}

/// Instructions are the text before the first option line; every option
/// line is `- key: description`. At least two options are required.
fn parse_template(name: &str, template: &str) -> Result<(String, Vec<Criterion>), DecisionError> {
    let invalid = |reason: &str| DecisionError::InvalidQuestion(format!("{name}: {reason}"));
    let mut instructions = Vec::new();
    let mut options = Vec::new();
    for line in template.lines() {
        let Some(option) = line.trim_start().strip_prefix("- ") else {
            if options.is_empty() {
                instructions.push(line);
            }
            continue;
        };
        let (key, description) = option
            .split_once(':')
            .ok_or_else(|| invalid("an option line needs `- key: description`"))?;
        let criterion = Criterion {
            key: key.trim().to_string(),
            description: description.trim().to_string(),
        };
        if criterion.key.is_empty() || criterion.description.is_empty() {
            return Err(invalid("an option has an empty key or description"));
        }
        options.push(criterion);
    }
    let instructions = instructions.join("\n").trim().to_string();
    if instructions.is_empty() {
        return Err(invalid("the instructions are empty"));
    }
    if !(2..=MAX_OPTIONS).contains(&options.len()) {
        return Err(invalid("a question needs 2 to 255 options"));
    }
    Ok((instructions, options))
}

/// Everything asked about one state. The state is JSON the model reads;
/// it may hold source text, so it is never written to traces.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionRequest {
    pub state: Value,
    pub questions: Vec<Question>,
}

impl DecisionRequest {
    pub fn new(state: Value) -> Self {
        Self {
            state,
            questions: Vec::new(),
        }
    }

    pub fn ask(mut self, question: Question) -> Self {
        self.questions.push(question);
        self
    }

    /// At least one question, unique names, unique option keys.
    pub fn validate(&self) -> Result<(), DecisionError> {
        if self.questions.is_empty() {
            return Err(DecisionError::InvalidQuestion("no questions".into()));
        }
        let mut names = HashSet::new();
        for question in &self.questions {
            if !names.insert(question.name.as_str()) {
                let name = &question.name;
                return Err(DecisionError::InvalidQuestion(format!(
                    "{name} is asked twice"
                )));
            }
            let keys: Vec<&str> = match &question.form {
                Form::Choice(options) | Form::Score(options) => {
                    options.iter().map(|option| option.key.as_str()).collect()
                }
                Form::YesNo { .. } => continue,
            };
            if keys.iter().collect::<HashSet<_>>().len() != keys.len() {
                let name = &question.name;
                return Err(DecisionError::InvalidQuestion(format!(
                    "{name} repeats an option"
                )));
            }
        }
        Ok(())
    }

    /// A short digest of the state: traces identify inputs without storing them.
    pub fn fingerprint(&self) -> String {
        let digest = Sha256::digest(self.state.to_string().as_bytes());
        digest
            .iter()
            .take(8)
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const YES_NO: &str = "Is it relevant?\n\n- yes: It is needed.\n- no: It is not.\n";

    #[test]
    fn templates_parse_into_forms() {
        let question = Question::yes_no("relevant", YES_NO).unwrap();
        assert_eq!(question.instructions, "Is it relevant?");
        assert_eq!(
            question.form,
            Form::YesNo {
                yes: "It is needed.".into(),
                no: "It is not.".into()
            }
        );
        let choice = Question::choice("tone", "Tone?\n- calm: Calm.\n- tense: Tense.\n").unwrap();
        assert!(matches!(choice.form, Form::Choice(ref options) if options[1].key == "tense"));
        assert!(Question::score("size", "Size?\n- small: S\n- large: L\n").is_ok());
    }

    #[test]
    fn broken_templates_are_rejected() {
        assert!(Question::yes_no("q", "Only one?\n- yes: y\n").is_err());
        assert!(Question::choice("q", "- a: A\n- b: B\n").is_err());
        assert!(Question::choice("q", "Q?\n- a A\n- b: B\n").is_err());
        assert!(Question::yes_no("q", "Q?\n- a: A\n- b: B\n").is_err());
    }

    #[test]
    fn every_shipped_question_parses() {
        for (name, template) in z_engine_prompts::decisions::ALL {
            let parsed =
                Question::yes_no(name, template).or_else(|_| Question::choice(name, template));
            assert!(parsed.is_ok(), "{name}: {parsed:?}");
        }
    }

    #[test]
    fn requests_need_unique_names_and_fingerprint_their_state() {
        let question = Question::yes_no("relevant", YES_NO).unwrap();
        let request = DecisionRequest::new(json!({ "text": "hi" })).ask(question.clone());
        assert!(request.validate().is_ok());
        assert_eq!(request.fingerprint().len(), 16);
        assert!(request.clone().ask(question).validate().is_err());
        assert!(DecisionRequest::new(json!({})).validate().is_err());
        let other = DecisionRequest::new(json!({ "text": "bye" }));
        assert_ne!(request.fingerprint(), other.fingerprint());
    }
}
