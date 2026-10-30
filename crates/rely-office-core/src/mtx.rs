// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! `MicroType` Express, the font compression of Embedded `OpenType` (the W3C member submission
//! of 2008): ten bytes of header, then three blocks each compressed with `LZCOMP`, which
//! together hold a TrueType font in the Compact Table Format.

mod ctf;
mod lzcomp;

/// The version of the format the submission describes.
const VERSION: u8 = 3;
const HEADER_SIZE: usize = 10;

/// A `MicroType` Express font as the TrueType font it compresses, no larger than `max_bytes`;
/// `None` when the bytes are not one or do not decompress within that size.
#[must_use]
pub fn decompress_mtx(bytes: &[u8], max_bytes: usize) -> Option<Vec<u8>> {
    let header = bytes.get(..HEADER_SIZE)?;
    let version = header[0];
    if !(1..=VERSION).contains(&version) {
        return None;
    }
    let offset = |at: usize| {
        usize::from(header[at]) << 16
            | usize::from(header[at + 1]) << 8
            | usize::from(header[at + 2])
    };
    let (second, third) = (offset(4), offset(7));
    if !(HEADER_SIZE <= second && second <= third && third <= bytes.len()) {
        return None;
    }
    let rest = lzcomp::decompress(&bytes[HEADER_SIZE..second], version, max_bytes)?;
    let push = lzcomp::decompress(&bytes[second..third], version, max_bytes)?;
    let code = lzcomp::decompress(&bytes[third..], version, max_bytes)?;
    ctf::to_truetype(&rest, &push, &code, max_bytes)
}

#[cfg(test)]
pub(crate) mod test_mtx;
#[cfg(test)]
mod tests;
