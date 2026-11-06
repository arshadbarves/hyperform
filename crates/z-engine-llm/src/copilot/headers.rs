//! Copilot's request headers: the static set every request carries, and
//! the per-request ones derived from the conversation.

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use z_engine_protocol::{ContentBlock, ToolResultPart};

use super::identity::API_VERSION;
use super::models::Route;
use crate::anthropic::beta_headers;
use crate::error::LlmError;
use crate::opencode;
use crate::provider::{apply_extra_headers, secret};
use crate::turns::is_user_input;
use crate::types::ModelRequest;

/// `token` is the GitHub token from the device sign-in.
pub(super) fn static_headers(
    token: &str,
    extra: &[(String, String)],
) -> Result<HeaderMap, LlmError> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(AUTHORIZATION, secret(&format!("Bearer {token}"))?);
    headers.insert(USER_AGENT, HeaderValue::from_static(opencode::USER_AGENT));
    headers.insert(
        "x-github-api-version",
        HeaderValue::from_static(API_VERSION),
    );
    headers.insert(
        "openai-intent",
        HeaderValue::from_static("conversation-edits"),
    );
    apply_extra_headers(&mut headers, extra)?;
    Ok(headers)
}

/// `x-initiator: user` spends one of the user's premium requests, so only
/// a request answering user input is marked as theirs; tool rounds are the
/// agent's.
pub(super) fn request_headers(request: &ModelRequest, route: Route) -> HeaderMap {
    let mut headers = match route {
        Route::Messages => beta_headers(request),
        Route::Chat | Route::Responses => HeaderMap::new(),
    };
    let initiator = if request.messages.last().is_some_and(is_user_input) {
        "user"
    } else {
        "agent"
    };
    headers.insert("x-initiator", HeaderValue::from_static(initiator));
    if has_images(request) {
        headers.insert("copilot-vision-request", HeaderValue::from_static("true"));
    }
    let interaction = request
        .session_key
        .as_deref()
        .and_then(|key| HeaderValue::from_str(key).ok());
    if let Some(interaction) = interaction {
        headers.insert("x-interaction-id", interaction);
    }
    headers
}

fn has_images(request: &ModelRequest) -> bool {
    request
        .messages
        .iter()
        .flat_map(|message| &message.content)
        .any(|block| match block {
            ContentBlock::Image { .. } => true,
            ContentBlock::ToolResult { content, .. } => content
                .iter()
                .any(|part| matches!(part, ToolResultPart::Image { .. })),
            _ => false,
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use z_engine_protocol::{CallId, Effort, MediaSource, Message, Role};

    use super::*;
    use crate::types::{ThinkingConfig, ToolSpec};

    fn png() -> MediaSource {
        MediaSource::Base64 {
            media_type: "image/png".into(),
            data: "AAAA".into(),
        }
    }

    fn request(messages: Vec<Message>) -> ModelRequest {
        ModelRequest::new("gpt-5", messages)
    }

    fn initiator(messages: Vec<Message>) -> String {
        let headers = request_headers(&request(messages), Route::Chat);
        headers["x-initiator"].to_str().unwrap().to_string()
    }

    fn result(blocks: Vec<ContentBlock>) -> Message {
        let mut content = vec![ContentBlock::tool_result(CallId::from("c1"), "ok", false)];
        content.extend(blocks);
        Message::new(Role::User, content)
    }

    #[test]
    fn static_headers_carry_the_token_and_opencode_identity() {
        let headers = static_headers("gho_secret", &[]).unwrap();
        assert_eq!(headers[AUTHORIZATION], "Bearer gho_secret");
        assert!(headers[AUTHORIZATION].is_sensitive());
        assert_eq!(headers[USER_AGENT], opencode::USER_AGENT);
        assert_eq!(headers["x-github-api-version"], API_VERSION);
        assert_eq!(headers["openai-intent"], "conversation-edits");
        assert!(!format!("{headers:?}").contains("gho_secret"));
        let custom = static_headers("t", &[("Openai-Intent".into(), "other".into())]).unwrap();
        assert_eq!(custom["openai-intent"], "other");
        assert!(static_headers("bad\ntoken", &[]).is_err());
    }

    #[test]
    fn only_user_input_is_user_initiated() {
        let reminder = ContentBlock::text("<system-reminder>\nx\n</system-reminder>");
        assert_eq!(initiator(vec![Message::user_text("hi")]), "user");
        assert_eq!(initiator(vec![result(vec![])]), "agent");
        assert_eq!(initiator(vec![result(vec![reminder])]), "agent");
        assert_eq!(
            initiator(vec![result(vec![ContentBlock::text("stop")])]),
            "user"
        );
        assert_eq!(
            initiator(vec![
                Message::user_text("hi"),
                Message::assistant_text("partial")
            ]),
            "agent"
        );
        assert_eq!(initiator(vec![]), "agent");
    }

    #[test]
    fn images_anywhere_mark_a_vision_request() {
        let plain = request(vec![Message::user_text("hi")]);
        assert!(!request_headers(&plain, Route::Chat).contains_key("copilot-vision-request"));
        let pasted = request(vec![Message::new(
            Role::User,
            vec![ContentBlock::Image { source: png() }],
        )]);
        let shot = request(vec![Message::new(
            Role::User,
            vec![ContentBlock::ToolResult {
                tool_use_id: CallId::from("c1"),
                content: vec![ToolResultPart::Image { source: png() }],
                is_error: false,
            }],
        )]);
        for vision in [pasted, shot] {
            assert_eq!(
                request_headers(&vision, Route::Responses)["copilot-vision-request"],
                "true"
            );
        }
    }

    #[test]
    fn sessions_and_the_thinking_beta_are_per_request() {
        let mut req = request(vec![Message::user_text("hi")]);
        assert!(!request_headers(&req, Route::Chat).contains_key("x-interaction-id"));
        req.session_key = Some("01K5ZQ8X1B2C3D4E5F6G7H8J9K".into());
        req.thinking = Some(ThinkingConfig {
            effort: Effort::Medium,
            budget_tokens: None,
        });
        req.tools = vec![ToolSpec {
            name: "Read".into(),
            description: "Read a file".into(),
            input_schema: json!({"type": "object"}),
        }];
        let chat = request_headers(&req, Route::Chat);
        assert_eq!(chat["x-interaction-id"], "01K5ZQ8X1B2C3D4E5F6G7H8J9K");
        assert!(!chat.contains_key("anthropic-beta"));
        let messages = request_headers(&req, Route::Messages);
        assert_eq!(
            messages["anthropic-beta"],
            "interleaved-thinking-2025-05-14"
        );
        req.thinking = None;
        assert!(!request_headers(&req, Route::Messages).contains_key("anthropic-beta"));
    }
}
