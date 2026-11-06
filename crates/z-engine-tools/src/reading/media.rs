//! Non-text views behind `Read`: images, notebooks, and binary files.

use std::path::Path;

use z_engine_host::{read_image, read_text};
use z_engine_protocol::{MediaSource, ToolResultPart};

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::notebook::{Notebook, render};
use crate::output::ToolOutput;
use crate::text::truncate_output;

/// Largest notebook file `Read` loads.
const MAX_NOTEBOOK_BYTES: usize = 64 * 1024 * 1024;

pub(crate) async fn image(
    ctx: &ToolCtx,
    path: &Path,
    media_type: &str,
) -> Result<ToolOutput, ToolError> {
    let display = ctx.display(path);
    if !ctx.vision {
        let size = file_len(path)
            .await
            .map_or_else(String::new, |len| format!(", {}", human_size(len)));
        return Ok(ToolOutput::text(
            format!(
                "{display} is an image ({media_type}{size}). The current model cannot view images, so its contents are not shown."
            ),
            "Image (not viewable by this model)",
        ));
    }
    let (media_type, data) = read_image(path, ctx.limits.max_image_bytes)
        .await
        .map_err(ToolError::host_failed)?;
    let size = human_size(data.len() as u64 * 3 / 4);
    Ok(ToolOutput::parts(
        vec![
            ToolResultPart::Text {
                text: format!("Image {display} ({media_type}, {size}):"),
            },
            ToolResultPart::Image {
                source: MediaSource::Base64 { media_type, data },
            },
        ],
        format!("Read image ({size})"),
    ))
}

pub(crate) async fn notebook(ctx: &ToolCtx, path: &Path) -> Result<ToolOutput, ToolError> {
    let file = read_text(path, MAX_NOTEBOOK_BYTES)
        .await
        .map_err(ToolError::host_failed)?;
    let display = ctx.display(path);
    let nb = match Notebook::parse(&file.content) {
        Ok(nb) => nb,
        Err(reason) => {
            let text = format!(
                "{display} cannot be shown as a notebook because {reason}.\n\n{}",
                file.content
            );
            return Ok(ToolOutput::text(
                truncate_output(ctx, "read", &text),
                "Read invalid notebook",
            ));
        }
    };
    let cells = nb.cells().len();
    let header = format!(
        "Notebook {display} ({}, {cells} cells). Edit cells with NotebookEdit using the ids below.\n\n",
        nb.language().unwrap_or("unknown language")
    );
    let mut parts = render(&nb, ctx.vision, ctx.limits.max_image_bytes);
    let text_chars: usize = parts
        .iter()
        .map(|part| match part {
            ToolResultPart::Text { text } => text.chars().count(),
            ToolResultPart::Image { .. } => 0,
        })
        .sum();
    if text_chars + header.len() > ctx.limits.max_result_chars {
        let flat: String = render(&nb, false, 0)
            .into_iter()
            .filter_map(|part| match part {
                ToolResultPart::Text { text } => Some(text),
                ToolResultPart::Image { .. } => None,
            })
            .collect();
        parts = vec![ToolResultPart::Text {
            text: truncate_output(ctx, "read", &format!("{header}{flat}")),
        }];
    } else {
        parts.insert(0, ToolResultPart::Text { text: header });
    }
    Ok(ToolOutput::parts(
        parts,
        format!("Read notebook ({cells} cells)"),
    ))
}

pub(crate) async fn binary(ctx: &ToolCtx, path: &Path) -> ToolOutput {
    let size = file_len(path)
        .await
        .map_or_else(|| "unknown size".to_string(), human_size);
    ToolOutput::text(
        format!(
            "{} is a binary file ({size}); its contents are not shown.",
            ctx.display(path)
        ),
        format!("Binary file ({size})"),
    )
}

/// The file's size on disk (reads no content).
async fn file_len(path: &Path) -> Option<u64> {
    read_text(path, 0).await.ok().map(|file| file.len)
}

pub(crate) fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["bytes", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} bytes")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_human_readable() {
        assert_eq!(human_size(12), "12 bytes");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }
}
