// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

#![no_main]

use std::sync::LazyLock;

use libfuzzer_sys::fuzz_target;
use rely_office_core::Limits;
use rely_office_render::{RenderError, RenderRequest, Renderer};

static RENDERER: LazyLock<Renderer> = LazyLock::new(|| {
    Renderer::new([
        include_bytes!("../../fonts/Carlito-Regular.ttf").to_vec(),
        include_bytes!("../../fonts/LiberationSerif-Regular.ttf").to_vec(),
    ])
});

// The source written from any readable document must typeset: a Typst error is a bug of the
// writer, which no input may reach.
fuzz_target!(|bytes: &[u8]| {
    let request = RenderRequest {
        pdf: false,
        limits: Limits {
            max_member_bytes: 2 * 1024 * 1024,
            max_text_chars: 5_000,
            max_rows: 30,
            max_columns: 10,
            max_images: 2,
            max_image_bytes: 128 * 1024,
            max_total_image_bytes: 256 * 1024,
            ..Limits::default()
        },
        ..RenderRequest::default()
    };
    for name in [None, Some("a.csv")] {
        if let Err(RenderError::Typeset(message)) = RENDERER.render(bytes, name, &request) {
            panic!("the generated source did not typeset: {message}");
        }
    }
});
