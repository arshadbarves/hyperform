//! `Read`: text with `cat -n` numbering and offset/limit windows, images,
//! PDFs by page, notebooks by cell, and a size note for binary files.
//! Every successful read is recorded for read-before-edit.

use std::path::Path;

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_host::{FileKind, HostError, read_text, sniff};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, path_field};
use crate::names;
use crate::output::ToolOutput;
use crate::reading::{self, parse_pages, read_pdf, window};
use crate::schema;
use crate::tool::Tool;

/// Largest text file `Read` loads; later parts need Grep.
const MAX_TEXT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        names::READ
    }

    fn description(&self) -> String {
        prompts::READ.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "file_path": {"type": "string", "description": "The file to read: an absolute path, or a path relative to the project root."},
                "offset": {"type": "integer", "description": "The line number to start reading from (1-based). Use it with limit for large files."},
                "limit": {"type": "integer", "minimum": 1, "description": "The number of lines to read (default 2000)."},
                "pages": {"type": "string", "description": "PDF page range such as \"1-5\" or \"3\"; at most 20 pages per call. PDF files only."}
            }),
            &["file_path"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, input: &Value, ctx: &ToolCtx) -> Action {
        Action::Read {
            paths: path_field(input, "file_path", ctx).into_iter().collect(),
        }
    }

    fn title(&self, input: &Value, ctx: &ToolCtx) -> String {
        match path_field(input, "file_path", ctx) {
            Some(path) => format!("Read {}", ctx.display(&path)),
            None => "Read".to_string(),
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let path = ctx.resolve(fields.non_empty("file_path")?);
        let offset = fields.usize("offset")?.unwrap_or(1).max(1);
        let limit = match fields.usize("limit")? {
            Some(0) => return Err(ToolError::invalid("`limit` must be at least 1")),
            Some(limit) => limit,
            None => ctx.limits.read_default_lines,
        };
        let pages = match fields.value("pages") {
            Some(Value::Number(number)) => Some(number.to_string()),
            _ => fields.optional_text("pages")?.map(str::to_string),
        };
        let kind = sniff(&path).await.map_err(|e| missing(ctx, &path, e))?;
        if pages.is_some() && kind != FileKind::Pdf {
            return Err(ToolError::invalid(
                "`pages` applies only to PDF files; use offset and limit for other files",
            ));
        }
        let output = match kind {
            FileKind::Text => read_lines(ctx, &path, offset, limit).await?,
            FileKind::Image { media_type } => reading::image(ctx, &path, &media_type).await?,
            FileKind::Pdf => {
                let pages = pages
                    .as_deref()
                    .map(parse_pages)
                    .transpose()
                    .map_err(ToolError::invalid)?;
                read_pdf(ctx, &path, pages).await?
            }
            FileKind::Notebook => reading::notebook(ctx, &path).await?,
            FileKind::Binary => reading::binary(ctx, &path).await,
        };
        if let Err(e) = ctx.files.record_read(&path) {
            tracing::warn!(path = %path.display(), error = %e, "could not record the read");
        }
        Ok(output.read(path))
    }
}

fn missing(ctx: &ToolCtx, path: &Path, error: HostError) -> ToolError {
    let display = ctx.display(path);
    if error.is_not_found() {
        return ToolError::failed(format!(
            "File does not exist: {display}. The project root is {}.",
            ctx.root.display()
        ));
    }
    match error {
        HostError::Invalid(reason) => ToolError::failed(format!(
            "{reason}. Read only reads files: use Glob to find files in a directory, or Bash with `ls` to list it."
        )),
        other => ToolError::failed(format!("cannot read {display}: {other}")),
    }
}

async fn read_lines(
    ctx: &ToolCtx,
    path: &Path,
    offset: usize,
    limit: usize,
) -> Result<ToolOutput, ToolError> {
    let file = read_text(path, MAX_TEXT_BYTES)
        .await
        .map_err(ToolError::host_failed)?;
    let display = ctx.display(path);
    if file.content.is_empty() {
        return Ok(ToolOutput::text(
            format!("{display} exists but is empty."),
            "Empty file",
        ));
    }
    let view = window(
        &file.content,
        offset,
        limit,
        ctx.limits.read_max_line_chars,
        ctx.limits.max_result_chars,
    )
    .map_err(ToolError::invalid)?;
    let mut body = view.body.clone();
    if file.truncated {
        body.push_str(&format!(
            "\n(Only the first {} of this {} file were loaded; search later parts with Grep.)\n",
            reading::human_size(MAX_TEXT_BYTES as u64),
            reading::human_size(file.len)
        ));
    }
    if file.lossy {
        body.push_str("\n(The file is not valid UTF-8; invalid bytes are shown as U+FFFD.)\n");
    }
    Ok(ToolOutput::text(body, view.summary()))
}
