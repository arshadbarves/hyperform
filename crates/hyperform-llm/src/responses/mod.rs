//! OpenAI Responses API adapter. Crate-internal: GitHub Copilot routes
//! GPT-5 class models here; it is not a settings wire format.

mod messages;
mod reasoning;
mod request;
mod stream;

pub(crate) use reasoning::is_responses_signature;
#[cfg(test)]
pub(crate) use reasoning::signature;
pub(crate) use request::build_body;
pub(crate) use stream::ResponsesStreamParser;
