// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

/// Bounds applied while reading a document, so a crafted file cannot exhaust memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The largest input accepted, in bytes.
    pub max_input_bytes: u64,
    /// The largest single archive member decompressed, in bytes: the defence against zip bombs.
    pub max_member_bytes: u64,
    /// The most characters of text returned by [`crate::extract_text`].
    pub max_text_chars: usize,
    /// The most rows of each sheet returned by [`crate::read_workbook`].
    pub max_rows: usize,
    /// The most columns of each sheet returned by [`crate::read_workbook`].
    pub max_columns: usize,
    /// The largest embedded image returned by [`crate::read_document_tree`], in bytes; a larger one
    /// is reported, not returned.
    pub max_image_bytes: u64,
    /// The most embedded images returned by [`crate::read_document_tree`].
    pub max_images: usize,
    /// The most bytes of all the images returned by [`crate::read_document_tree`] together.
    pub max_total_image_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 50 * 1024 * 1024,
            max_member_bytes: 128 * 1024 * 1024,
            max_text_chars: 1_000_000,
            max_rows: 1_000,
            max_columns: 60,
            max_image_bytes: 10 * 1024 * 1024,
            max_images: 200,
            max_total_image_bytes: 100 * 1024 * 1024,
        }
    }
}
