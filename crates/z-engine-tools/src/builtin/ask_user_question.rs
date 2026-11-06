//! `AskUserQuestion`: one to four multiple-choice questions for the user.

use std::collections::HashSet;

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;
use z_engine_protocol::Question;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::Fields;
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct AskUserQuestionTool;

fn questions(input: &Value) -> Result<Vec<Question>, ToolError> {
    let questions: Vec<Question> = Fields::new(input)?.required("questions")?;
    if !(1..=4).contains(&questions.len()) {
        return Err(ToolError::invalid(format!(
            "ask 1-4 questions per call, not {}",
            questions.len()
        )));
    }
    let mut texts = HashSet::new();
    for (index, question) in questions.iter().enumerate() {
        let number = index + 1;
        if question.question.trim().is_empty() {
            return Err(ToolError::invalid(format!(
                "question {number}: `question` must not be empty"
            )));
        }
        if !texts.insert(question.question.trim()) {
            return Err(ToolError::invalid(format!(
                "question {number} repeats an earlier question"
            )));
        }
        if !(2..=4).contains(&question.options.len()) {
            return Err(ToolError::invalid(format!(
                "question {number}: give 2-4 options, not {}",
                question.options.len()
            )));
        }
        let mut labels = HashSet::new();
        for option in &question.options {
            if option.label.trim().is_empty() || !labels.insert(option.label.trim()) {
                return Err(ToolError::invalid(format!(
                    "question {number}: option labels must be non-empty and distinct"
                )));
            }
        }
    }
    Ok(questions)
}

#[async_trait]
impl Tool for AskUserQuestionTool {
    fn name(&self) -> &str {
        names::ASK_USER_QUESTION
    }

    fn description(&self) -> String {
        prompts::ASK_USER_QUESTION.to_string()
    }

    fn input_schema(&self) -> Value {
        let option = schema::object(
            json!({
                "label": {"type": "string", "description": "The choice as shown to the user (1-5 words)."},
                "description": {"type": "string", "description": "What the choice means or its trade-off."}
            }),
            &["label", "description"],
        );
        let question = schema::object(
            json!({
                "question": {"type": "string", "description": "The complete question, ending with a question mark."},
                "header": {"type": "string", "description": "A short chip label of at most about 12 characters."},
                "options": {"type": "array", "minItems": 2, "maxItems": 4, "items": option},
                "multiSelect": {"type": "boolean", "description": "Allow several options to be chosen."}
            }),
            &["question", "header", "options", "multiSelect"],
        );
        schema::object(
            json!({
                "questions": {"type": "array", "minItems": 1, "maxItems": 4, "items": question, "description": "1-4 related questions."}
            }),
            &["questions"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Other { read_only: true }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        let first = input.pointer("/questions/0");
        let label = first
            .and_then(|q| q.get("header"))
            .and_then(Value::as_str)
            .filter(|header| !header.trim().is_empty())
            .or_else(|| {
                first
                    .and_then(|q| q.get("question"))
                    .and_then(Value::as_str)
            });
        match label {
            Some(label) => format!("Ask: {label}"),
            None => "Ask the user".to_string(),
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let questions = questions(&input)?;
        let interaction = ctx.ports.interaction()?;
        if !interaction.can_ask(ctx) {
            return Err(ToolError::unavailable(
                "AskUserQuestion is not available here: subagents cannot ask the user. Make the most reasonable choice, state it as an assumption, and continue.",
            ));
        }
        if let Some(note) = ctx
            .until_cancelled(interaction.already_answered(ctx, &questions))
            .await?
        {
            return Ok(ToolOutput::text(note, "Answered earlier in this chat"));
        }
        let asked = questions.len();
        let answers = ctx
            .until_cancelled(interaction.ask(ctx, questions))
            .await?
            .map_err(ToolError::failed)?;
        let Some(answers) = answers else {
            return Ok(ToolOutput::text(
                "The user dismissed the questions without answering. Do not ask them again; proceed on your best judgment and state your assumptions, or explain what is blocked.",
                "Questions dismissed",
            ));
        };
        let mut text = "The user answered your questions:".to_string();
        for answer in &answers {
            let chosen = if answer.answers.is_empty() {
                "(no answer)".to_string()
            } else {
                answer.answers.join(", ")
            };
            text.push_str(&format!("\n- \"{}\" -> {chosen}", answer.question));
        }
        text.push_str("\nContinue with these answers in mind.");
        Ok(ToolOutput::text(
            text,
            format!("Answered {} of {asked} questions", answers.len()),
        ))
    }
}
