//! `WebSearch`: results from the configured search backend, filtered by
//! allowed and blocked domains.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_host::{HostError, SearchBackend};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::text::truncate_output;
use crate::tool::Tool;

const MAX_RESULTS: usize = 10;

#[derive(Debug, Default)]
pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        names::WEB_SEARCH
    }

    fn description(&self) -> String {
        prompts::WEB_SEARCH.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "query": {"type": "string", "minLength": 2, "description": "The search query."},
                "allowed_domains": {"type": "array", "items": {"type": "string"}, "description": "Only return results from these domains."},
                "blocked_domains": {"type": "array", "items": {"type": "string"}, "description": "Never return results from these domains."}
            }),
            &["query"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Search
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        format!(
            "Search the web for \"{}\"",
            str_field(input, "query").unwrap_or("").trim()
        )
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let query = fields.non_empty("query")?.trim();
        if query.chars().count() < 2 {
            return Err(ToolError::invalid("`query` must be at least 2 characters"));
        }
        let allowed: Vec<String> = fields.parse("allowed_domains")?.unwrap_or_default();
        let blocked: Vec<String> = fields.parse("blocked_domains")?.unwrap_or_default();
        if ctx.web_options.search == SearchBackend::None {
            return Err(ToolError::unavailable(
                "web search is not configured; the user can choose a search provider in settings",
            ));
        }
        let hits = ctx
            .web
            .search(
                &ctx.web_options.search,
                query,
                &allowed,
                &blocked,
                MAX_RESULTS,
                ctx.cancel.clone(),
            )
            .await
            .map_err(|e| match e {
                HostError::Cancelled => ToolError::Cancelled,
                HostError::Invalid(reason) => ToolError::invalid(reason),
                other => ToolError::failed(format!("the web search failed: {other}")),
            })?;
        if hits.is_empty() {
            return Ok(ToolOutput::text(
                format!("No results found for \"{query}\"."),
                "No results",
            ));
        }
        let mut text = format!("Web search results for \"{query}\":\n");
        for (index, hit) in hits.iter().enumerate() {
            let title = if hit.title.is_empty() {
                &hit.url
            } else {
                &hit.title
            };
            text.push_str(&format!("\n{}. {title}\n   {}\n", index + 1, hit.url));
            if !hit.snippet.is_empty() {
                text.push_str(&format!("   {}\n", hit.snippet));
            }
        }
        let count = hits.len();
        Ok(ToolOutput::text(
            truncate_output(ctx, "websearch", &text),
            format!("{count} {}", if count == 1 { "result" } else { "results" }),
        ))
    }
}
