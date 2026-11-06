//! `NotebookEdit`: replace, insert, or delete one cell of a Jupyter notebook
//! the model has read, preserving everything else in Jupyter's format.

use std::path::Path;

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;
use z_engine_protocol::Preview;

use crate::context::ToolCtx;
use crate::edit::{ensure_fresh, load_text, save};
use crate::error::ToolError;
use crate::input::{Fields, path_field, str_field};
use crate::names;
use crate::notebook::{CellType, Notebook, cell_id, joined};
use crate::output::ToolOutput;
use crate::schema;
use crate::text::{cap_preview, unified_diff};
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct NotebookEditTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Replace,
    Insert,
    Delete,
}

#[derive(Debug)]
struct Request {
    cell_id: Option<String>,
    source: String,
    cell_type: Option<CellType>,
    mode: Mode,
}

/// What the edit did, for the result and the preview.
#[derive(Debug)]
struct Change {
    message: String,
    before: String,
    after: String,
}

fn request(fields: &Fields<'_>) -> Result<Request, ToolError> {
    let mode = match fields.optional_text("edit_mode")? {
        None | Some("replace") => Mode::Replace,
        Some("insert") => Mode::Insert,
        Some("delete") => Mode::Delete,
        Some(other) => {
            return Err(ToolError::invalid(format!(
                "`edit_mode` must be replace, insert, or delete, not {other:?}"
            )));
        }
    };
    let cell_type = fields
        .optional_text("cell_type")?
        .map(|kind| {
            CellType::parse(kind).ok_or_else(|| {
                ToolError::invalid(format!(
                    "`cell_type` must be code or markdown, not {kind:?}"
                ))
            })
        })
        .transpose()?;
    Ok(Request {
        cell_id: fields.optional_text("cell_id")?.map(str::to_string),
        source: fields.str("new_source")?.unwrap_or_default().to_string(),
        cell_type,
        mode,
    })
}

fn apply(nb: &mut Notebook, req: &Request, display: &str) -> Result<Change, ToolError> {
    let locate = |nb: &Notebook, id: &str| {
        nb.find(id).ok_or_else(|| {
            ToolError::failed(format!(
                "cell {id:?} was not found in {display}. Read shows every cell id; notebooks without ids use cell-N, counting from 0."
            ))
        })
    };
    let required_id = || {
        req.cell_id
            .as_deref()
            .ok_or_else(|| ToolError::invalid("`cell_id` is required to replace or delete a cell"))
    };
    match req.mode {
        Mode::Replace => {
            let index = locate(nb, required_id()?)?;
            let id = cell_id(&nb.cells()[index], index);
            let before = joined(nb.cells()[index].get("source"));
            nb.replace(index, &req.source, req.cell_type)
                .map_err(ToolError::failed)?;
            Ok(Change {
                message: format!("Updated cell {id} in {display}."),
                before,
                after: req.source.clone(),
            })
        }
        Mode::Insert => {
            let kind = req.cell_type.ok_or_else(|| {
                ToolError::invalid("`cell_type` (code or markdown) is required to insert a cell")
            })?;
            let (at, place) = match req.cell_id.as_deref() {
                Some(id) => (locate(nb, id)? + 1, format!("after cell {id}")),
                None => (0, "at the top".to_string()),
            };
            let id = nb
                .insert(at, &req.source, kind)
                .map_err(ToolError::failed)?;
            Ok(Change {
                message: format!("Inserted {} cell {id} {place} in {display}.", kind.label()),
                before: String::new(),
                after: req.source.clone(),
            })
        }
        Mode::Delete => {
            let index = locate(nb, required_id()?)?;
            let id = cell_id(&nb.cells()[index], index);
            let removed = nb.delete(index).map_err(ToolError::failed)?;
            Ok(Change {
                message: format!("Deleted cell {id} from {display}."),
                before: joined(removed.get("source")),
                after: String::new(),
            })
        }
    }
}

async fn load(ctx: &ToolCtx, path: &Path) -> Result<Notebook, ToolError> {
    let text = load_text(ctx, path).await?.ok_or_else(|| {
        ToolError::failed(format!("Notebook does not exist: {}", ctx.display(path)))
    })?;
    Notebook::parse(&text).map_err(|reason| {
        ToolError::failed(format!(
            "{} cannot be edited as a notebook because {reason}",
            ctx.display(path)
        ))
    })
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str {
        names::NOTEBOOK_EDIT
    }

    fn description(&self) -> String {
        prompts::NOTEBOOK_EDIT.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "notebook_path": {"type": "string", "description": "The notebook (.ipynb) to edit: an absolute path, or a path relative to the project root."},
                "cell_id": {"type": "string", "description": "The cell to replace or delete, or to insert after; as shown by Read (cell-N for notebooks without ids)."},
                "new_source": {"type": "string", "description": "The new source of the cell (ignored for delete)."},
                "cell_type": {"type": "string", "enum": ["code", "markdown"], "description": "The cell type; required for insert."},
                "edit_mode": {"type": "string", "enum": ["replace", "insert", "delete"], "description": "replace (default), insert, or delete."}
            }),
            &["notebook_path", "new_source"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        false
    }

    fn action(&self, input: &Value, ctx: &ToolCtx) -> Action {
        Action::Write {
            paths: path_field(input, "notebook_path", ctx)
                .into_iter()
                .collect(),
        }
    }

    fn title(&self, input: &Value, ctx: &ToolCtx) -> String {
        let Some(path) = path_field(input, "notebook_path", ctx) else {
            return "NotebookEdit".to_string();
        };
        let display = ctx.display(&path);
        let cell = str_field(input, "cell_id").unwrap_or("?");
        match str_field(input, "edit_mode") {
            Some("insert") => format!("Insert cell in {display}"),
            Some("delete") => format!("Delete cell {cell} in {display}"),
            _ => format!("Edit cell {cell} in {display}"),
        }
    }

    async fn preview(&self, input: &Value, ctx: &ToolCtx) -> Option<Preview> {
        let path = path_field(input, "notebook_path", ctx)?;
        let req = request(&Fields::new(input).ok()?).ok()?;
        let display = ctx.display(&path);
        let mut nb = load(ctx, &path).await.ok()?;
        let change = apply(&mut nb, &req, &display).ok()?;
        Some(Preview::Diff {
            diff: cap_preview(unified_diff(&change.before, &change.after, &display)),
            path: display,
        })
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let path = ctx.resolve(fields.non_empty("notebook_path")?);
        let req = request(&fields)?;
        if path
            .extension()
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("ipynb"))
        {
            return Err(ToolError::invalid(
                "NotebookEdit edits Jupyter notebooks (.ipynb); use Edit for other files",
            ));
        }
        let display = ctx.display(&path);
        let _guard = ctx.locks.lock(&path).await;
        let mut nb = load(ctx, &path).await?;
        ensure_fresh(ctx, &path)?;
        let change = apply(&mut nb, &req, &display)?;
        let text = nb.to_json().map_err(ToolError::failed)?;
        save(ctx, &path, &text).await?;
        Ok(ToolOutput::text(change.message.clone(), change.message).wrote(path))
    }
}
