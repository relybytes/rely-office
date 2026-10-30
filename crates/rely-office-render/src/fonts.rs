// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! Which font sets a document's text: its own typeface when the renderer holds it, otherwise a
//! metric-compatible substitute, so lines break where the office suite breaks them.

use std::cell::RefCell;
use std::collections::HashMap;

use rely_office_core::{DocumentFonts, FontGeneric, font_substitute};

pub(crate) const SANS: &str = "Carlito";
pub(crate) const SERIF: &str = "Caladea";
pub(crate) const ARIAL: &str = "Liberation Sans";
pub(crate) const TIMES: &str = "Liberation Serif";
pub(crate) const MONO: &str = "Liberation Mono";

/// The families the npm package ships, all under the SIL Open Font License 1.1: Carlito and
/// Caladea match Calibri and Cambria, the Liberation fonts match Arial, Times New Roman and
/// Courier New.
#[must_use]
pub fn bundled_font_families() -> &'static [&'static str] {
    &[SANS, SERIF, ARIAL, TIMES, MONO]
}

thread_local! {
    /// What the font table of the document being set says of its families: kind and other name.
    static FAMILIES: RefCell<HashMap<String, (FontGeneric, Option<String>)>> =
        RefCell::new(HashMap::new());
}

/// The font table of the document being set, for the families its text names; it applies until
/// the scope is dropped.
pub(crate) struct FamiliesScope;

impl FamiliesScope {
    pub(crate) fn enter(fonts: &DocumentFonts) -> Self {
        FAMILIES.with(|families| {
            *families.borrow_mut() = fonts
                .families
                .iter()
                .map(|font| {
                    (
                        font.name.trim().to_ascii_lowercase(),
                        (font.generic, font.alt_name.clone()),
                    )
                })
                .collect();
        });
        Self
    }
}

impl Drop for FamiliesScope {
    fn drop(&mut self) {
        FAMILIES.with(|families| families.borrow_mut().clear());
    }
}

/// The substitute with the same metrics as `family`, or the nearest of the kind its document
/// says it is.
fn substitute(family: &str) -> &'static str {
    let generic = FAMILIES.with(|families| {
        families
            .borrow()
            .get(&family.trim().to_ascii_lowercase())
            .map_or(FontGeneric::Auto, |(generic, _)| *generic)
    });
    font_substitute(family, generic)
}

/// The font list of body text: the document's family, the other name its document gives it,
/// its substitute, then the defaults.
pub(crate) fn body_fonts(family: Option<&str>) -> Vec<String> {
    let mut fonts = Vec::new();
    if let Some(family) = family.map(str::trim).filter(|family| !family.is_empty()) {
        fonts.push(family.to_owned());
        let alt = FAMILIES.with(|families| {
            families
                .borrow()
                .get(&family.to_ascii_lowercase())
                .and_then(|(_, alt)| alt.clone())
        });
        if let Some(alt) = alt.filter(|alt| !alt.trim().is_empty()) {
            fonts.push(alt.trim().to_owned());
        }
        fonts.push(substitute(family).to_owned());
    }
    for fallback in [SANS, ARIAL] {
        if !fonts.iter().any(|font| font == fallback) {
            fonts.push(fallback.to_owned());
        }
    }
    fonts.dedup();
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_the_office_fonts_with_metric_compatible_ones() {
        assert_eq!(substitute("Calibri"), SANS);
        assert_eq!(substitute(" cambria "), SERIF);
        assert_eq!(substitute("Arial"), ARIAL);
        assert_eq!(substitute("Times New Roman"), TIMES);
        assert_eq!(substitute("Courier New"), MONO);
        assert_eq!(substitute("Fira Code"), MONO);
        assert_eq!(substitute("Noto Serif"), TIMES);
        assert_eq!(substitute("Noto Sans"), SANS);
        assert_eq!(substitute("Comic Sans MS"), SANS);
    }

    #[test]
    fn lists_the_document_font_first_and_the_defaults_last() {
        assert_eq!(body_fonts(None), vec![SANS, ARIAL]);
        assert_eq!(body_fonts(Some("  ")), vec![SANS, ARIAL]);
        assert_eq!(body_fonts(Some("Calibri")), vec!["Calibri", SANS, ARIAL]);
        assert_eq!(
            body_fonts(Some("Times New Roman")),
            vec!["Times New Roman", TIMES, SANS, ARIAL]
        );
        assert_eq!(body_fonts(Some("Carlito")), vec![SANS, ARIAL]);
    }

    #[test]
    fn follows_the_font_table_of_the_document_being_set() {
        let fonts = DocumentFonts {
            families: vec![rely_office_core::DocumentFont {
                name: "Mystery Book".to_owned(),
                alt_name: Some("Mystery".to_owned()),
                generic: FontGeneric::Roman,
                ..rely_office_core::DocumentFont::default()
            }],
            files: Vec::new(),
        };
        {
            let _scope = FamiliesScope::enter(&fonts);
            assert_eq!(
                body_fonts(Some("Mystery Book")),
                vec!["Mystery Book", "Mystery", TIMES, SANS, ARIAL]
            );
            assert_eq!(substitute("Aptos"), SANS);
        }
        assert_eq!(
            body_fonts(Some("Mystery Book")),
            vec!["Mystery Book", SANS, ARIAL]
        );
    }
}
