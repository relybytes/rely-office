// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The line height of each font, so the lines of a paragraph are as tall as an office suite
//! makes them: Word's single spacing is the font's Windows ascent plus descent, or its
//! typographic line with the line gap when that is taller, while Typst sets a line between its
//! ascender and descender, and the leading makes up the difference.

use std::collections::HashMap;

use typst::layout::Abs;
use typst::text::{Font, FontStyle, FontVariations, FontWeight};

/// How tall a line of one font is, in ems.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LineMetrics {
    /// From Typst's top edge to its bottom edge: ascender less descender.
    pub box_em: f64,
    /// A single-spaced line of an office suite.
    pub single_em: f64,
}

/// What a font Typst does not hold is given: a box of one em and single lines of 1.15 em.
const FALLBACK: LineMetrics = LineMetrics {
    box_em: 1.0,
    single_em: 1.15,
};

/// The line metrics of each family the renderer holds, by lower-case name.
#[derive(Debug, Clone, Default)]
pub(crate) struct FontLines {
    families: HashMap<String, LineMetrics>,
}

impl FontLines {
    /// The metrics of each family, read from its regular face, else from its first one.
    pub(crate) fn from_fonts(fonts: &[Font]) -> Self {
        let mut families = HashMap::new();
        for font in fonts {
            let family = font.info().family.to_lowercase();
            let regular = font.info().variant.style == FontStyle::Normal
                && font.info().variant.weight == FontWeight::REGULAR;
            if regular || !families.contains_key(&family) {
                families.insert(family, metrics_of(font));
            }
        }
        Self { families }
    }

    /// The metrics of the first of `families` the renderer holds.
    pub(crate) fn line(&self, families: &[String]) -> LineMetrics {
        families
            .iter()
            .find_map(|family| self.families.get(&family.to_lowercase()))
            .copied()
            .unwrap_or(FALLBACK)
    }
}

fn metrics_of(font: &Font) -> LineMetrics {
    let instance = font.clone().instantiate(
        font.info().variant,
        Abs::pt(12.0),
        &FontVariations::default(),
    );
    let metrics = instance.metrics();
    let box_em = metrics.ascender.get() - metrics.descender.get();
    let face = instance.ttf();
    let units_per_em = f64::from(face.units_per_em().max(1));
    // Word takes the larger of the Windows metrics and the typographic line with its gap.
    let typographic = (f64::from(face.ascender()) - f64::from(face.descender())
        + f64::from(face.line_gap()))
        / units_per_em;
    let windows = face.tables().os2.map_or(0.0, |os2| {
        (f64::from(os2.windows_ascender()) + f64::from(os2.windows_descender()).abs())
            / units_per_em
    });
    let single_em = typographic.max(windows);
    LineMetrics {
        box_em: if box_em > 0.0 {
            box_em
        } else {
            FALLBACK.box_em
        },
        single_em: if single_em > 0.0 {
            single_em
        } else {
            FALLBACK.single_em
        },
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use typst::foundations::Bytes;

    use super::*;

    fn font(name: &str) -> Font {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fonts")
            .join(name);
        Font::new(Bytes::new(std::fs::read(path).unwrap()), 0).unwrap()
    }

    #[test]
    fn reads_the_single_line_of_word_from_the_windows_metrics() {
        let lines = FontLines::from_fonts(&[
            font("Carlito-Bold.ttf"),
            font("Carlito-Regular.ttf"),
            font("LiberationSerif-Regular.ttf"),
        ]);
        let carlito = lines.line(&["Carlito".to_owned()]);
        assert!((carlito.single_em - 1.22).abs() < 0.01, "{carlito:?}");
        assert!(carlito.box_em > 0.9 && carlito.box_em < carlito.single_em);
        let serif = lines.line(&["Times New Roman".to_owned(), "liberation serif".to_owned()]);
        assert!((serif.single_em - 1.15).abs() < 0.01, "{serif:?}");
        assert_eq!(lines.line(&["Aptos".to_owned()]), FALLBACK);
        assert_eq!(FontLines::default().line(&[]), FALLBACK);
    }
}
