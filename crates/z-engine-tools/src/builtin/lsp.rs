//! `LSP`: language-server queries (definitions, references, symbols, call
//! hierarchy, diagnostics, rename previews), validated per operation.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, path_field, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::ports::LspRequest;
use crate::schema;
use crate::text::truncate_output;
use crate::tool::Tool;

const OPERATIONS: [&str; 10] = [
    "definition",
    "references",
    "hover",
    "documentSymbols",
    "workspaceSymbols",
    "implementations",
    "incomingCalls",
    "outgoingCalls",
    "diagnostics",
    "renamePreview",
];

/// Operations on the symbol at a position.
const POSITIONAL: [&str; 7] = [
    "definition",
    "references",
    "hover",
    "implementations",
    "incomingCalls",
    "outgoingCalls",
    "renamePreview",
];

#[derive(Debug, Default)]
pub struct LspTool;

fn request(fields: &Fields<'_>, ctx: &ToolCtx) -> Result<LspRequest, ToolError> {
    let operation = fields.non_empty("operation")?.trim();
    if !OPERATIONS.contains(&operation) {
        return Err(ToolError::invalid(format!(
            "`operation` must be one of {}, not {operation:?}",
            OPERATIONS.join(", ")
        )));
    }
    let req = LspRequest {
        operation: operation.to_string(),
        file_path: fields
            .optional_text("file_path")?
            .map(|path| ctx.resolve(path).display().to_string()),
        line: fields.u32("line")?,
        character: fields.u32("character")?,
        query: fields.optional_text("query")?.map(str::to_string),
        new_name: fields.optional_text("new_name")?.map(str::to_string),
    };
    let needs = |present: bool, field: &str| {
        if present {
            Ok(())
        } else {
            Err(ToolError::invalid(format!(
                "`{field}` is required for {operation}"
            )))
        }
    };
    if POSITIONAL.contains(&operation) || operation == "documentSymbols" {
        needs(req.file_path.is_some(), "file_path")?;
    }
    if POSITIONAL.contains(&operation) {
        needs(req.line.is_some_and(|line| line >= 1), "line (1-based)")?;
        needs(req.character.is_some_and(|c| c >= 1), "character (1-based)")?;
    }
    if operation == "workspaceSymbols" {
        needs(req.query.is_some(), "query")?;
    }
    if operation == "renamePreview" {
        needs(req.new_name.is_some(), "new_name")?;
    }
    Ok(req)
}

#[async_trait]
impl Tool for LspTool {
    fn name(&self) -> &str {
        names::LSP
    }

    fn description(&self) -> String {
        prompts::LSP.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "operation": {"type": "string", "enum": OPERATIONS, "description": "The query to run."},
                "file_path": {"type": "string", "description": "The file: absolute or relative to the project root."},
                "line": {"type": "integer", "minimum": 1, "description": "1-based line of the symbol."},
                "character": {"type": "integer", "minimum": 1, "description": "1-based column of the symbol."},
                "query": {"type": "string", "description": "Symbol name to search for (workspaceSymbols)."},
                "new_name": {"type": "string", "description": "The new name (renamePreview)."}
            }),
            &["operation"],
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
        let operation = str_field(input, "operation").unwrap_or("query");
        let Some(path) = path_field(input, "file_path", ctx) else {
            return format!("LSP {operation}");
        };
        let mut title = format!("LSP {operation} {}", ctx.display(&path));
        if let (Some(line), Some(character)) = (input.get("line"), input.get("character")) {
            title.push_str(&format!(":{line}:{character}"));
        }
        title
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let req = request(&Fields::new(&input)?, ctx)?;
        let operation = req.operation.clone();
        let lsp = ctx.ports.lsp()?;
        let answer = ctx
            .until_cancelled(lsp.query(ctx, req))
            .await?
            .map_err(ToolError::failed)?;
        let text = if answer.trim().is_empty() {
            format!("No results for {operation}.")
        } else {
            truncate_output(ctx, "lsp", &answer)
        };
        Ok(ToolOutput::text(text, format!("LSP {operation}")))
    }
}
