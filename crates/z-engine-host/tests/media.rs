//! Image payloads and PDF text extraction.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use z_engine_host::{HostError, pdf_text, read_image, read_pdf_base64};

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
async fn images_are_typed_by_magic_bytes_and_size_limited() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("pixel.png");
    std::fs::write(&png, PNG).unwrap();
    let (media_type, data) = read_image(&png, 5 * 1024 * 1024).await.unwrap();
    assert_eq!(media_type, "image/png");
    assert_eq!(STANDARD.decode(data).unwrap(), PNG);

    let misnamed = dir.path().join("pixel.jpg");
    std::fs::write(&misnamed, PNG).unwrap();
    assert_eq!(read_image(&misnamed, 1024).await.unwrap().0, "image/png");

    let fake = dir.path().join("fake.png");
    std::fs::write(&fake, "not an image").unwrap();
    assert!(matches!(
        read_image(&fake, 1024).await,
        Err(HostError::Invalid(_))
    ));
    assert!(matches!(
        read_image(&png, 10).await,
        Err(HostError::Invalid(_))
    ));
    let missing = read_image(&dir.path().join("none.png"), 1024)
        .await
        .unwrap_err();
    assert!(missing.is_not_found());
}

#[tokio::test]
async fn pdf_text_is_extracted_per_page_up_to_the_limit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.pdf");
    std::fs::write(&path, pdf(&["Hello PDF", "Second page"])).unwrap();

    let pages = pdf_text(&path, 10).await.unwrap();
    assert_eq!(pages.len(), 2);
    assert!(pages[0].contains("Hello PDF"), "{pages:?}");
    assert!(pages[1].contains("Second page"), "{pages:?}");
    assert_eq!(pdf_text(&path, 1).await.unwrap().len(), 1);

    let encoded = read_pdf_base64(&path, 1024 * 1024).await.unwrap();
    assert!(STANDARD.decode(encoded).unwrap().starts_with(b"%PDF-1.4"));
}

#[tokio::test]
async fn malformed_or_foreign_pdfs_are_invalid_not_panics() {
    let dir = tempfile::tempdir().unwrap();
    let broken = dir.path().join("broken.pdf");
    std::fs::write(
        &broken,
        b"%PDF-1.4\n1 0 obj << /Type /Catalog /Pages 9 0 R >>\ntrailer garbage",
    )
    .unwrap();
    assert!(matches!(
        pdf_text(&broken, 5).await,
        Err(HostError::Invalid(_))
    ));

    let text = dir.path().join("notes.pdf");
    std::fs::write(&text, "just text").unwrap();
    assert!(matches!(
        pdf_text(&text, 5).await,
        Err(HostError::Invalid(_))
    ));
    assert!(matches!(
        read_pdf_base64(&text, 1024).await,
        Err(HostError::Invalid(_))
    ));
}
