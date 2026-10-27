// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The device-independent bitmaps a metafile draws, as PNG an SVG can embed. The PNG is
//! written with stored deflate blocks, so no compressor is needed: an SVG of a preview is
//! transient, and its size is bounded by the bitmap's.

/// No bitmap is converted past this many pixels.
const MAX_PIXELS: u64 = 16 * 1024 * 1024;
/// The largest stored deflate block.
const STORED_BLOCK: usize = 65_535;

/// A bitmap ready for an SVG: its media type and bytes.
pub(super) struct Picture {
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
}

/// A DIB read: a JPEG or a PNG it carries as it is, or its pixels as RGBA, top row first.
pub(super) enum Decoded {
    Encoded(Picture),
    Pixels {
        width: usize,
        height: usize,
        rgba: Vec<u8>,
    },
}

impl Decoded {
    /// The picture an SVG embeds.
    pub(super) fn into_picture(self) -> Option<Picture> {
        match self {
            Self::Encoded(picture) => Some(picture),
            Self::Pixels {
                width,
                height,
                rgba,
            } => Some(Picture {
                media_type: "image/png",
                bytes: rgba_png(width, height, &rgba)?,
            }),
        }
    }
}

/// A DIB as a picture: `info` is its `BITMAPINFO` (header and colour table), `bits` its pixels.
/// A DIB that carries a JPEG or a PNG gives those bytes; RLE and unknown layouts give `None`.
pub(super) fn dib_picture(info: &[u8], bits: &[u8]) -> Option<Picture> {
    decode_dib(info, bits)?.into_picture()
}

/// A DIB's pixels, or the JPEG or PNG it carries.
#[expect(
    clippy::too_many_lines,
    reason = "the header, the colour table, the masks and the rows are read in one pass"
)]
pub(super) fn decode_dib(info: &[u8], bits: &[u8]) -> Option<Decoded> {
    let header_size = usize::try_from(u32_at(info, 0)?).ok()?;
    let (width, height, bit_count, compression, colours_used) = if header_size == 12 {
        (
            i64::from(u16_at(info, 4)?),
            i64::from(u16_at(info, 6)?),
            u16_at(info, 10)?,
            0,
            0,
        )
    } else if header_size >= 40 {
        (
            i64::from(i32_at(info, 4)?),
            i64::from(i32_at(info, 8)?),
            u16_at(info, 14)?,
            u32_at(info, 16)?,
            u32_at(info, 32)?,
        )
    } else {
        return None;
    };
    match compression {
        4 => {
            return Some(Decoded::Encoded(Picture {
                media_type: "image/jpeg",
                bytes: bits.to_vec(),
            }));
        }
        5 => {
            return Some(Decoded::Encoded(Picture {
                media_type: "image/png",
                bytes: bits.to_vec(),
            }));
        }
        0..=3 => {}
        _ => return None,
    }
    let top_down = height < 0;
    let (width, height) = (width.unsigned_abs(), height.unsigned_abs());
    if width == 0 || height == 0 || width.saturating_mul(height) > MAX_PIXELS {
        return None;
    }
    let (width, height) = (usize::try_from(width).ok()?, usize::try_from(height).ok()?);
    let palette_entry = if header_size == 12 { 3 } else { 4 };
    let masks_size = if compression == 3 && header_size == 40 {
        12
    } else {
        0
    };
    let palette_start = header_size + masks_size;
    let palette_count = if bit_count <= 8 {
        match usize::try_from(colours_used).ok()? {
            0 => 1 << bit_count,
            count => count.min(256),
        }
    } else {
        0
    };
    let palette: Vec<[u8; 3]> = (0..palette_count)
        .map(|index| {
            let at = palette_start + index * palette_entry;
            info.get(at..at + 3)
                .map_or([0, 0, 0], |rgb| [rgb[2], rgb[1], rgb[0]])
        })
        .collect();
    // The masks follow a 40-byte header, and sit at the same place inside a larger one.
    let masks = if compression == 3 {
        Some([u32_at(info, 40)?, u32_at(info, 44)?, u32_at(info, 48)?])
    } else {
        None
    };
    let expanded;
    let (bits, bit_count) = if matches!(compression, 1 | 2) {
        expanded = expand_rle(bits, width, height, compression == 2);
        (expanded.as_slice(), 8)
    } else {
        (bits, bit_count)
    };
    let stride = (width * usize::from(bit_count)).div_ceil(32) * 4;
    if bits.len() < stride.checked_mul(height)? {
        return None;
    }
    let mut pixels = Vec::with_capacity(width * height * 4);
    let mut any_alpha = false;
    for row in 0..height {
        let source_row = if top_down { row } else { height - 1 - row };
        let line = &bits[source_row * stride..(source_row + 1) * stride];
        for column in 0..width {
            let rgba = pixel(line, column, bit_count, &palette, masks)?;
            any_alpha |= rgba[3] != 0;
            pixels.extend_from_slice(&rgba);
        }
    }
    // Only a 32-bit bitmap has alpha, and one whose alpha is zero everywhere does not use it.
    if bit_count != 32 || !any_alpha {
        for alpha in pixels.iter_mut().skip(3).step_by(4) {
            *alpha = 255;
        }
    }
    Some(Decoded::Pixels {
        width,
        height,
        rgba: pixels,
    })
}

/// An image made transparent where a mask is black, as the `SRCPAINT` mask and `SRCAND`
/// image pair of a metafile draws it: the mask's brightness is the image's opacity, scaled to
/// the image when their sizes differ.
pub(super) fn masked(
    (width, height, image): (usize, usize, &[u8]),
    (mask_width, mask_height, mask): (usize, usize, &[u8]),
) -> Option<Picture> {
    if mask_width == 0 || mask_height == 0 || image.len() != width * height * 4 {
        return None;
    }
    let mut rgba = image.to_vec();
    for row in 0..height {
        let mask_row = row * mask_height / height.max(1);
        for column in 0..width {
            let mask_column = column * mask_width / width.max(1);
            let at = (mask_row * mask_width + mask_column) * 4;
            let brightness = mask.get(at..at + 3).map_or(255, |rgb| {
                (u16::from(rgb[0]) + u16::from(rgb[1]) + u16::from(rgb[2])) / 3
            });
            rgba[(row * width + column) * 4 + 3] = u8::try_from(brightness).unwrap_or(255);
        }
    }
    Some(Picture {
        media_type: "image/png",
        bytes: rgba_png(width, height, &rgba)?,
    })
}

/// A DIB stored whole, its bits right after its `BITMAPINFO`, as a WMF record carries it.
pub(super) fn packed_dib_picture(dib: &[u8]) -> Option<Picture> {
    let header_size = usize::try_from(u32_at(dib, 0)?).ok()?;
    let (bit_count, compression, colours_used, entry) = if header_size == 12 {
        (u16_at(dib, 10)?, 0, 0, 3)
    } else {
        (u16_at(dib, 14)?, u32_at(dib, 16)?, u32_at(dib, 32)?, 4)
    };
    let palette = if bit_count <= 8 {
        match usize::try_from(colours_used).ok()? {
            0 => 1 << bit_count,
            count => count.min(256),
        }
    } else {
        usize::try_from(colours_used).ok()?.min(256)
    };
    let masks = if compression == 3 && header_size == 40 {
        12
    } else {
        0
    };
    let bits_at = header_size + masks + palette * entry;
    dib_picture(dib.get(..bits_at)?, dib.get(bits_at..)?)
}

/// RLE8 or RLE4 pixels expanded to one palette index a byte, in the rows of an uncompressed
/// 8-bit DIB: a count and a value repeat the value, a zero opens an escape that ends the line,
/// ends the bitmap, moves by a delta or copies the indexes that follow.
fn expand_rle(bits: &[u8], width: usize, height: usize, four_bits: bool) -> Vec<u8> {
    let stride = width.div_ceil(4) * 4;
    let mut out = vec![0_u8; stride * height];
    let (mut x, mut y) = (0_usize, 0_usize);
    let mut at = 0;
    let mut put = |x: &mut usize, y: usize, index: u8| {
        if *x < width && y < height {
            out[y * stride + *x] = index;
        }
        *x += 1;
    };
    while at + 1 < bits.len() && y < height {
        let (count, value) = (usize::from(bits[at]), bits[at + 1]);
        at += 2;
        if count > 0 {
            for index in 0..count {
                let shown = if four_bits {
                    if index.is_multiple_of(2) {
                        value >> 4
                    } else {
                        value & 0x0F
                    }
                } else {
                    value
                };
                put(&mut x, y, shown);
            }
            continue;
        }
        match value {
            0 => {
                x = 0;
                y += 1;
            }
            1 => break,
            2 => {
                let (Some(dx), Some(dy)) = (bits.get(at), bits.get(at + 1)) else {
                    break;
                };
                x += usize::from(*dx);
                y += usize::from(*dy);
                at += 2;
            }
            literal => {
                let literal = usize::from(literal);
                let bytes = if four_bits {
                    literal.div_ceil(2)
                } else {
                    literal
                };
                let Some(run) = bits.get(at..at + bytes) else {
                    break;
                };
                for index in 0..literal {
                    let shown = if four_bits {
                        let byte = run[index / 2];
                        if index.is_multiple_of(2) {
                            byte >> 4
                        } else {
                            byte & 0x0F
                        }
                    } else {
                        run[index]
                    };
                    put(&mut x, y, shown);
                }
                at += bytes + bytes % 2;
            }
        }
    }
    out
}

fn pixel(
    line: &[u8],
    column: usize,
    bit_count: u16,
    palette: &[[u8; 3]],
    masks: Option<[u32; 3]>,
) -> Option<[u8; 4]> {
    let from_palette = |index: usize| {
        let [red, green, blue] = palette.get(index).copied().unwrap_or([0, 0, 0]);
        [red, green, blue, 0]
    };
    Some(match bit_count {
        1 => from_palette(usize::from((line.get(column / 8)? >> (7 - column % 8)) & 1)),
        4 => {
            let byte = line.get(column / 2)?;
            from_palette(usize::from(if column.is_multiple_of(2) {
                byte >> 4
            } else {
                byte & 0x0F
            }))
        }
        8 => from_palette(usize::from(*line.get(column)?)),
        16 => {
            let value = u32::from(u16::from_le_bytes([
                *line.get(column * 2)?,
                *line.get(column * 2 + 1)?,
            ]));
            let [red, green, blue] = masks.unwrap_or([0x7C00, 0x03E0, 0x001F]);
            [
                channel(value, red),
                channel(value, green),
                channel(value, blue),
                0,
            ]
        }
        24 => {
            let at = column * 3;
            [*line.get(at + 2)?, *line.get(at + 1)?, *line.get(at)?, 0]
        }
        32 => {
            let at = column * 4;
            let value = u32::from_le_bytes(line.get(at..at + 4)?.try_into().ok()?);
            match masks {
                Some([red, green, blue]) => [
                    channel(value, red),
                    channel(value, green),
                    channel(value, blue),
                    0,
                ],
                None => [line[at + 2], line[at + 1], line[at], line[at + 3]],
            }
        }
        _ => return None,
    })
}

/// A colour channel a bit mask selects, scaled to eight bits.
fn channel(value: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let bits = (mask >> shift).count_ones();
    let raw = (value & mask) >> shift;
    let maximum = (1_u64 << bits) - 1;
    u8::try_from(u64::from(raw) * 255 / maximum.max(1)).unwrap_or(255)
}

/// An RGBA image as a PNG, its pixel data in stored deflate blocks.
pub(super) fn rgba_png(width: usize, height: usize, pixels: &[u8]) -> Option<Vec<u8>> {
    let row = width.checked_mul(4)?;
    if width == 0 || height == 0 || pixels.len() != row.checked_mul(height)? {
        return None;
    }
    let mut raw = Vec::with_capacity((row + 1) * height);
    for line in pixels.chunks_exact(row) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(STORED_BLOCK).peekable();
    while let Some(block) = blocks.next() {
        zlib.push(u8::from(blocks.peek().is_none()));
        let length = u16::try_from(block.len()).ok()?;
        zlib.extend_from_slice(&length.to_le_bytes());
        zlib.extend_from_slice(&(!length).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&u32::try_from(width).ok()?.to_be_bytes());
    header.extend_from_slice(&u32::try_from(height).ok()?.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut png, *b"IHDR", &header);
    chunk(&mut png, *b"IDAT", &zlib);
    chunk(&mut png, *b"IEND", &[]);
    Some(png)
}

fn chunk(png: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    let length = u32::try_from(data.len()).unwrap_or(u32::MAX);
    png.extend_from_slice(&length.to_be_bytes());
    let start = png.len();
    png.extend_from_slice(&kind);
    png.extend_from_slice(data);
    let crc = crc32(&png[start..]);
    png.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut first, mut second) = (1_u32, 0_u32);
    for chunk in bytes.chunks(5_552) {
        for byte in chunk {
            first += u32::from(*byte);
            second += first;
        }
        first %= 65_521;
        second %= 65_521;
    }
    (second << 16) | first
}

/// Bytes in standard base64 with padding.
pub(super) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
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

pub(super) fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

pub(super) fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

pub(super) fn i32_at(bytes: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

pub(super) fn i16_at(bytes: &[u8], at: usize) -> Option<i16> {
    Some(i16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

#[cfg(test)]
mod tests;
