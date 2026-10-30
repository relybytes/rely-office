// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;

const SLIDES: &str = r#"{"widthEmu":9144000,"heightEmu":6858000,"slides":[{"background":{"type":"solid","colour":"FFEEDD"},
"shapes":[{"frame":{"x":0,"y":0,"width":914400,"height":457200},"rotation":0,"geometry":{"type":"rectangle"},
"fill":{"type":"picture","media":0},"line":null,"content":{"type":"text","paragraphs":[{"level":0,"align":"left","bullet":null,
"marginLeftEmu":0,"indentEmu":0,"lineSpacingPercent":100,"spaceBefore":0,"spaceAfter":0,"runs":[{"text":"Ciao","size":1800,
"bold":false,"italic":false,"underline":false,"strike":false,"colour":"112233","family":"Calibri","lineBreak":false}]}],
"anchor":"top","insetsEmu":[0,0,0,0],"wrap":true}}],
"notes":{"paragraphs":[],"anchor":"top","insetsEmu":[0,0,0,0],"wrap":true},
"hidden":true,"transition":{"effect":"fade","advanceOnClick":true},
"animations":[{"shape":0,"class":"entrance","preset":10,"presetSubtype":0,"trigger":"onClick","delayMs":0,"durationMs":500}]}]}"#;

#[test]
fn writes_slides_from_json_and_reads_them_back() {
    let png = b"\x89PNG\r\n\x1a\nimmagine".to_vec();
    let bytes = pptx_from_json(
        SLIDES,
        vec![Media {
            content_type: "image/png",
            bytes: png,
        }],
    )
    .unwrap();
    let handle = presentation(&bytes, None).unwrap();
    let read: serde_json::Value = serde_json::from_str(&handle.json()).unwrap();
    let slide = &read["slides"][0];
    assert_eq!(slide["background"]["colour"], "FFEEDD");
    assert_eq!(slide["hidden"], true);
    assert_eq!(slide["transition"]["effect"], "fade");
    assert_eq!(slide["animations"][0]["preset"], 10);
    assert_eq!(slide["shapes"][0]["fill"]["type"], "picture");
    assert_eq!(
        slide["shapes"][0]["content"]["paragraphs"][0]["runs"][0]["text"],
        "Ciao"
    );
    assert_eq!(read["media"][0]["contentType"], "image/png");
}

#[test]
fn refuses_json_that_is_not_a_presentation() {
    let failure = pptx_from_json(r#"{"slides":[]}"#, Vec::new()).unwrap_err();
    assert_eq!(failure.code, "INVALID_PRESENTATION");
    let failure = pptx_from_json(
        r#"{"widthEmu":1,"heightEmu":1,"slides":[],"extra":1}"#,
        Vec::new(),
    )
    .unwrap_err();
    assert_eq!(
        failure.code, "INVALID_PRESENTATION",
        "unknown fields are refused"
    );
}
