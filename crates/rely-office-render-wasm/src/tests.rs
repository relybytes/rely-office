// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use serde_json::{Value, json};

use super::*;

fn renderer() -> RendererHandle {
    let mut renderer = RendererHandle::new();
    for name in [
        "Carlito-Regular.ttf",
        "Carlito-Bold.ttf",
        "LiberationSans-Regular.ttf",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fonts")
            .join(name);
        renderer.add_font(std::fs::read(path).unwrap());
    }
    renderer
}

#[test]
fn reads_options_over_the_defaults() {
    let request = render_request(None, None).unwrap();
    assert_eq!(request, RenderRequest::default());
    assert_eq!(
        render_request(Some("null"), Some("")).unwrap(),
        RenderRequest::default()
    );

    let request = render_request(
        Some(r#"{"pdf":false,"thumbnailWidth":320,"pageImages":{"pixelsPerPoint":1.5,"maxPages":3}}"#),
        Some(r#"{"maxRows":10}"#),
    )
    .unwrap();
    assert!(!request.pdf);
    assert_eq!(request.thumbnail_width, Some(320));
    assert_eq!(
        request.page_images,
        Some(PageImages {
            pixels_per_point: 1.5,
            max_pages: 3
        })
    );
    assert_eq!(request.limits.max_rows, 10);
    assert_eq!(
        request.limits.max_text_chars,
        render_limits().max_text_chars
    );
}

#[test]
fn refuses_options_it_cannot_read() {
    assert_eq!(
        render_request(Some(r#"{"colour":"red"}"#), None)
            .unwrap_err()
            .code,
        "INVALID_OPTIONS"
    );
    assert_eq!(
        render_request(None, Some(r#"{"maxRows":"many"}"#))
            .unwrap_err()
            .code,
        "INVALID_LIMITS"
    );
}

#[test]
fn renders_and_hands_the_bytes_over_once() {
    let mut handle = renderer();
    assert_eq!(handle.font_count(), 3);
    let mut rendered = handle
        .render_document(
            b"Cliente,Importo\nRossi,10\n",
            Some("vendite.csv"),
            Some(r#"{"thumbnailWidth":120,"pageImages":{"pixelsPerPoint":0.25,"maxPages":5}}"#),
            None,
        )
        .unwrap();
    let summary: Value = serde_json::from_str(&rendered.json()).unwrap();
    assert_eq!(
        summary,
        json!({
            "format": "csv",
            "fidelity": "layout",
            "pageCount": 1,
            "truncated": false,
            "hasPdf": true,
            "hasThumbnail": true,
            "pageImageCount": 1
        })
    );
    assert!(rendered.take_pdf().unwrap().starts_with(b"%PDF-"));
    assert!(rendered.take_pdf().is_none());
    assert!(rendered.take_thumbnail().unwrap().starts_with(b"\x89PNG"));
    assert!(rendered.take_thumbnail().is_none());
    assert!(rendered.take_page_image(0).unwrap().starts_with(b"\x89PNG"));
    assert!(rendered.take_page_image(0).is_none());
    assert!(rendered.take_page_image(9).is_none());
}

#[test]
fn rebuilds_the_typesetter_when_a_font_is_added() {
    let mut renderer = renderer();
    assert_eq!(renderer.font_count(), 3);
    renderer.add_font(b"not a font".to_vec());
    assert_eq!(renderer.font_count(), 3);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fonts/Caladea-Regular.ttf");
    renderer.add_font(std::fs::read(path).unwrap());
    assert_eq!(renderer.font_count(), 4);
}

#[test]
fn gives_render_failures_their_codes() {
    let mut renderer = renderer();
    let failure = renderer
        .render_document(b"plain bytes", None, None, None)
        .err()
        .unwrap();
    assert_eq!(failure.code, "UNSUPPORTED_FORMAT");
    assert_eq!(failure.message, "unknown cannot be rendered");
    assert_eq!(
        render_failure(RenderError::Typeset("x".to_owned())).code,
        "TYPESET_FAILED"
    );
    assert_eq!(
        render_failure(RenderError::Export("x".to_owned())).code,
        "EXPORT_FAILED"
    );
    let damaged = renderer
        .render_document(b"PK\x03\x04broken", Some("a.docx"), None, None)
        .err()
        .unwrap();
    assert_ne!(damaged.code, "INTERNAL");
}
