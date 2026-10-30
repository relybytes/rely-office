// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

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

/// The width and height of a PNG, JPEG, GIF or BMP image in pixels, from its header; `None` for
/// any other format or a header cut short.
pub(crate) fn image_pixel_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let be16 = |at: usize| {
        Some(u32::from(u16::from_be_bytes(
            bytes.get(at..at + 2)?.try_into().ok()?,
        )))
    };
    let le16 = |at: usize| {
        Some(u32::from(u16::from_le_bytes(
            bytes.get(at..at + 2)?.try_into().ok()?,
        )))
    };
    let be32 = |at: usize| Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    let le32 = |at: usize| Some(i32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    let size = match sniff_image_type(bytes) {
        "image/png" if bytes.get(12..16) == Some(b"IHDR".as_slice()) => (be32(16)?, be32(20)?),
        "image/gif" => (le16(6)?, le16(8)?),
        "image/bmp" => (le32(18)?.unsigned_abs(), le32(22)?.unsigned_abs()),
        "image/jpeg" => {
            let mut at = 2;
            loop {
                if *bytes.get(at)? != 0xFF {
                    return None;
                }
                let marker = *bytes.get(at + 1)?;
                if marker == 0xFF {
                    at += 1;
                    continue;
                }
                // A start-of-frame marker, but not the huffman, arithmetic or JPEG-LS ones.
                if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
                    break (be16(at + 7)?, be16(at + 5)?);
                }
                at += 2 + usize::try_from(be16(at + 2)?).ok()?;
            }
        }
        _ => return None,
    };
    (size.0 > 0 && size.1 > 0).then_some(size)
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
    fn reads_the_pixel_size_of_common_images() {
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x01\x2C\0\0\0\xC8";
        assert_eq!(image_pixel_size(png), Some((300, 200)));
        assert_eq!(image_pixel_size(b"GIF89a\x0A\0\x05\0"), Some((10, 5)));
        let mut bmp = b"BM".to_vec();
        bmp.resize(18, 0);
        bmp.extend_from_slice(&4_i32.to_le_bytes());
        bmp.extend_from_slice(&(-3_i32).to_le_bytes());
        assert_eq!(image_pixel_size(&bmp), Some((4, 3)));
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xFF, 0xC4, 0, 2, 0xFF, 0xC0, 0, 11, 8, 0,
            50, 0, 80,
        ];
        assert_eq!(image_pixel_size(&jpeg), Some((80, 50)));
        assert_eq!(
            image_pixel_size(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0x00]),
            None
        );
        assert_eq!(
            image_pixel_size(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\0\0\0\0\x01"),
            None
        );
        assert_eq!(image_pixel_size(b"RIFF\0\0\0\0WEBPVP8 "), None);
        assert_eq!(image_pixel_size(b"GIF8"), None);
    }

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
