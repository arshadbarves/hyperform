//! Native Anthropic Messages API adapter.

mod client;
mod messages;
mod request;
mod stream;

pub(crate) use client::{ANTHROPIC_VERSION, AnthropicClient, beta_headers};
pub(crate) use request::build_body;
pub(crate) use stream::MessagesStreamParser;
