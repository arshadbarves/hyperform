//! `Grep`: ripgrep-compatible content search with the Claude Code inputs
//! (`output_mode`, `-i`, `-n`, `-A`/`-B`/`-C`, `head_limit`, `offset`).

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_host::{GrepMode, GrepQuery, HostError, grep};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, path_field, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::text::{fit_entries, grep_entries, rank_before_cut, truncate_output};
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct GrepTool;

fn query(fields: &Fields<'_>, ctx: &ToolCtx) -> Result<GrepQuery, ToolError> {
    let mode = match fields.optional_text("output_mode")? {
        None | Some("files_with_matches") => GrepMode::FilesWithMatches,
        Some("content") => GrepMode::Content,
        Some("count") => GrepMode::Count,
        Some(other) => {
            return Err(ToolError::invalid(format!(
                "`output_mode` must be content, files_with_matches, or count, not {other:?}"
            )));
        }
    };
    let context = fields.usize("-C")?;
    Ok(GrepQuery {
        pattern: fields.required_str("pattern")?.to_string(),
        path: fields.optional_text("path")?.map(|path| ctx.resolve(path)),
        glob: fields.optional_text("glob")?.map(str::to_string),
        file_type: fields.optional_text("type")?.map(str::to_string),
        case_insensitive: fields.bool("-i")?.unwrap_or(false),
        multiline: fields.bool("multiline")?.unwrap_or(false),
        before: fields.usize("-B")?.or(context).unwrap_or(0),
        after: fields.usize("-A")?.or(context).unwrap_or(0),
        mode,
        line_numbers: fields.bool("-n")?.unwrap_or(true),
        head_limit: fields.usize("head_limit")?.filter(|&limit| limit > 0),
        offset: fields.usize("offset")?.unwrap_or(0),
    })
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        names::GREP
    }

    fn description(&self) -> String {
        prompts::GREP.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "pattern": {"type": "string", "description": "The regular expression to search for (ripgrep syntax)."},
                "path": {"type": "string", "description": "File or directory to search in (default: the project root)."},
                "glob": {"type": "string", "description": "Only search files matching this glob, e.g. \"*.js\" or \"**/*.{ts,tsx}\"."},
                "type": {"type": "string", "description": "Only search files of this ripgrep type, e.g. \"rust\", \"py\", \"js\"."},
                "output_mode": {"type": "string", "enum": ["content", "files_with_matches", "count"], "description": "content (matching lines), files_with_matches (paths, the default), or count (matches per file)."},
                "-i": {"type": "boolean", "description": "Case-insensitive search."},
                "-n": {"type": "boolean", "description": "Show line numbers in content mode (default true)."},
                "-A": {"type": "integer", "minimum": 0, "description": "Lines of context after each match (content mode)."},
                "-B": {"type": "integer", "minimum": 0, "description": "Lines of context before each match (content mode)."},
                "-C": {"type": "integer", "minimum": 0, "description": "Lines of context before and after each match (content mode)."},
                "multiline": {"type": "boolean", "description": "Let patterns span lines and `.` match newlines (default false)."},
                "head_limit": {"type": "integer", "minimum": 1, "description": "Return only the first N lines or entries."},
                "offset": {"type": "integer", "minimum": 0, "description": "Skip the first N lines or entries (use with head_limit to page)."}
            }),
            &["pattern"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, input: &Value, ctx: &ToolCtx) -> Action {
        Action::Read {
            paths: vec![path_field(input, "path", ctx).unwrap_or_else(|| ctx.root.clone())],
        }
    }

    fn title(&self, input: &Value, ctx: &ToolCtx) -> String {
        let pattern = str_field(input, "pattern").unwrap_or("");
        match path_field(input, "path", ctx) {
            Some(path) => format!("Grep \"{pattern}\" in {}", ctx.display(&path)),
            None => format!("Grep \"{pattern}\""),
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let query = query(&fields, ctx)?;
        let result = grep(&ctx.root, &query, ctx.cancel.clone())
            .await
            .map_err(|e| match e {
                HostError::NotFound(_) => ToolError::failed(format!(
                    "Path does not exist: {}",
                    query
                        .path
                        .as_deref()
                        .map_or_else(String::new, |p| ctx.display(p))
                )),
                other => other.into(),
            })?;
        let files_label = |n: usize| if n == 1 { "file" } else { "files" };
        if result.text.is_empty() {
            let text = if query.offset > 0 && result.files > 0 {
                format!("No results past offset {}", query.offset)
            } else {
                "No matches found".to_string()
            };
            return Ok(ToolOutput::text(text, "No matches"));
        }
        let (mut text, summary) = match query.mode {
            GrepMode::FilesWithMatches => (
                format!(
                    "Found {} {}\n{}\n",
                    result.files,
                    files_label(result.files),
                    result.text
                ),
                format!("Found {} {}", result.files, files_label(result.files)),
            ),
            GrepMode::Count => (
                format!(
                    "{}\n\nFound {} matches across {} {}.\n",
                    result.text,
                    result.matches,
                    result.files,
                    files_label(result.files)
                ),
                format!(
                    "{} matches in {} {}",
                    result.matches,
                    result.files,
                    files_label(result.files)
                ),
            ),
            GrepMode::Content => (
                format!("{}\n", result.text),
                format!(
                    "Found {} matches in {} {}",
                    result.matches,
                    result.files,
                    files_label(result.files)
                ),
            ),
        };
        if result.truncated {
            let shown = result.text.lines().count();
            text.push_str(&match query.head_limit {
                Some(_) => format!(
                    "\n(More results are available: continue with offset={}.)\n",
                    query.offset + shown
                ),
                None => "\n(Results were truncated. Narrow the search with path, glob, or type, or page through it with head_limit and offset.)\n".to_string(),
            });
        }
        if let Some(ranked) = ranked(ctx, &query, &result.text, &text).await {
            return Ok(ToolOutput::text(ranked, summary));
        }
        Ok(ToolOutput::text(
            truncate_output(ctx, "grep", &text),
            summary,
        ))
    }
}

/// Room for the closing note of ranked results.
const NOTE_ROOM: usize = 400;

/// Results over the budget, when no page was asked for: the most relevant
/// to the task first, while they fit; the full text is spilled. `None`
/// keeps today's head-and-tail cut.
async fn ranked(ctx: &ToolCtx, query: &GrepQuery, body: &str, full: &str) -> Option<String> {
    let budget = ctx.limits.max_result_chars;
    if full.chars().count() <= budget || query.head_limit.is_some() || query.offset > 0 {
        return None;
    }
    let entries = grep_entries(body, query.mode == GrepMode::Content);
    let order = rank_before_cut(ctx, names::GREP, &query.pattern, &entries).await?;
    let saved = ctx.spill_output("grep", full);
    let room = budget.saturating_sub(NOTE_ROOM).max(budget / 2);
    let (shown_text, shown) = fit_entries(&entries, &order, room);
    let place = saved.map_or_else(String::new, |path| {
        format!(
            " The full results are saved at {}; read them with Read (offset and limit) or narrow the search with path, glob, or type.",
            path.display()
        )
    });
    Some(format!(
        "{shown_text}\n({shown} of {} results shown, the most relevant to the task first.{place})\n",
        entries.len()
    ))
}
