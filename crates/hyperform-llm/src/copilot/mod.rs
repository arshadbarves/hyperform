//! GitHub Copilot: device sign-in, per-model routing over Chat
//! Completions, Responses and Anthropic Messages, and Copilot's headers.

mod body;
mod client;
mod headers;
mod identity;
mod login;
mod models;

pub use client::build_copilot_client;
pub(crate) use identity::is_copilot_host;
pub use identity::{copilot_login_url, is_copilot_url};
pub use login::{DeviceCode, LoginPoll, poll_login, start_login};
