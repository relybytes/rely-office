// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn run(text: &str) -> TextRun {
    TextRun {
        x: 1.0,
        y: 2.0,
        text: text.to_owned(),
        family: "A&B".to_owned(),
        size: 10.0,
        bold: false,
        italic: false,
        underline: true,
        strike: false,
        colour: "112233".to_owned(),
        anchor: "end",
        baseline_shift: 8.0,
        angle: 0.0,
        width: None,
        background: Some(("FFFF00".to_owned(), [0.0, 0.0, 4.0, 2.0])),
    }
}

#[test]
fn writes_paths_text_and_images() {
    let mut canvas = Canvas::default();
    let paint = Paint {
        fill: None,
        even_odd: true,
        stroke: Some("000000".to_owned()),
        stroke_width: 0.0,
        dashes: Vec::new(),
    };
    canvas.path("M0 0L1 1", &paint);
    canvas.path("", &paint);
    canvas.path(
        "M0 0",
        &Paint {
            stroke: None,
            ..paint.clone()
        },
    );
    canvas.text(&run("<a> & \"b\"\t"));
    canvas.text(&TextRun {
        underline: false,
        strike: true,
        background: None,
        ..run("  ")
    });
    canvas.text(&TextRun {
        background: None,
        ..run("")
    });
    canvas.image("image/png", b"x", [10.0, 10.0, -4.0, 2.0]);
    canvas.image("image/png", b"x", [0.0, 0.0, 0.0, 2.0]);
    let svg = canvas.finish([0.0, 0.0, 20.0, 10.0], (0.0, 5.0)).unwrap();
    assert!(svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1pt" height="5pt" viewBox="0 0 20 10" preserveAspectRatio="none">"#));
    assert!(svg.contains(r##"<path d="M0 0L1 1" fill="none" stroke="#000000" stroke-width="0.01" stroke-linejoin="round"/>"##));
    assert_eq!(
        svg.matches("<path").count(),
        1,
        "an empty path and an unpainted one are left out"
    );
    assert!(svg.contains(r##"<rect x="0" y="0" width="4" height="2" fill="#FFFF00"/>"##));
    assert!(svg.contains(r##"<text x="1" y="10" font-family="A&amp;B" font-size="10" fill="#112233" text-anchor="end" text-decoration="underline" xml:space="preserve">&lt;a&gt; &amp; &quot;b&quot; </text>"##));
    assert_eq!(svg.matches("<text").count(), 1, "blank text is left out");
    assert!(svg.contains(r#"<image x="6" y="10" width="4" height="2" preserveAspectRatio="none" transform="translate(8 11) scale(-1 1) translate(-8 -11)" href="data:image/png;base64,eA=="/>"#));
    assert_eq!(svg.matches("<image").count(), 1);
    assert_eq!(
        Canvas::default().finish([0.0, 0.0, 1.0, 1.0], (1.0, 1.0)),
        None
    );
    let mut drawn = Canvas::default();
    drawn.path("M0 0L1 1", &paint);
    assert_eq!(
        drawn.finish([0.0, 0.0, 0.0, 1.0], (1.0, 1.0)),
        None,
        "no view"
    );
}

#[test]
fn clips_and_stops_at_its_size_cap() {
    let paint = Paint {
        fill: Some("FF0000".to_owned()),
        even_odd: false,
        stroke: None,
        stroke_width: 1.0,
        dashes: vec![2.0, 1.0],
    };
    let mut canvas = Canvas::default();
    canvas.set_clip(&["M0 0H1V1Z".to_owned(), "M0 0H2V2Z".to_owned()]);
    canvas.set_clip(&["M0 0H1V1Z".to_owned(), "M0 0H2V2Z".to_owned()]);
    canvas.path("M0 0L1 1", &paint);
    canvas.set_clip(&[]);
    canvas.set_clip(&[]);
    canvas.path("M0 0L2 2", &paint);
    let svg = canvas.finish([0.0, 0.0, 2.0, 2.0], (2.0, 2.0)).unwrap();
    assert!(svg.contains(concat!(
        r#"<clipPath id="clip1"><path d="M0 0H1V1Z"/></clipPath>"#,
        r#"<clipPath id="clip2" clip-path="url(#clip1)"><path d="M0 0H2V2Z"/></clipPath>"#,
        r##"<g clip-path="url(#clip2)"><path d="M0 0L1 1" fill="#FF0000"/></g><path d="M0 0L2 2""##
    )));
    let mut open = Canvas::default();
    open.set_clip(&["M0 0H1V1Z".to_owned()]);
    open.path("M0 0L1 1", &paint);
    assert!(
        open.finish([0.0, 0.0, 1.0, 1.0], (1.0, 1.0))
            .unwrap()
            .ends_with("</g></svg>")
    );
    let mut full = Canvas::default();
    let long = "M0 0".repeat(MAX_SVG_BYTES / 4);
    full.path(&long, &paint);
    full.path("M0 0L1 1", &paint);
    assert_eq!(
        full.finish([0.0, 0.0, 1.0, 1.0], (1.0, 1.0)),
        None,
        "nothing fit"
    );
}

#[test]
fn writes_numbers_and_escapes_text() {
    assert_eq!(number(1.0), "1");
    assert_eq!(number(1.256), "1.26");
    assert_eq!(number(-0.001), "0");
    assert_eq!(number(f64::NAN), "0");
    assert_eq!(number(10.50), "10.5");
    assert_eq!(escape("a\u{1}b\u{FFFF}c\nd"), "abc d");
}
