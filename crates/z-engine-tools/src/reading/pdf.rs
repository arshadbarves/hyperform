//! PDFs for `Read`: page ranges (`"3"`, `"1-5"`, `"10-"`) capped at 20 pages
//! per call, and the extracted text laid out page by page.

use std::path::Path;

use z_engine_host::pdf_text;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::output::ToolOutput;
use crate::text::truncate_output;

pub(crate) const MAX_PAGES: usize = 20;

/// An inclusive, 1-based page range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pages {
    pub(crate) first: usize,
    pub(crate) last: usize,
}

pub(crate) fn parse_pages(spec: &str) -> Result<Pages, String> {
    let invalid = || {
        format!(
            "`pages` must look like \"3\", \"1-5\", or \"10-\" (pages count from 1), not {spec:?}"
        )
    };
    let number = |text: &str| text.trim().parse::<usize>().ok().filter(|&n| n >= 1);
    let spec_trimmed = spec.trim();
    let pages = match spec_trimmed.split_once('-') {
        None => {
            let page = number(spec_trimmed).ok_or_else(invalid)?;
            Pages {
                first: page,
                last: page,
            }
        }
        Some((first, "")) => {
            let first = number(first).ok_or_else(invalid)?;
            Pages {
                first,
                last: first + MAX_PAGES - 1,
            }
        }
        Some((first, last)) => Pages {
            first: number(first).ok_or_else(invalid)?,
            last: number(last).ok_or_else(invalid)?,
        },
    };
    if pages.last < pages.first {
        return Err(invalid());
    }
    if pages.last - pages.first + 1 > MAX_PAGES {
        return Err(format!(
            "`pages` spans {} pages; read at most {MAX_PAGES} pages per call",
            pages.last - pages.first + 1
        ));
    }
    Ok(pages)
}

pub(crate) async fn read_pdf(
    ctx: &ToolCtx,
    path: &Path,
    pages: Option<Pages>,
) -> Result<ToolOutput, ToolError> {
    let wanted = pages.unwrap_or(Pages {
        first: 1,
        last: MAX_PAGES,
    });
    // One extra page tells whether more follow.
    let extracted = pdf_text(path, wanted.last + 1)
        .await
        .map_err(ToolError::host_failed)?;
    let display = ctx.display(path);
    if extracted.len() < wanted.first {
        return Err(ToolError::failed(format!(
            "{display} has {} pages, so there is no page {}",
            extracted.len(),
            wanted.first
        )));
    }
    let last = wanted.last.min(extracted.len());
    let mut text = format!("PDF {display}: pages {}-{last}\n", wanted.first);
    let mut any_text = false;
    for (index, page) in extracted[wanted.first - 1..last].iter().enumerate() {
        text.push_str(&format!("\n--- Page {} ---\n", wanted.first + index));
        if page.trim().is_empty() {
            text.push_str("(no extractable text on this page)\n");
        } else {
            any_text = true;
            text.push_str(page.trim());
            text.push('\n');
        }
    }
    if !any_text {
        text.push_str(
            "\n(These pages have no extractable text; the PDF may contain scanned images.)\n",
        );
    }
    if extracted.len() > last {
        text.push_str(&format!(
            "\n(More pages follow. Read them with pages=\"{}-{}\".)\n",
            last + 1,
            last + MAX_PAGES
        ));
    }
    Ok(ToolOutput::text(
        truncate_output(ctx, "read", &text),
        format!("Read PDF pages {}-{last}", wanted.first),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ranges_parse_and_respect_the_cap() {
        assert_eq!(parse_pages("3"), Ok(Pages { first: 3, last: 3 }));
        assert_eq!(parse_pages(" 1-5 "), Ok(Pages { first: 1, last: 5 }));
        assert_eq!(
            parse_pages("10-"),
            Ok(Pages {
                first: 10,
                last: 29
            })
        );
        assert!(parse_pages("0").is_err());
        assert!(parse_pages("5-2").is_err());
        assert!(parse_pages("a-b").is_err());
        assert!(parse_pages("1-21").unwrap_err().contains("at most 20"));
    }
}
