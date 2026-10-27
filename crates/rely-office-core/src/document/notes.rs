// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The parts around the body: footnotes, endnotes, headers, footers and the settings that say how
//! headers alternate. A damaged side part is left out rather than failing the document.

use super::NoteKind;
use crate::Limits;
use crate::archive::{ZipPackage, read_text_member};
use crate::relationships::{Relationship, read_relationships};
use crate::xml_tree::{Element, Namespace, parse_tree};

const W: Namespace = Namespace::Word;
/// Word's default distance between tab stops, 1.25 cm.
const DEFAULT_TAB_TWIPS: u32 = 708;
/// A tab distance under a millimetre or over the page is a crafted file.
const MIN_TAB_TWIPS: u32 = 57;
const MAX_TAB_TWIPS: u32 = 31_680;

/// A part read with its relationships.
pub(super) struct Part {
    pub root: Element,
    pub relationships: Vec<Relationship>,
}

/// Read a side part, or nothing when it is missing or damaged.
pub(super) fn read_part(package: &mut ZipPackage<'_>, path: &str, limits: &Limits) -> Option<Part> {
    let xml = read_text_member(package, path, limits).ok()??;
    let root = parse_tree(&xml, super::keeps_whitespace).ok()?;
    let relationships = read_relationships(package, path, limits).unwrap_or_default();
    Some(Part {
        root,
        relationships,
    })
}

/// The footnote and endnote parts of a document.
#[derive(Default)]
pub(super) struct NoteParts {
    pub footnotes: Option<Part>,
    pub endnotes: Option<Part>,
}

impl NoteParts {
    /// The note with this id and the relationships of its part; the separators Word keeps among
    /// the notes are not notes.
    pub(super) fn find(&self, kind: NoteKind, id: &str) -> Option<(&Element, &[Relationship])> {
        let (part, name) = match kind {
            NoteKind::Footnote => (self.footnotes.as_ref()?, "footnote"),
            NoteKind::Endnote => (self.endnotes.as_ref()?, "endnote"),
        };
        let note = part.root.elements().find(|note| {
            note.is(W, name)
                && note.attribute(W, "id") == Some(id)
                && note
                    .attribute(W, "type")
                    .is_none_or(|kind| kind == "normal")
        })?;
        Some((note, &part.relationships))
    }
}

/// The comments of a document, and how they thread: for the last paragraph of each comment,
/// the last paragraph of the comment it answers and whether it is done (`commentsExtended`).
#[derive(Default)]
pub(super) struct CommentParts {
    pub comments: Option<Part>,
    pub threads: std::collections::HashMap<String, (Option<String>, bool)>,
}

impl CommentParts {
    /// Read the comments part and its threads, either of them missing or damaged left out.
    pub(super) fn read(comments: Option<Part>, extended: Option<Part>) -> Self {
        let mut threads = std::collections::HashMap::new();
        if let Some(extended) = extended {
            for entry in extended
                .root
                .elements()
                .filter(|entry| entry.name == "commentEx")
            {
                if let Some(paragraph) = entry.attribute(Namespace::Other, "paraId") {
                    threads.insert(
                        paragraph.to_owned(),
                        (
                            entry
                                .attribute(Namespace::Other, "paraIdParent")
                                .map(str::to_owned),
                            entry.attribute(Namespace::Other, "done") == Some("1"),
                        ),
                    );
                }
            }
        }
        Self { comments, threads }
    }

    /// The comment with this id and the relationships of its part.
    pub(super) fn find(&self, id: &str) -> Option<(&Element, &[Relationship])> {
        let part = self.comments.as_ref()?;
        let comment = part
            .root
            .elements()
            .find(|comment| comment.is(W, "comment") && comment.attribute(W, "id") == Some(id))?;
        Some((comment, &part.relationships))
    }
}

/// What `word/settings.xml` says about the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Settings {
    /// Odd and even pages have headers and footers of their own.
    pub even_and_odd_headers: bool,
    pub default_tab_twips: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            even_and_odd_headers: false,
            default_tab_twips: DEFAULT_TAB_TWIPS,
        }
    }
}

impl Settings {
    pub(super) fn parse(xml: Option<&str>) -> Self {
        let Some(root) = xml.and_then(|xml| parse_tree(xml, |_, _| false).ok()) else {
            return Self::default();
        };
        Self {
            even_and_odd_headers: root
                .child(W, "evenAndOddHeaders")
                .is_some_and(super::styles::on_off),
            default_tab_twips: root
                .child(W, "defaultTabStop")
                .and_then(|stop| stop.attribute(W, "val"))
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|value| (MIN_TAB_TWIPS..=MAX_TAB_TWIPS).contains(value))
                .unwrap_or(DEFAULT_TAB_TWIPS),
        }
    }
}

/// Which header or footer a reference of a section names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Slot {
    Header,
    Footer,
    FirstHeader,
    FirstFooter,
    EvenHeader,
    EvenFooter,
}

/// The header and footer parts a section refers to, by slot.
pub(super) fn section_parts(
    section: &Element,
    relationships: &[Relationship],
    package: &mut ZipPackage<'_>,
    limits: &Limits,
) -> Vec<(Slot, Part)> {
    let mut parts = Vec::new();
    for reference in section.elements() {
        let header = reference.is(W, "headerReference");
        if !header && !reference.is(W, "footerReference") {
            continue;
        }
        let slot = match (reference.attribute(W, "type"), header) {
            (Some("first"), true) => Slot::FirstHeader,
            (Some("first"), false) => Slot::FirstFooter,
            (Some("even"), true) => Slot::EvenHeader,
            (Some("even"), false) => Slot::EvenFooter,
            (_, true) => Slot::Header,
            (_, false) => Slot::Footer,
        };
        if parts.iter().any(|(known, _)| *known == slot) {
            continue;
        }
        let Some(target) = reference
            .attribute(Namespace::Relationships, "id")
            .and_then(|id| {
                relationships
                    .iter()
                    .find(|relationship| relationship.id == id && !relationship.external)
            })
            .map(|relationship| relationship.target.clone())
        else {
            continue;
        };
        if let Some(part) = read_part(package, &target, limits) {
            parts.push((slot, part));
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_header_settings() {
        let settings = Settings::parse(Some(concat!(
            r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
            r#"<w:defaultTabStop w:val="720"/><w:evenAndOddHeaders/></w:settings>"#
        )));
        assert_eq!(
            settings,
            Settings {
                even_and_odd_headers: true,
                default_tab_twips: 720
            }
        );
        let odd = Settings::parse(Some(concat!(
            r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
            r#"<w:defaultTabStop w:val="1"/><w:evenAndOddHeaders w:val="0"/></w:settings>"#
        )));
        assert_eq!(odd, Settings::default());
        assert_eq!(Settings::parse(Some("<damaged")), Settings::default());
        assert_eq!(Settings::parse(None), Settings::default());
    }
}
