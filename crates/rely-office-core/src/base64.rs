// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Standard base64, for the pictures a page carries in `data:` addresses and the bytes a message
//! carries as text.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Bytes in standard base64 with padding.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for group in bytes.chunks(3) {
        let value = (u32::from(group[0]) << 16)
            | (u32::from(*group.get(1).unwrap_or(&0)) << 8)
            | u32::from(*group.get(2).unwrap_or(&0));
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= group.len() {
                text.push(char::from(ALPHABET[((value >> shift) & 0x3F) as usize]));
            } else {
                text.push('=');
            }
        }
    }
    text
}

/// The bytes of standard or URL-safe base64, whitespace and padding allowed; `None` for any
/// other character or a length no encoder writes.
#[must_use]
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len() / 4 * 3);
    let mut value = 0u32;
    let mut bits = 0u32;
    for character in text.bytes() {
        let digit = match character {
            b'A'..=b'Z' => character - b'A',
            b'a'..=b'z' => character - b'a' + 26,
            b'0'..=b'9' => character - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b' ' | b'\t' | b'\r' | b'\n' => continue,
            _ => return None,
        };
        value = (value << 6) | u32::from(digit);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push(((value >> bits) & 0xFF) as u8);
        }
    }
    (bits < 6).then_some(bytes)
}

/// The type and bytes of a `data:` address holding base64, `data:image/png;base64,iVBOR...`.
pub(crate) fn data_url(address: &str) -> Option<(&str, Vec<u8>)> {
    let rest = address.trim().strip_prefix("data:")?;
    let (header, payload) = rest.split_once(',')?;
    let content_type = header.strip_suffix(";base64")?;
    Some((content_type, decode(payload)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_every_tail_length() {
        for text in ["", "f", "fo", "foo", "foob", "fooba", "foobar"] {
            let encoded = encode(text.as_bytes());
            assert_eq!(decode(&encoded).unwrap(), text.as_bytes(), "{encoded}");
        }
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(decode("Zm9v\nYmFy").unwrap(), b"foobar");
        assert_eq!(decode("-_8").unwrap(), [0xFB, 0xFF]);
        assert_eq!(decode("Zm9v!"), None);
        assert_eq!(decode("Z"), None);
    }

    #[test]
    fn reads_a_data_address() {
        let (content_type, bytes) = data_url("data:image/png;base64,iVBORw0KGgo=").unwrap();
        assert_eq!(content_type, "image/png");
        assert_eq!(bytes, b"\x89PNG\r\n\x1a\n");
        assert_eq!(data_url("data:image/png,raw"), None);
        assert_eq!(data_url("https://relybytes.com/a.png"), None);
    }
}
