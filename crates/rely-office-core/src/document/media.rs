// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

/// The media type of an image, read from its first bytes; `application/octet-stream` when they
/// match no image format a document embeds.
#[must_use]
pub fn sniff_image_type(bytes: &[u8]) -> &'static str {
    const SIGNATURES: [(&[u8], &str); 7] = [
        (b"\x89PNG\r\n\x1a\n", "image/png"),
        (b"\xFF\xD8\xFF", "image/jpeg"),
        (b"GIF87a", "image/gif"),
        (b"GIF89a", "image/gif"),
        (b"BM", "image/bmp"),
        (b"II*\0", "image/tiff"),
        (b"MM\0*", "image/tiff"),
    ];
    if let Some((_, content_type)) = SIGNATURES
        .iter()
        .find(|(signature, _)| bytes.starts_with(signature))
    {
        return content_type;
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return "image/webp";
    }
    if bytes.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A]) {
        return "image/wmf";
    }
    if bytes.starts_with(&[1, 0, 0, 0]) && bytes.get(40..44) == Some(b" EMF".as_slice()) {
        return "image/emf";
    }
    if looks_like_svg(bytes) {
        return "image/svg+xml";
    }
    "application/octet-stream"
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let head = String::from_utf8_lossy(head);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    (head.starts_with("<svg") || head.starts_with("<?xml")) && head.contains("<svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_the_image_formats_documents_embed() {
        assert_eq!(sniff_image_type(b"\x89PNG\r\n\x1a\nrest"), "image/png");
        assert_eq!(sniff_image_type(b"\xFF\xD8\xFF\xE0"), "image/jpeg");
        assert_eq!(sniff_image_type(b"GIF89a.."), "image/gif");
        assert_eq!(sniff_image_type(b"BM...."), "image/bmp");
        assert_eq!(sniff_image_type(b"II*\0...."), "image/tiff");
        assert_eq!(sniff_image_type(b"RIFF\0\0\0\0WEBPVP8 "), "image/webp");
        assert_eq!(sniff_image_type(&[0xD7, 0xCD, 0xC6, 0x9A, 0]), "image/wmf");
        let mut emf = vec![1, 0, 0, 0];
        emf.resize(40, 0);
        emf.extend_from_slice(b" EMF");
        assert_eq!(sniff_image_type(&emf), "image/emf");
        assert_eq!(
            sniff_image_type(b"<?xml version=\"1.0\"?>\n<svg xmlns=\"x\"/>"),
            "image/svg+xml"
        );
        assert_eq!(sniff_image_type(b"<svg/>"), "image/svg+xml");
    }

    #[test]
    fn answers_octet_stream_for_anything_else() {
        assert_eq!(sniff_image_type(b""), "application/octet-stream");
        assert_eq!(sniff_image_type(b"RIFF"), "application/octet-stream");
        assert_eq!(
            sniff_image_type(b"<?xml?><html/>"),
            "application/octet-stream"
        );
        assert_eq!(sniff_image_type(&[1, 0, 0, 0]), "application/octet-stream");
    }
}
