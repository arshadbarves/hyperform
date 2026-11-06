//! The OpenCode client identity HyperForm presents where only OpenCode is
//! served: OpenCode Zen's free tier and GitHub Copilot through OpenCode's
//! OAuth app.

/// Zen's free tier rejects older versions with HTTP 426.
pub(crate) const USER_AGENT: &str = "opencode/1.18.31";
