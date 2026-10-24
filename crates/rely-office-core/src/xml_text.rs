// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A streaming walk that turns the XML of a document part into plain text for a search index.

use quick_xml::Reader;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::{BytesStart, Event};

use crate::OfficeError;
use crate::text::TextBuffer;

/// The most spaces a single `OpenDocument` `text:s` element may expand to.
const MAX_SPACE_RUN: usize = 1_000;

/// What a format's XML means for its text; every list holds local names, without prefix.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TextRules<'a> {
    /// Text is taken only inside one of these elements.
    pub text_containers: &'a [&'a str],
    /// A newline follows the end of each of these (paragraphs).
    pub newline_after: &'a [&'a str],
    /// A tab follows the end of each of these (table cells).
    pub tab_after: &'a [&'a str],
    /// Each of these stands for a tab.
    pub tab_elements: &'a [&'a str],
    /// Each of these stands for a line break.
    pub break_elements: &'a [&'a str],
    /// The whole subtree of each of these is left out.
    pub skipped: &'a [&'a str],
    /// The `OpenDocument` space run: `text:s`, repeated by its `text:c` attribute.
    pub space_element: Option<&'a str>,
}

/// An attribute's value with its entities resolved; `None` when they cannot be.
pub(crate) fn attribute_value(attribute: &Attribute<'_>) -> Option<String> {
    attribute
        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
        .ok()
        .map(std::borrow::Cow::into_owned)
}

/// Append the text of one XML part to the buffer, stopping once the buffer is full.
pub(crate) fn append_xml_text(
    xml: &str,
    rules: &TextRules<'_>,
    buffer: &mut TextBuffer,
) -> Result<(), OfficeError> {
    let mut reader = Reader::from_str(xml);
    let mut container_depth = 0_usize;
    let mut skipped_depth = 0_usize;

    loop {
        if buffer.is_truncated() {
            return Ok(());
        }
        let event = reader
            .read_event()
            .map_err(|error| OfficeError::MalformedXml(error.to_string()))?;
        match event {
            Event::Start(element) => {
                let name = element.local_name();
                let name = name.as_ref();
                if skipped_depth > 0 || rules.skipped.contains(&name) {
                    skipped_depth += 1;
                } else if rules.text_containers.contains(&name) {
                    container_depth += 1;
                }
            }
            Event::End(element) => {
                let name = element.local_name();
                let name = name.as_ref();
                if skipped_depth > 0 {
                    skipped_depth -= 1;
                    continue;
                }
                if rules.text_containers.contains(&name) {
                    container_depth = container_depth.saturating_sub(1);
                }
                close_element(name, rules, buffer);
            }
            Event::Empty(element) if skipped_depth == 0 => {
                append_empty_element(&element, rules, container_depth > 0, buffer);
            }
            Event::Text(run) if skipped_depth == 0 && container_depth > 0 => {
                buffer.push_str(&run.xml10_content());
            }
            Event::CData(run) if skipped_depth == 0 && container_depth > 0 => {
                buffer.push_str(&run);
            }
            Event::GeneralRef(reference) if skipped_depth == 0 && container_depth > 0 => {
                if let Ok(Some(character)) = reference.resolve_char_ref() {
                    buffer.push(character);
                } else if let Some(resolved) = quick_xml::escape::resolve_xml_entity(&reference) {
                    buffer.push_str(resolved);
                }
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
    }
}

fn close_element(name: &str, rules: &TextRules<'_>, buffer: &mut TextBuffer) {
    if rules.newline_after.contains(&name) {
        // An empty paragraph adds nothing a search can match, so blank lines collapse.
        if !buffer.ends_with_newline() {
            buffer.push('\n');
        }
    } else if rules.tab_after.contains(&name) {
        buffer.push('\t');
    }
}

fn append_empty_element(
    element: &BytesStart<'_>,
    rules: &TextRules<'_>,
    in_container: bool,
    buffer: &mut TextBuffer,
) {
    let name = element.local_name();
    let name = name.as_ref();
    if rules.skipped.contains(&name) {
        return;
    }
    if rules.tab_elements.contains(&name) {
        buffer.push('\t');
    } else if rules.break_elements.contains(&name) {
        buffer.push('\n');
    } else if rules.space_element == Some(name) && in_container {
        let count = element
            .attributes()
            .filter_map(Result::ok)
            .find(|attribute| attribute.key.local_name().as_ref() == "c")
            .and_then(|attribute| attribute_value(&attribute)?.trim().parse::<usize>().ok())
            .unwrap_or(1)
            .min(MAX_SPACE_RUN);
        for _ in 0..count {
            buffer.push(' ');
        }
    } else {
        close_element(name, rules, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: TextRules<'static> = TextRules {
        text_containers: &["p"],
        newline_after: &["p"],
        tab_after: &["cell"],
        tab_elements: &["tab"],
        break_elements: &["br"],
        skipped: &["hidden"],
        space_element: Some("s"),
    };

    fn walk(xml: &str, max_chars: usize) -> (String, bool) {
        let mut buffer = TextBuffer::new(max_chars);
        append_xml_text(xml, &RULES, &mut buffer).unwrap();
        let truncated = buffer.is_truncated();
        (buffer.into_text(), truncated)
    }

    #[test]
    fn takes_text_only_inside_containers() {
        let (text, _) = walk("<r>out<p>in<x:b>side</x:b></p></r>", 100);
        assert_eq!(text, "inside\n");
    }

    #[test]
    fn expands_tabs_breaks_spaces_and_cells() {
        let (text, _) = walk(
            r#"<r><cell><p>a<tab/>b<br/>c<s c="3"/>d<s/>e</p></cell></r>"#,
            100,
        );
        assert_eq!(text, "a\tb\nc   d e\n\t");
    }

    #[test]
    fn leaves_out_skipped_subtrees() {
        let (text, _) = walk("<r><p>a<hidden><p>b</p><br/></hidden>c</p></r>", 100);
        assert_eq!(text, "ac\n");
    }

    #[test]
    fn resolves_entities_and_cdata() {
        let (text, _) = walk("<p>a &amp; b &#233;<![CDATA[<c>]]></p>", 100);
        assert_eq!(text, "a & b \u{e9}<c>\n");
    }

    #[test]
    fn caps_a_space_run() {
        let (text, _) = walk(r#"<p><s c="999999999"/></p>"#, 1_000_000);
        assert_eq!(text.len(), MAX_SPACE_RUN + 1);
    }

    #[test]
    fn stops_when_the_buffer_is_full() {
        let (text, truncated) = walk("<p>abcdef</p><p>ghi</p>", 3);
        assert_eq!(text, "abc");
        assert!(truncated);
    }

    #[test]
    fn refuses_malformed_xml() {
        let mut buffer = TextBuffer::new(10);
        let error = append_xml_text("<p>a</q>", &RULES, &mut buffer).unwrap_err();
        assert!(matches!(error, OfficeError::MalformedXml(_)));
    }
}
