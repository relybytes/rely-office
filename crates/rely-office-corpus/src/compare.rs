// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! How far one page image is from another.
//!
//! Both pages are turned to grey, cut to the area they share and averaged over blocks of
//! [`BLOCK`] pixels, so a line set a pixel or two lower is not a different page while a missing
//! paragraph, table or image is. The distance is the mean absolute difference of the blocks, from
//! 0 (the same) to 1 (black against white everywhere).

/// The side of the averaging block, in pixels: at 72 dpi, about a line of 11 pt text.
pub(crate) const BLOCK: usize = 8;
/// Pages whose sides differ by more than this share are different pages, whatever they show.
const SIZE_TOLERANCE: f64 = 0.02;

/// A page in grey, 0 black to 255 white.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GreyPage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl GreyPage {
    /// Decode a PNG, laid over white where it is transparent.
    pub(crate) fn from_png(bytes: &[u8]) -> Result<Self, String> {
        let pixmap = tiny_skia::Pixmap::decode_png(bytes).map_err(|error| error.to_string())?;
        let pixels = pixmap
            .pixels()
            .iter()
            .map(|pixel| {
                // Premultiplied colour over white: colour + (1 - alpha) * 255.
                let background = 255 - u32::from(pixel.alpha());
                let red = u32::from(pixel.red()) + background;
                let green = u32::from(pixel.green()) + background;
                let blue = u32::from(pixel.blue()) + background;
                u8::try_from((red * 299 + green * 587 + blue * 114) / 1000).unwrap_or(u8::MAX)
            })
            .collect();
        Ok(Self {
            width: pixmap.width() as usize,
            height: pixmap.height() as usize,
            pixels,
        })
    }

    /// The mean grey of each block, row by row, over the first `width` by `height` pixels.
    fn blocks(&self, width: usize, height: usize) -> Vec<f64> {
        let columns = width.div_ceil(BLOCK);
        let rows = height.div_ceil(BLOCK);
        let mut blocks = Vec::with_capacity(columns * rows);
        for row in 0..rows {
            for column in 0..columns {
                let mut sum = 0u64;
                let mut count = 0u64;
                for y in row * BLOCK..((row + 1) * BLOCK).min(height) {
                    for x in column * BLOCK..((column + 1) * BLOCK).min(width) {
                        sum += u64::from(self.pixels[y * self.width + x]);
                        count += 1;
                    }
                }
                #[allow(clippy::cast_precision_loss)]
                blocks.push(sum as f64 / count.max(1) as f64);
            }
        }
        blocks
    }
}

/// The distance between two pages, or `None` when their sizes say they are different pages.
pub(crate) fn page_distance(ours: &GreyPage, theirs: &GreyPage) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)]
    let close = |a: usize, b: usize| a.abs_diff(b) as f64 <= SIZE_TOLERANCE * a.max(b) as f64;
    if !close(ours.width, theirs.width) || !close(ours.height, theirs.height) {
        return None;
    }
    let width = ours.width.min(theirs.width);
    let height = ours.height.min(theirs.height);
    if width == 0 || height == 0 {
        return Some(0.0);
    }
    let ours = ours.blocks(width, height);
    let theirs = theirs.blocks(width, height);
    let total: f64 = ours.iter().zip(&theirs).map(|(a, b)| (a - b).abs()).sum();
    #[allow(clippy::cast_precision_loss)]
    Some(total / ours.len() as f64 / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(width: usize, height: usize, grey: impl Fn(usize, usize) -> u8) -> GreyPage {
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                pixels.push(grey(x, y));
            }
        }
        GreyPage {
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn finds_no_distance_between_equal_pages() {
        let text = page(64, 64, |x, y| if (x + y) % 3 == 0 { 0 } else { 255 });
        assert_eq!(page_distance(&text, &text), Some(0.0));
    }

    #[test]
    fn finds_the_whole_distance_between_black_and_white() {
        let white = page(16, 16, |_, _| 255);
        let black = page(16, 16, |_, _| 0);
        assert_eq!(page_distance(&white, &black), Some(1.0));
    }

    #[test]
    fn forgives_a_shift_of_a_pixel_more_than_a_missing_block() {
        let line = |offset: usize| page(80, 80, move |_, y| if y == 20 + offset { 0 } else { 255 });
        let shifted = page_distance(&line(0), &line(1)).unwrap();
        let missing = page_distance(&line(0), &page(80, 80, |_, _| 255)).unwrap();
        assert!(shifted < missing / 4.0, "{shifted} against {missing}");
    }

    #[test]
    fn tells_pages_of_another_size_apart() {
        let a4 = page(595, 842, |_, _| 255);
        let near = page(596, 841, |_, _| 255);
        let landscape = page(842, 595, |_, _| 255);
        assert_eq!(page_distance(&a4, &near), Some(0.0));
        assert_eq!(page_distance(&a4, &landscape), None);
        assert_eq!(
            page_distance(&page(0, 0, |_, _| 0), &page(0, 0, |_, _| 0)),
            Some(0.0)
        );
    }

    #[test]
    fn decodes_a_png_over_white() {
        let mut pixmap = tiny_skia::Pixmap::new(2, 1).unwrap();
        pixmap.pixels_mut()[0] = tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 0, 255).unwrap();
        let decoded = GreyPage::from_png(&pixmap.encode_png().unwrap()).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 1));
        assert_eq!(decoded.pixels, vec![0, 255]);
        assert!(GreyPage::from_png(b"not a png").is_err());
    }
}
