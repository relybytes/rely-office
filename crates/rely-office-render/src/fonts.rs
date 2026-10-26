// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Which font sets a document's text: its own typeface when the renderer holds it, otherwise a
//! metric-compatible substitute, so lines break where the office suite breaks them.

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

/// The substitute with the same metrics as `family`, or the nearest in kind.
fn substitute(family: &str) -> &'static str {
    let family = family.trim().to_ascii_lowercase();
    match family.as_str() {
        "calibri" | "calibri light" | "aptos" | "aptos display" | "candara" | "corbel" => SANS,
        "cambria" | "cambria math" | "constantia" => SERIF,
        "arial" | "helvetica" | "arial narrow" | "verdana" | "tahoma" | "segoe ui"
        | "trebuchet ms" | "liberation sans" | "arimo" | "dejavu sans" => ARIAL,
        "times new roman" | "times" | "georgia" | "garamond" | "book antiqua" | "palatino"
        | "palatino linotype" | "liberation serif" | "tinos" | "dejavu serif" => TIMES,
        "courier new" | "courier" | "consolas" | "lucida console" | "liberation mono"
        | "cousine" | "dejavu sans mono" => MONO,
        family if family.contains("mono") || family.contains("code") => MONO,
        family if family.contains("serif") && !family.contains("sans") => TIMES,
        _ => SANS,
    }
}

/// The font list of body text: the document's family, its substitute, then the defaults.
pub(crate) fn body_fonts(family: Option<&str>) -> Vec<String> {
    let mut fonts = Vec::new();
    if let Some(family) = family.map(str::trim).filter(|family| !family.is_empty()) {
        fonts.push(family.to_owned());
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
}
