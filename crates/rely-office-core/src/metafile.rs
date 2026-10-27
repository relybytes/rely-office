// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Windows metafiles drawn as SVG: the Enhanced Metafile of [MS-EMF] and the Windows Metafile
//! of [MS-WMF], the vector pictures a doc, a docx or a pptx carries when a drawing came from an
//! older program or was pasted from Excel. Their records are played on a device context, as
//! Windows plays them, and every line, shape, text and bitmap they draw becomes an SVG element.

mod bitmap;
mod context;
mod emf;
mod svg;
mod wmf;

use crate::document::sniff_image_type;

/// No metafile plays more records than this.
const MAX_RECORDS: usize = 500_000;
/// No SVG grows past this many bytes; a picture that would is drawn as far as it goes.
const MAX_SVG_BYTES: usize = 16 * 1024 * 1024;

/// An EMF or a WMF drawn as SVG; `None` for any other bytes, or a metafile with nothing a
/// picture can show.
#[must_use]
pub fn convert_metafile_to_svg(bytes: &[u8]) -> Option<Vec<u8>> {
    let svg = match sniff_image_type(bytes) {
        "image/emf" => emf::emf_svg(bytes),
        "image/wmf" => wmf::wmf_svg(bytes),
        _ if wmf::is_unplaceable_wmf(bytes) => wmf::wmf_svg(bytes),
        _ => None,
    }?;
    Some(svg.into_bytes())
}

#[cfg(test)]
mod tests;
