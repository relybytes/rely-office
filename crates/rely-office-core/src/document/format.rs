// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! What a paragraph sets itself, as an editor changes it: its spacing, its line spacing, its
//! indents, a page break before it and the named style of Word it takes.
//!
//! The look of a paragraph mixes what the paragraph sets with what its style chain gives it,
//! which is right for drawing it and wrong for editing it: an editor that wrote every inherited
//! value back would pin the style's spacing on every paragraph. [`ParagraphFormat`] is the part
//! the paragraph itself sets, the only one the JSON tree carries.

use super::look::{LineSpacing, ParagraphLook};
use crate::xml_tree::{Element, Namespace};

const W: Namespace = Namespace::Word;

/// The built-in paragraph styles of Word the tree names beside the headings, the quote and the
/// code, which have blocks of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum NamedStyle {
    Title,
    Subtitle,
    IntenseQuote,
    Caption,
    NoSpacing,
}

impl NamedStyle {
    /// The style id the writer's styles part defines for it.
    #[must_use]
    pub fn style_id(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Subtitle => "Subtitle",
            Self::IntenseQuote => "IntenseQuote",
            Self::Caption => "Caption",
            Self::NoSpacing => "NoSpacing",
        }
    }

    /// The style a Word style name stands for, in English or in Italian, case aside.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "title" | "titolo" => Some(Self::Title),
            "subtitle" | "sottotitolo" => Some(Self::Subtitle),
            "intense quote" | "citazione intensa" => Some(Self::IntenseQuote),
            "caption" | "didascalia" => Some(Self::Caption),
            "no spacing" | "nessuna spaziatura" => Some(Self::NoSpacing),
            _ => None,
        }
    }
}

/// How the line spacing of a paragraph is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", tag = "rule", content = "value")
)]
pub enum LineSpacingFormat {
    /// A multiple of single spacing, in 240ths: 240 is single, 360 one and a half.
    Auto(u32),
    /// Exactly this height, in twentieths of a point.
    Exact(u32),
    /// At least this height, in twentieths of a point.
    AtLeast(u32),
}

impl From<LineSpacing> for LineSpacingFormat {
    fn from(spacing: LineSpacing) -> Self {
        match spacing {
            LineSpacing::Auto(value) => Self::Auto(value),
            LineSpacing::Exact(value) => Self::Exact(value),
            LineSpacing::AtLeast(value) => Self::AtLeast(value),
        }
    }
}

impl From<LineSpacingFormat> for LineSpacing {
    fn from(spacing: LineSpacingFormat) -> Self {
        match spacing {
            LineSpacingFormat::Auto(value) => Self::Auto(value),
            LineSpacingFormat::Exact(value) => Self::Exact(value),
            LineSpacingFormat::AtLeast(value) => Self::AtLeast(value),
        }
    }
}

/// What a paragraph sets itself; every field left out is the style's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
pub struct ParagraphFormat {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub style: Option<NamedStyle>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub space_before_twips: Option<u32>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub space_after_twips: Option<u32>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub line_spacing: Option<LineSpacingFormat>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub indent_left_twips: Option<i32>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub indent_right_twips: Option<i32>,
    /// Positive indents the first line, negative hangs it.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub indent_first_line_twips: Option<i32>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "std::ops::Not::not"))]
    pub page_break_before: bool,
}

impl ParagraphFormat {
    /// Whether the paragraph sets nothing itself.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// What a `w:pPr` sets itself, its page break aside: the reader decides that one.
    pub(crate) fn parse(properties: &Element) -> Self {
        let own = ParagraphLook::parse(properties);
        Self {
            style: None,
            space_before_twips: own.space_before_twips,
            space_after_twips: own.space_after_twips,
            line_spacing: own.line_spacing.map(Into::into),
            indent_left_twips: own.indent_left_twips,
            indent_right_twips: own.indent_right_twips,
            indent_first_line_twips: own.indent_first_line_twips,
            page_break_before: properties.child(W, "pageBreakBefore").is_some(),
        }
    }

    /// The look a paragraph of this format has when nothing else gives it one, as a tree from
    /// JSON is written.
    #[must_use]
    pub fn to_look(self) -> ParagraphLook {
        ParagraphLook {
            space_before_twips: self.space_before_twips,
            space_after_twips: self.space_after_twips,
            line_spacing: self.line_spacing.map(Into::into),
            indent_left_twips: self.indent_left_twips,
            indent_right_twips: self.indent_right_twips,
            indent_first_line_twips: self.indent_first_line_twips,
            page_break_before: self.page_break_before,
            format: self,
            ..ParagraphLook::default()
        }
    }
}

/// The `format` of a block's look in the JSON tree: written from the format, read into a look.
#[cfg(feature = "serde")]
pub(crate) mod look_as_format {
    use super::{ParagraphFormat, ParagraphLook};

    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "the signature serde asks for"
    )]
    pub fn serialize<S: serde::Serializer>(
        look: &ParagraphLook,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&look.format, serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ParagraphLook, D::Error> {
        let format: ParagraphFormat = serde::Deserialize::deserialize(deserializer)?;
        Ok(format.to_look())
    }

    #[must_use]
    pub fn is_empty(look: &ParagraphLook) -> bool {
        look.format.is_empty()
    }
}

/// The page field of a run's look in the JSON tree.
#[cfg(feature = "serde")]
pub(crate) mod look_field {
    use super::super::look::{PageField, RunLook};

    #[allow(clippy::ref_option, reason = "the signature serde asks for")]
    pub fn serialize<S: serde::Serializer>(
        look: &RunLook,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&look.field, serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<RunLook, D::Error> {
        let field: Option<PageField> = serde::Deserialize::deserialize(deserializer)?;
        Ok(RunLook {
            field,
            ..RunLook::default()
        })
    }

    #[must_use]
    pub fn is_empty(look: &RunLook) -> bool {
        look.field.is_none()
    }
}

/// The column widths of a table's look in the JSON tree.
#[cfg(feature = "serde")]
pub(crate) mod table_widths {
    use super::super::look::TableLook;

    pub fn serialize<S: serde::Serializer>(
        look: &TableLook,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&look.column_widths_twips, serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<TableLook, D::Error> {
        let column_widths_twips: Vec<u32> = serde::Deserialize::deserialize(deserializer)?;
        Ok(TableLook {
            column_widths_twips,
            ..TableLook::default()
        })
    }

    #[must_use]
    pub fn is_empty(look: &TableLook) -> bool {
        look.column_widths_twips.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_the_named_styles_in_english_and_italian() {
        assert_eq!(NamedStyle::from_name("Title"), Some(NamedStyle::Title));
        assert_eq!(
            NamedStyle::from_name(" Sottotitolo "),
            Some(NamedStyle::Subtitle)
        );
        assert_eq!(
            NamedStyle::from_name("Citazione intensa"),
            Some(NamedStyle::IntenseQuote)
        );
        assert_eq!(NamedStyle::from_name("caption"), Some(NamedStyle::Caption));
        assert_eq!(
            NamedStyle::from_name("Nessuna spaziatura"),
            Some(NamedStyle::NoSpacing)
        );
        assert_eq!(NamedStyle::from_name("Normal"), None);
        assert_eq!(NamedStyle::IntenseQuote.style_id(), "IntenseQuote");
    }

    #[test]
    fn a_format_gives_the_look_it_sets_and_keeps_itself() {
        let format = ParagraphFormat {
            line_spacing: Some(LineSpacingFormat::Exact(300)),
            page_break_before: true,
            ..ParagraphFormat::default()
        };

        let look = format.to_look();

        assert_eq!(look.line_spacing, Some(LineSpacing::Exact(300)));
        assert!(look.page_break_before);
        assert_eq!(look.format, format);
        assert!(!format.is_empty());
        assert!(ParagraphFormat::default().is_empty());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn the_json_tree_carries_only_what_a_paragraph_sets() {
        use crate::document::{Block, Inline, RunLook};
        let plain = Block::Paragraph {
            content: vec![Inline::Text {
                text: "a".to_owned(),
                marks: Vec::new(),
                look: RunLook::default(),
            }],
            align: None,
            look: ParagraphLook {
                space_after_twips: Some(160),
                ..ParagraphLook::default()
            },
        };
        let formatted = Block::Paragraph {
            content: Vec::new(),
            align: None,
            look: ParagraphFormat {
                style: Some(NamedStyle::Caption),
                line_spacing: Some(LineSpacingFormat::Auto(480)),
                ..ParagraphFormat::default()
            }
            .to_look(),
        };

        assert_eq!(
            serde_json::to_string(&plain).unwrap(),
            r#"{"type":"paragraph","content":[{"type":"text","text":"a"}]}"#
        );
        let json = serde_json::to_string(&formatted).unwrap();
        assert_eq!(
            json,
            r#"{"type":"paragraph","content":[],"format":{"style":"caption","lineSpacing":{"rule":"auto","value":480}}}"#
        );
        assert_eq!(serde_json::from_str::<Block>(&json).unwrap(), formatted);
    }
}
