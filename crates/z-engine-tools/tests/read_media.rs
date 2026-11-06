//! `Read` on images (with and without vision), notebooks, and PDFs.

mod support;

use serde_json::json;
use support::{ctx, err_text, ok_text};
use z_engine_protocol::{MediaSource, ToolResultPart};
use z_engine_tools::Tool;
use z_engine_tools::builtin::ReadTool;

/// A valid 1x1 transparent PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

/// A minimal PDF with one Helvetica text line per page.
fn pdf(pages: &[&str]) -> Vec<u8> {
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        String::new(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut kids = Vec::new();
    for text in pages {
        let page_id = objects.len() + 1;
        kids.push(format!("{page_id} 0 R"));
        let content = format!("BT /F1 24 Tf 72 720 Td ({text}) Tj ET");
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            page_id + 1
        ));
        objects.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ));
    }
    objects[1] = format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids.join(" "),
        pages.len()
    );
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[tokio::test]
async fn images_are_image_parts_with_vision() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("shot.png"), PNG).unwrap();
    let output = ReadTool
        .call(json!({"file_path": "shot.png"}), &ctx(dir.path()))
        .await
        .unwrap();
    assert!(
        matches!(&output.content[0], ToolResultPart::Text { text } if text.starts_with("Image shot.png (image/png"))
    );
    let ToolResultPart::Image {
        source: MediaSource::Base64 { media_type, data },
    } = &output.content[1]
    else {
        panic!("{:?}", output.content);
    };
    assert_eq!(media_type, "image/png");
    assert!(!data.is_empty());
    assert!(output.summary.starts_with("Read image"));
}

#[tokio::test]
async fn images_without_vision_are_a_note() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("shot.png"), PNG).unwrap();
    let mut ctx = ctx(dir.path());
    ctx.vision = false;
    let output = ReadTool
        .call(json!({"file_path": "shot.png"}), &ctx)
        .await
        .unwrap();
    assert_eq!(output.content.len(), 1);
    assert!(
        output.text_content().contains("cannot view images"),
        "{}",
        output.text_content()
    );
    let mut ctx = ctx.clone();
    ctx.vision = true;
    ctx.limits.max_image_bytes = 10;
    let err = err_text(&ReadTool, &ctx, json!({"file_path": "shot.png"})).await;
    assert!(err.contains("the limit is 10"), "{err}");
}

#[tokio::test]
async fn notebooks_show_cells_with_ids_and_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = json!({
        "cells": [
            {"cell_type": "markdown", "id": "intro", "metadata": {}, "source": ["# Analysis"]},
            {"cell_type": "code", "id": "calc", "metadata": {}, "execution_count": 1, "source": ["1 + 1"],
             "outputs": [
                {"output_type": "execute_result", "data": {"text/plain": ["2"]}, "metadata": {}},
                {"output_type": "display_data", "data": {"image/png": "aGVsbG8="}, "metadata": {}}
             ]}
        ],
        "metadata": {"kernelspec": {"language": "python"}},
        "nbformat": 4, "nbformat_minor": 5
    });
    std::fs::write(dir.path().join("nb.ipynb"), notebook.to_string()).unwrap();
    let ctx = ctx(dir.path());
    let output = ReadTool
        .call(json!({"file_path": "nb.ipynb"}), &ctx)
        .await
        .unwrap();
    let text = output.text_content();
    assert!(
        text.contains("Notebook nb.ipynb (python, 2 cells)"),
        "{text}"
    );
    assert!(text.contains("<cell id=\"intro\" type=\"markdown\">\n# Analysis\n</cell>"));
    assert!(text.contains(
        "<cell id=\"calc\" type=\"code\">\n1 + 1\n</cell>\n<outputs cell=\"calc\">\n2\n"
    ));
    assert!(
        output
            .content
            .iter()
            .any(|part| matches!(part, ToolResultPart::Image { .. }))
    );
    assert_eq!(output.summary, "Read notebook (2 cells)");
    assert!(ctx.files.check_fresh(&dir.path().join("nb.ipynb")).is_ok());
}

#[tokio::test]
async fn pdfs_are_read_page_by_page() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("doc.pdf"),
        pdf(&["First page", "Second page", "Third page"]),
    )
    .unwrap();
    let ctx = ctx(dir.path());
    let all = ok_text(&ReadTool, &ctx, json!({"file_path": "doc.pdf"})).await;
    assert!(all.starts_with("PDF doc.pdf: pages 1-3"), "{all}");
    assert!(all.contains("--- Page 2 ---\nSecond page"), "{all}");
    let one = ok_text(
        &ReadTool,
        &ctx,
        json!({"file_path": "doc.pdf", "pages": "2"}),
    )
    .await;
    assert!(
        one.contains("Second page") && !one.contains("Third page"),
        "{one}"
    );
    assert!(one.contains("More pages follow"), "{one}");
    let err = err_text(
        &ReadTool,
        &ctx,
        json!({"file_path": "doc.pdf", "pages": "7"}),
    )
    .await;
    assert!(err.contains("has 3 pages"), "{err}");
    let err = err_text(
        &ReadTool,
        &ctx,
        json!({"file_path": "doc.pdf", "pages": "1-30"}),
    )
    .await;
    assert!(err.contains("at most 20 pages"), "{err}");
}
