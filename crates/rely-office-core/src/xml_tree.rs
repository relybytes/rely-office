// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A small element tree with resolved namespaces, for parts read structurally rather than as a
//! stream of text.

use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;

use crate::OfficeError;
use crate::xml_text::attribute_value;

/// How deep elements may nest; past it the part is refused instead of exhausting the stack.
const MAX_DEPTH: usize = 256;

/// The namespaces the readers look at; every other one is [`Namespace::Other`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Namespace {
    /// No namespace: an unprefixed attribute, or an element outside any default namespace.
    None,
    Word,
    Relationships,
    WordDrawing,
    Drawing,
    Presentation,
    Chart,
    SpreadsheetDrawing,
    Picture,
    Math,
    MarkupCompatibility,
    Vml,
    VmlOffice,
    OdfOffice,
    OdfText,
    OdfTable,
    OdfStyle,
    OdfFo,
    OdfDraw,
    OdfSvg,
    OdfPresentation,
    XLink,
    Other,
}

impl Namespace {
    fn from_uri(uri: &str) -> Self {
        match uri {
            "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            | "http://purl.oclc.org/ooxml/wordprocessingml/main" => Self::Word,
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            | "http://purl.oclc.org/ooxml/officeDocument/relationships" => Self::Relationships,
            "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            | "http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing" => Self::WordDrawing,
            "http://schemas.openxmlformats.org/drawingml/2006/main"
            | "http://purl.oclc.org/ooxml/drawingml/main" => Self::Drawing,
            "http://schemas.openxmlformats.org/presentationml/2006/main"
            | "http://purl.oclc.org/ooxml/presentationml/main" => Self::Presentation,
            "http://schemas.openxmlformats.org/drawingml/2006/chart"
            | "http://purl.oclc.org/ooxml/drawingml/chart" => Self::Chart,
            "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"
            | "http://purl.oclc.org/ooxml/drawingml/spreadsheetDrawing" => Self::SpreadsheetDrawing,
            "http://schemas.openxmlformats.org/drawingml/2006/picture"
            | "http://purl.oclc.org/ooxml/drawingml/picture" => Self::Picture,
            "http://schemas.openxmlformats.org/officeDocument/2006/math"
            | "http://purl.oclc.org/ooxml/officeDocument/math" => Self::Math,
            "http://schemas.openxmlformats.org/markup-compatibility/2006" => {
                Self::MarkupCompatibility
            }
            "urn:schemas-microsoft-com:vml" => Self::Vml,
            "urn:schemas-microsoft-com:office:office" => Self::VmlOffice,
            "urn:oasis:names:tc:opendocument:xmlns:office:1.0" => Self::OdfOffice,
            "urn:oasis:names:tc:opendocument:xmlns:text:1.0" => Self::OdfText,
            "urn:oasis:names:tc:opendocument:xmlns:table:1.0" => Self::OdfTable,
            "urn:oasis:names:tc:opendocument:xmlns:style:1.0" => Self::OdfStyle,
            "urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" => Self::OdfFo,
            "urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" => Self::OdfDraw,
            "urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" => Self::OdfSvg,
            "urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" => Self::OdfPresentation,
            "http://www.w3.org/1999/xlink" => Self::XLink,
            _ => Self::Other,
        }
    }

    fn from_resolved(resolved: &ResolveResult<'_>) -> Self {
        match resolved {
            ResolveResult::Bound(namespace) => Self::from_uri(namespace.as_ref()),
            ResolveResult::Unbound => Self::None,
            ResolveResult::Unknown(_) => Self::Other,
        }
    }
}

/// A node of the tree: an element or a run of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    Element(Element),
    Text(String),
}

/// An element with its namespace, local name, attributes and children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Element {
    pub namespace: Namespace,
    pub name: String,
    attributes: Vec<(Namespace, String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    pub(crate) fn is(&self, namespace: Namespace, name: &str) -> bool {
        self.namespace == namespace && self.name == name
    }

    pub(crate) fn attribute(&self, namespace: Namespace, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(attribute_namespace, attribute_name, _)| {
                *attribute_namespace == namespace && attribute_name == name
            })
            .map(|(_, _, value)| value.as_str())
    }

    /// The child elements, without the text between them.
    pub(crate) fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|node| match node {
            Node::Element(element) => Some(element),
            Node::Text(_) => None,
        })
    }

    pub(crate) fn child(&self, namespace: Namespace, name: &str) -> Option<&Element> {
        self.elements().find(|element| element.is(namespace, name))
    }

    /// The first element of the subtree, this one included, with that name.
    pub(crate) fn descendant(&self, namespace: Namespace, name: &str) -> Option<&Element> {
        if self.is(namespace, name) {
            return Some(self);
        }
        self.elements()
            .find_map(|element| element.descendant(namespace, name))
    }

    /// The text directly inside this element.
    pub(crate) fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|node| match node {
                Node::Text(text) => Some(text.as_str()),
                Node::Element(_) => None,
            })
            .collect()
    }
}

/// Parse a part into its root element. Whitespace-only text is kept only inside the elements
/// `keeps_whitespace` names, where it is content (`w:t`) rather than indentation.
pub(crate) fn parse_tree(
    xml: &str,
    keeps_whitespace: impl Fn(Namespace, &str) -> bool,
) -> Result<Element, OfficeError> {
    let malformed = |error: &dyn std::fmt::Display| OfficeError::MalformedXml(error.to_string());
    let mut reader = NsReader::from_str(xml);
    let mut stack: Vec<Element> = Vec::new();

    loop {
        let (resolved, event) = reader
            .read_resolved_event()
            .map_err(|error| malformed(&error))?;
        let namespace = Namespace::from_resolved(&resolved);
        match event {
            Event::Start(start) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(OfficeError::MalformedXml(format!(
                        "elements nest deeper than {MAX_DEPTH} levels"
                    )));
                }
                let element = element_from(&reader, namespace, &start);
                stack.push(element);
            }
            Event::Empty(start) => {
                let element = element_from(&reader, namespace, &start);
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(element)),
                    None => return Ok(element),
                }
            }
            Event::End(_) => {
                let Some(element) = stack.pop() else {
                    return Err(OfficeError::MalformedXml("unbalanced end tag".to_owned()));
                };
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(element)),
                    None => return Ok(element),
                }
            }
            Event::Text(text) => {
                push_text(&mut stack, &text.xml10_content(), &keeps_whitespace);
            }
            Event::CData(text) => push_text(&mut stack, &text, &keeps_whitespace),
            Event::GeneralRef(reference) => {
                let resolved = match reference.resolve_char_ref() {
                    Ok(Some(character)) => Some(character.to_string()),
                    _ => quick_xml::escape::resolve_xml_entity(&reference).map(str::to_owned),
                };
                if let Some(resolved) = resolved {
                    push_text(&mut stack, &resolved, &|_, _| true);
                }
            }
            Event::Eof => {
                return Err(OfficeError::MalformedXml(
                    "the part ends before its root element closes".to_owned(),
                ));
            }
            _ => {}
        }
    }
}

fn element_from(reader: &NsReader<&[u8]>, namespace: Namespace, start: &BytesStart<'_>) -> Element {
    let attributes = start
        .attributes()
        .filter_map(Result::ok)
        .filter_map(|attribute| {
            let (resolved, local_name) = reader.resolver().resolve_attribute(attribute.key);
            let value = attribute_value(&attribute)?;
            Some((
                Namespace::from_resolved(&resolved),
                local_name.as_ref().to_owned(),
                value,
            ))
        })
        .collect();
    Element {
        namespace,
        name: start.local_name().as_ref().to_owned(),
        attributes,
        children: Vec::new(),
    }
}

fn push_text(
    stack: &mut [Element],
    text: &str,
    keeps_whitespace: &impl Fn(Namespace, &str) -> bool,
) {
    let Some(parent) = stack.last_mut() else {
        return;
    };
    if text.trim().is_empty() && !keeps_whitespace(parent.namespace, &parent.name) {
        return;
    }
    if let Some(Node::Text(previous)) = parent.children.last_mut() {
        previous.push_str(text);
    } else {
        parent.children.push(Node::Text(text.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    fn keeps_word_text(namespace: Namespace, name: &str) -> bool {
        namespace == Namespace::Word && name == "t"
    }

    #[test]
    fn resolves_namespaces_whatever_the_prefix() {
        let xml = format!(
            r#"<x:document xmlns:x="{W}"><x:p x:rsid="1" plain="2"><x:t> a </x:t></x:p></x:document>"#
        );
        let root = parse_tree(&xml, keeps_word_text).unwrap();
        assert!(root.is(Namespace::Word, "document"));
        let paragraph = root.child(Namespace::Word, "p").unwrap();
        assert_eq!(paragraph.attribute(Namespace::Word, "rsid"), Some("1"));
        assert_eq!(paragraph.attribute(Namespace::None, "plain"), Some("2"));
        assert_eq!(paragraph.child(Namespace::Word, "t").unwrap().text(), " a ");
    }

    #[test]
    fn keeps_whitespace_only_where_it_is_content() {
        let xml = format!(r#"<w:p xmlns:w="{W}">  <w:t> </w:t>  <w:x>  </w:x></w:p>"#);
        let root = parse_tree(&xml, keeps_word_text).unwrap();
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.child(Namespace::Word, "t").unwrap().text(), " ");
        assert_eq!(
            root.child(Namespace::Word, "x").unwrap().children,
            Vec::new()
        );
    }

    #[test]
    fn resolves_entities_into_the_text() {
        let xml = format!(r#"<w:t xmlns:w="{W}">a &amp; b &#232;<![CDATA[<c>]]></w:t>"#);
        assert_eq!(
            parse_tree(&xml, keeps_word_text).unwrap().text(),
            "a & b \u{e8}<c>"
        );
    }

    #[test]
    fn finds_descendants_depth_first() {
        let xml = format!(r#"<w:a xmlns:w="{W}"><w:b><w:c id="1"/></w:b><w:c id="2"/></w:a>"#);
        let root = parse_tree(&xml, keeps_word_text).unwrap();
        let found = root.descendant(Namespace::Word, "c").unwrap();
        assert_eq!(found.attribute(Namespace::None, "id"), Some("1"));
        assert!(root.descendant(Namespace::Word, "z").is_none());
    }

    #[test]
    fn accepts_an_empty_root() {
        let root = parse_tree("<root/>", |_, _| false).unwrap();
        assert!(root.is(Namespace::None, "root"));
    }

    #[test]
    fn refuses_malformed_or_unfinished_xml() {
        assert!(parse_tree("<a><b></a>", |_, _| false).is_err());
        assert!(parse_tree("<a><b/>", |_, _| false).is_err());
        assert!(parse_tree("", |_, _| false).is_err());
    }

    #[test]
    fn refuses_nesting_past_the_depth_limit() {
        let xml = format!(
            "{}{}",
            "<a>".repeat(MAX_DEPTH + 1),
            "</a>".repeat(MAX_DEPTH + 1)
        );
        assert!(matches!(
            parse_tree(&xml, |_, _| false).unwrap_err(),
            OfficeError::MalformedXml(_)
        ));
        let xml = format!("{}{}", "<a>".repeat(MAX_DEPTH), "</a>".repeat(MAX_DEPTH));
        assert!(parse_tree(&xml, |_, _| false).is_ok());
    }
}
