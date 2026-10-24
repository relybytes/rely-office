// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The styles part (`word/styles.xml`): what a paragraph style means for its paragraphs, and
//! the formatting a character style gives its runs.

use std::collections::HashMap;

use crate::OfficeError;
use crate::xml_tree::{Element, Namespace, parse_tree};

const W: Namespace = Namespace::Word;
/// How many `w:basedOn` steps are followed; a longer chain is a loop or a crafted file.
const MAX_STYLE_CHAIN: usize = 16;
/// Outline level 9 means body text.
const BODY_TEXT_OUTLINE_LEVEL: u8 = 9;

/// What a paragraph is, as its style names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParagraphRole {
    Normal,
    /// A heading of this level, 1 to 9.
    Heading(u8),
    Quote,
    Code,
    /// A paragraph drawn as a rule, as `LibreOffice` exports an HTML `hr`.
    HorizontalRule,
}

/// Name fragments of each role: the English built-in names, which Word stores whatever the
/// language of its interface, and the Italian ones `LibreOffice` writes from an Italian interface.
const QUOTE_NAMES: [&str; 3] = ["quot", "block text", "citazione"];
const CODE_NAMES: [&str; 4] = ["code", "preformatted", "codice", "preformattato"];
const RULE_NAMES: [&str; 2] = ["horizontal line", "linea orizzontale"];

/// A reference to a numbering definition: `w:numPr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NumberingReference {
    pub num_id: u32,
    pub level: Option<u8>,
}

/// A paragraph style with its `w:basedOn` chain resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParagraphStyle {
    pub role: ParagraphRole,
    /// The outline level, 0 for a top heading; `None` for body text.
    pub outline_level: Option<u8>,
    pub numbering: Option<NumberingReference>,
}

/// Run formatting where every property may be unset, so a run's own properties can override a
/// style's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RunFormat {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub code: Option<bool>,
    pub superscript: Option<bool>,
    pub subscript: Option<bool>,
    pub hidden: Option<bool>,
}

impl RunFormat {
    /// Read `w:rPr`; the old formatting a tracked change keeps (`w:rPrChange`) is not read.
    pub(crate) fn parse(properties: &Element) -> Self {
        let mut format = Self::default();
        for property in properties
            .elements()
            .filter(|element| element.namespace == W)
        {
            match property.name.as_str() {
                "b" => format.bold = Some(on_off(property)),
                "i" => format.italic = Some(on_off(property)),
                "u" => {
                    format.underline = Some(
                        property
                            .attribute(W, "val")
                            .is_none_or(|value| value != "none"),
                    );
                }
                "strike" | "dstrike" => format.strike = Some(on_off(property)),
                "vanish" => format.hidden = Some(on_off(property)),
                "vertAlign" => {
                    let value = property.attribute(W, "val").unwrap_or_default();
                    format.superscript = Some(value == "superscript");
                    format.subscript = Some(value == "subscript");
                }
                "rFonts" => {
                    let fonts = ["ascii", "hAnsi", "cs"]
                        .iter()
                        .filter_map(|name| property.attribute(W, name))
                        .collect::<Vec<_>>();
                    if !fonts.is_empty() {
                        format.code = Some(fonts.iter().any(|font| is_monospace_font(font)));
                    }
                }
                _ => {}
            }
        }
        format
    }

    /// This format with every property `over` sets taking its place.
    pub(crate) fn overridden_by(self, over: Self) -> Self {
        Self {
            bold: over.bold.or(self.bold),
            italic: over.italic.or(self.italic),
            underline: over.underline.or(self.underline),
            strike: over.strike.or(self.strike),
            code: over.code.or(self.code),
            superscript: over.superscript.or(self.superscript),
            subscript: over.subscript.or(self.subscript),
            hidden: over.hidden.or(self.hidden),
        }
    }
}

/// A toggle property: present means on, unless its value says otherwise.
pub(crate) fn on_off(element: &Element) -> bool {
    !matches!(
        element.attribute(W, "val"),
        Some("false" | "0" | "off" | "none")
    )
}

/// A font name that is monospace in every common office and system font set.
pub(crate) fn is_monospace_font(font: &str) -> bool {
    let font = font.to_ascii_lowercase();
    [
        "mono",
        "courier",
        "consolas",
        "menlo",
        "monaco",
        "lucida console",
        "inconsolata",
        "code",
    ]
    .iter()
    .any(|marker| font.contains(marker))
}

#[derive(Debug, Clone, Default)]
struct StyleDefinition {
    paragraph: bool,
    name: String,
    based_on: Option<String>,
    outline_level: Option<u8>,
    numbering: Option<NumberingReference>,
    run_format: RunFormat,
}

/// The styles of a document, by id.
#[derive(Debug, Default)]
pub(crate) struct Styles {
    by_id: HashMap<String, StyleDefinition>,
    default_paragraph: Option<String>,
}

impl Styles {
    /// Parse the styles part; a document without one has only direct formatting.
    pub(crate) fn parse(xml: Option<&str>) -> Result<Self, OfficeError> {
        let Some(xml) = xml else {
            return Ok(Self::default());
        };
        let root = parse_tree(xml, |_, _| false)?;
        let mut styles = Self::default();
        for style in root.elements().filter(|element| element.is(W, "style")) {
            let Some(id) = style.attribute(W, "styleId") else {
                continue;
            };
            let kind = style.attribute(W, "type").unwrap_or("paragraph");
            if kind != "paragraph" && kind != "character" {
                continue;
            }
            let paragraph = kind == "paragraph";
            if paragraph
                && style
                    .attribute(W, "default")
                    .is_some_and(|value| value == "1")
            {
                styles.default_paragraph = Some(id.to_owned());
            }
            let paragraph_properties = style.child(W, "pPr");
            styles.by_id.insert(
                id.to_owned(),
                StyleDefinition {
                    paragraph,
                    name: style
                        .child(W, "name")
                        .and_then(|name| name.attribute(W, "val"))
                        .unwrap_or(id)
                        .to_ascii_lowercase(),
                    based_on: style
                        .child(W, "basedOn")
                        .and_then(|based_on| based_on.attribute(W, "val"))
                        .map(str::to_owned),
                    outline_level: paragraph_properties.and_then(outline_level),
                    numbering: paragraph_properties.and_then(numbering_reference),
                    run_format: style
                        .child(W, "rPr")
                        .map(RunFormat::parse)
                        .unwrap_or_default(),
                },
            );
        }
        Ok(styles)
    }

    /// What a paragraph of this style is; `None` means the default paragraph style.
    pub(crate) fn paragraph_style(&self, style_id: Option<&str>) -> ParagraphStyle {
        let mut resolved = ParagraphStyle {
            role: ParagraphRole::Normal,
            outline_level: None,
            numbering: None,
        };
        let mut role = None;
        for definition in self.chain(style_id.or(self.default_paragraph.as_deref())) {
            if !definition.paragraph {
                continue;
            }
            role = role.or_else(|| role_from_name(&definition.name));
            resolved.outline_level = resolved.outline_level.or(definition.outline_level);
            resolved.numbering = resolved.numbering.or(definition.numbering);
        }
        resolved.role = role.unwrap_or(ParagraphRole::Normal);
        resolved
    }

    /// The formatting a character style gives a run, with the code role its name may carry.
    pub(crate) fn character_format(&self, style_id: &str) -> RunFormat {
        let mut format = RunFormat::default();
        let chain: Vec<&StyleDefinition> = self.chain(Some(style_id)).collect();
        for definition in chain.iter().rev() {
            let mut named = RunFormat::default();
            match definition.name.as_str() {
                "strong" => named.bold = Some(true),
                "emphasis" => named.italic = Some(true),
                name if name.contains("code") || name.contains("verbatim") => {
                    named.code = Some(true);
                }
                _ => {}
            }
            format = format
                .overridden_by(named)
                .overridden_by(definition.run_format);
        }
        format
    }

    /// The style and the styles it is based on, nearest first, at most [`MAX_STYLE_CHAIN`].
    fn chain<'a>(&'a self, style_id: Option<&'a str>) -> impl Iterator<Item = &'a StyleDefinition> {
        let mut next = style_id;
        std::iter::from_fn(move || {
            let definition = self.by_id.get(next?)?;
            next = definition.based_on.as_deref();
            Some(definition)
        })
        .take(MAX_STYLE_CHAIN)
    }
}

/// The role a style's name gives it: built-in names are stored in English whatever the language
/// of the user interface.
fn role_from_name(name: &str) -> Option<ParagraphRole> {
    if let Some(level) = name.strip_prefix("heading ") {
        return level
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|level| (1..=9).contains(level))
            .map(ParagraphRole::Heading);
    }
    if name == "title" {
        return Some(ParagraphRole::Heading(1));
    }
    let matches_any = |fragments: &[&str]| fragments.iter().any(|fragment| name.contains(fragment));
    if matches_any(&QUOTE_NAMES) {
        Some(ParagraphRole::Quote)
    } else if matches_any(&CODE_NAMES) {
        Some(ParagraphRole::Code)
    } else if matches_any(&RULE_NAMES) {
        Some(ParagraphRole::HorizontalRule)
    } else {
        None
    }
}

/// `w:outlineLvl` of a `w:pPr`, unless it says body text.
pub(crate) fn outline_level(properties: &Element) -> Option<u8> {
    properties
        .child(W, "outlineLvl")?
        .attribute(W, "val")?
        .parse::<u8>()
        .ok()
        .filter(|level| *level < BODY_TEXT_OUTLINE_LEVEL)
}

/// `w:numPr` of a `w:pPr`.
pub(crate) fn numbering_reference(properties: &Element) -> Option<NumberingReference> {
    let numbering = properties.child(W, "numPr")?;
    let num_id = numbering
        .child(W, "numId")?
        .attribute(W, "val")?
        .parse::<u32>()
        .ok()?;
    let level = numbering
        .child(W, "ilvl")
        .and_then(|level| level.attribute(W, "val"))
        .and_then(|level| level.parse::<u8>().ok());
    Some(NumberingReference { num_id, level })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::styles_xml;

    fn styles(definitions: &[(&str, &str, &str)]) -> Styles {
        Styles::parse(Some(&styles_xml(definitions))).unwrap()
    }

    #[test]
    fn reads_heading_styles_by_name_and_by_outline_level() {
        let styles = styles(&[
            ("Titolo1", "heading 1", ""),
            (
                "Mio",
                "Mio titolo",
                r#"<w:pPr><w:outlineLvl w:val="2"/></w:pPr>"#,
            ),
            ("Titolo", "Title", ""),
        ]);
        assert_eq!(
            styles.paragraph_style(Some("Titolo1")).role,
            ParagraphRole::Heading(1)
        );
        assert_eq!(styles.paragraph_style(Some("Mio")).outline_level, Some(2));
        assert_eq!(
            styles.paragraph_style(Some("Titolo")).role,
            ParagraphRole::Heading(1)
        );
        assert_eq!(
            styles.paragraph_style(Some("Assente")).role,
            ParagraphRole::Normal
        );
    }

    #[test]
    fn inherits_role_numbering_and_outline_through_based_on() {
        let styles = styles(&[
            (
                "Base",
                "List Bullet",
                r#"<w:pPr><w:numPr><w:numId w:val="7"/></w:numPr></w:pPr>"#,
            ),
            ("Figlio", "Il mio elenco", r#"<w:basedOn w:val="Base"/>"#),
            ("Citazione", "Intense Quote", ""),
            (
                "Sotto",
                "Mia citazione",
                r#"<w:basedOn w:val="Citazione"/>"#,
            ),
        ]);
        let child = styles.paragraph_style(Some("Figlio"));
        assert_eq!(
            child.numbering,
            Some(NumberingReference {
                num_id: 7,
                level: None
            })
        );
        assert_eq!(
            styles.paragraph_style(Some("Sotto")).role,
            ParagraphRole::Quote
        );
    }

    #[test]
    fn recognises_code_quote_and_rule_styles_in_english_and_italian() {
        let styles = styles(&[
            ("Codice", "Source Code", ""),
            ("Pre", "HTML Preformatted", ""),
            ("Blocco", "Block Text", ""),
            ("Q", "Quotations", ""),
            ("Blocco2", "Citazione in blocco", ""),
            ("Pre2", "Testo preformattato", ""),
            ("Linea", "Linea orizzontale", ""),
            ("Line", "Horizontal Line", ""),
        ]);
        assert_eq!(
            styles.paragraph_style(Some("Codice")).role,
            ParagraphRole::Code
        );
        assert_eq!(
            styles.paragraph_style(Some("Pre")).role,
            ParagraphRole::Code
        );
        assert_eq!(
            styles.paragraph_style(Some("Blocco")).role,
            ParagraphRole::Quote
        );
        assert_eq!(styles.paragraph_style(Some("Q")).role, ParagraphRole::Quote);
        assert_eq!(
            styles.paragraph_style(Some("Blocco2")).role,
            ParagraphRole::Quote
        );
        assert_eq!(
            styles.paragraph_style(Some("Pre2")).role,
            ParagraphRole::Code
        );
        assert_eq!(
            styles.paragraph_style(Some("Linea")).role,
            ParagraphRole::HorizontalRule
        );
        assert_eq!(
            styles.paragraph_style(Some("Line")).role,
            ParagraphRole::HorizontalRule
        );
    }

    #[test]
    fn stops_a_based_on_loop() {
        let styles = styles(&[
            ("A", "uno", r#"<w:basedOn w:val="B"/>"#),
            ("B", "due", r#"<w:basedOn w:val="A"/>"#),
        ]);
        assert_eq!(
            styles.paragraph_style(Some("A")).role,
            ParagraphRole::Normal
        );
    }

    #[test]
    fn uses_the_default_paragraph_style_when_none_is_named() {
        let xml = styles_xml(&[("Normale", "Normal", "")]).replace(
            r#"w:styleId="Normale""#,
            r#"w:styleId="Normale" w:default="1""#,
        );
        let xml = xml.replace(r#"w:val="Normal""#, r#"w:val="heading 2""#);
        let styles = Styles::parse(Some(&xml)).unwrap();
        assert_eq!(styles.paragraph_style(None).role, ParagraphRole::Heading(2));
    }

    #[test]
    fn gives_character_styles_their_formatting() {
        let xml = styles_xml(&[]).replace(
            "</w:styles>",
            concat!(
                r#"<w:style w:type="character" w:styleId="Forte"><w:name w:val="Strong"/></w:style>"#,
                r#"<w:style w:type="character" w:styleId="Codice"><w:name w:val="HTML Code"/></w:style>"#,
                r#"<w:style w:type="character" w:styleId="Mio"><w:name w:val="Mio"/><w:basedOn w:val="Forte"/><w:rPr><w:i/></w:rPr></w:style>"#,
                "</w:styles>",
            ),
        );
        let styles = Styles::parse(Some(&xml)).unwrap();
        assert_eq!(styles.character_format("Forte").bold, Some(true));
        assert_eq!(styles.character_format("Codice").code, Some(true));
        let mine = styles.character_format("Mio");
        assert_eq!((mine.bold, mine.italic), (Some(true), Some(true)));
        assert_eq!(styles.character_format("Assente"), RunFormat::default());
    }

    #[test]
    fn reads_run_properties_with_their_toggle_values() {
        let xml = r#"<w:rPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:b/><w:i w:val="0"/><w:u w:val="single"/><w:strike/><w:vertAlign w:val="superscript"/><w:rFonts w:ascii="Courier New"/><w:vanish w:val="false"/></w:rPr>"#;
        let format = RunFormat::parse(&parse_tree(xml, |_, _| false).unwrap());
        assert_eq!(format.bold, Some(true));
        assert_eq!(format.italic, Some(false));
        assert_eq!(format.underline, Some(true));
        assert_eq!(format.strike, Some(true));
        assert_eq!(format.superscript, Some(true));
        assert_eq!(format.subscript, Some(false));
        assert_eq!(format.code, Some(true));
        assert_eq!(format.hidden, Some(false));

        let base = RunFormat {
            bold: Some(true),
            italic: Some(true),
            ..RunFormat::default()
        };
        let over = RunFormat {
            bold: Some(false),
            ..RunFormat::default()
        };
        let merged = base.overridden_by(over);
        assert_eq!((merged.bold, merged.italic), (Some(false), Some(true)));
    }

    #[test]
    fn tells_monospace_fonts() {
        assert!(is_monospace_font("Courier New"));
        assert!(is_monospace_font("JetBrains Mono"));
        assert!(is_monospace_font("Consolas"));
        assert!(!is_monospace_font("Calibri"));
        assert!(!is_monospace_font("Times New Roman"));
    }

    #[test]
    fn has_no_style_without_a_styles_part() {
        let styles = Styles::parse(None).unwrap();
        assert_eq!(
            styles.paragraph_style(Some("x")).role,
            ParagraphRole::Normal
        );
    }

    #[test]
    fn refuses_a_malformed_styles_part() {
        assert!(Styles::parse(Some("<w:styles><w:style></w:styles>")).is_err());
    }
}
