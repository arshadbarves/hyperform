//! `WebFetch`: fetches a page (guarded, cached, converted to markdown) and
//! answers the prompt with the side model, or returns the page itself.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_host::{DEFAULT_FETCH_MAX_BYTES, FetchOptions, FetchedPage, HostError};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::reading::human_size;
use crate::schema;
use crate::text::{truncate_output, truncate_with};
use crate::tool::Tool;

/// Page characters handed to the side model.
const EXTRACT_MAX_CHARS: usize = 100_000;
/// Body characters shown with an HTTP error status.
const ERROR_BODY_CHARS: usize = 2_000;

#[derive(Debug, Default)]
pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        names::WEB_FETCH
    }

    fn description(&self) -> String {
        prompts::WEB_FETCH.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "url": {"type": "string", "format": "uri", "description": "The URL to fetch."},
                "prompt": {"type": "string", "description": "What to find out from the page."}
            }),
            &["url", "prompt"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Fetch {
            url: str_field(input, "url")
                .unwrap_or_default()
                .trim()
                .to_string(),
        }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        format!("Fetch {}", str_field(input, "url").unwrap_or("").trim())
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let url = fields.non_empty("url")?.trim();
        let prompt = fields.non_empty("prompt")?;
        let options = FetchOptions {
            max_bytes: DEFAULT_FETCH_MAX_BYTES,
            allow_private_network: ctx.web_options.allow_private_network,
        };
        let page = ctx
            .web
            .fetch(url, &options, ctx.cancel.clone())
            .await
            .map_err(|e| fetch_error(url, e))?;
        if let Some(target) = &page.redirect {
            return Ok(ToolOutput::text(
                format!(
                    "{url} redirected to a different host: {target}\nThe redirect was not followed. To read that page, call WebFetch again with url \"{target}\" and the same prompt."
                ),
                format!("Redirected to {}", host_of(target)),
            ));
        }
        if page.status >= 400 {
            let body = truncate_with(&page.content, ERROR_BODY_CHARS, |_| None);
            return Ok(ToolOutput::text(
                format!("HTTP {} from {}.\n\n{body}", page.status, page.final_url),
                format!("HTTP {}", page.status),
            )
            .with_error(true));
        }
        answer(ctx, &page, prompt).await
    }
}

async fn answer(ctx: &ToolCtx, page: &FetchedPage, prompt: &str) -> Result<ToolOutput, ToolError> {
    let summary = format!(
        "Fetched {} ({})",
        host_of(&page.final_url),
        human_size(page.content.len() as u64)
    );
    let truncated = if page.truncated {
        "\n(The page was truncated because it is very large.)"
    } else {
        ""
    };
    let mut note = String::new();
    if let Some(side) = ctx
        .ports
        .side_model
        .clone()
        .filter(|_| ctx.web_options.fetch_extract)
    {
        let document = format!(
            "URL: {}{truncated}\n\n{}",
            page.final_url,
            truncate_with(&page.content, EXTRACT_MAX_CHARS, |_| None)
        );
        match ctx
            .until_cancelled(side.extract(ctx, document, prompt.to_string()))
            .await?
        {
            Ok(answer) => {
                return Ok(ToolOutput::text(
                    format!("{}\n\n(Source: {})", answer.trim_end(), page.final_url),
                    summary,
                ));
            }
            Err(e) => {
                note = format!("(The page could not be summarized: {e}. Its content follows.)\n\n");
            }
        }
    }
    let cached = if page.from_cache { " (cached)" } else { "" };
    let text = format!(
        "{note}Content of {}{cached}:{truncated}\n\n{}",
        page.final_url, page.content
    );
    Ok(ToolOutput::text(
        truncate_output(ctx, "webfetch", &text),
        summary,
    ))
}

fn fetch_error(url: &str, error: HostError) -> ToolError {
    match error {
        HostError::Cancelled => ToolError::Cancelled,
        HostError::Invalid(reason) => ToolError::invalid(reason),
        HostError::Blocked(reason) => ToolError::failed(format!("{url} was not fetched: {reason}")),
        HostError::Timeout => ToolError::failed(format!("fetching {url} timed out")),
        other => ToolError::failed(format!("fetching {url} failed: {other}")),
    }
}

/// The host part of a URL, for summaries.
pub(crate) fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_are_extracted_for_summaries() {
        assert_eq!(host_of("https://docs.rs/tokio/latest"), "docs.rs");
        assert_eq!(host_of("http://127.0.0.1:8080?q=1"), "127.0.0.1:8080");
        assert_eq!(host_of("example.com"), "example.com");
    }
}
