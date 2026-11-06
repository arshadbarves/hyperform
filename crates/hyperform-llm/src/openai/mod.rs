//! OpenAI-compatible Chat Completions adapter.

mod client;
mod messages;
mod request;
mod stream;

pub(crate) use client::OpenAiChatClient;
pub(crate) use request::{ChatDialect, build_body, effort_label};
pub(crate) use stream::ChatStreamParser;
