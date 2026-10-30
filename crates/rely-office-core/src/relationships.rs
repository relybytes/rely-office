// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The relationship parts of an OOXML package (`_rels/*.rels`).

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::archive::{ZipPackage, read_text_member, resolve_part_path};
use crate::xml_text::attribute_value;
use crate::{Limits, OfficeError};

/// One relationship of a part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Relationship {
    pub id: String,
    /// The relationship type URI.
    pub kind: String,
    /// For an internal target, the member path resolved against the source part; for an external
    /// one, the target exactly as written.
    pub target: String,
    pub external: bool,
}

impl Relationship {
    /// Whether the type URI ends with this name (`image`, `hyperlink`, `notesSlide`, ...), which is
    /// how both the transitional and the strict OOXML namespaces spell it.
    pub(crate) fn is_kind(&self, name: &str) -> bool {
        self.kind
            .rsplit_once('/')
            .is_some_and(|(_, last)| last == name)
    }
}

/// The relationships of a part; a part with no relationship file has none.
pub(crate) fn read_relationships(
    package: &mut ZipPackage<'_>,
    source_part: &str,
    limits: &Limits,
) -> Result<Vec<Relationship>, OfficeError> {
    let rels_path = match source_part.rsplit_once('/') {
        Some((folder, file)) => format!("{folder}/_rels/{file}.rels"),
        None => format!("_rels/{source_part}.rels"),
    };
    let Some(xml) = read_text_member(package, &rels_path, limits)? else {
        return Ok(Vec::new());
    };
    parse_relationships(&xml, source_part)
}

fn parse_relationships(xml: &str, source_part: &str) -> Result<Vec<Relationship>, OfficeError> {
    let mut reader = Reader::from_str(xml);
    let mut relationships = Vec::new();
    loop {
        match reader
            .read_event()
            .map_err(|error| OfficeError::MalformedXml(error.to_string()))?
        {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == "Relationship" =>
            {
                if let Some(relationship) = relationship_from(&element, source_part) {
                    relationships.push(relationship);
                }
            }
            Event::Eof => return Ok(relationships),
            _ => {}
        }
    }
}

fn relationship_from(element: &BytesStart<'_>, source_part: &str) -> Option<Relationship> {
    let mut id = None;
    let mut kind = String::new();
    let mut target = None;
    let mut external = false;
    for attribute in element.attributes().filter_map(Result::ok) {
        let value = attribute_value(&attribute)?;
        match attribute.key.local_name().as_ref() {
            "Id" => id = Some(value),
            "Type" => kind = value,
            "Target" => target = Some(value),
            "TargetMode" => external = value.eq_ignore_ascii_case("External"),
            _ => {}
        }
    }
    let target = target?;
    Some(Relationship {
        id: id?,
        kind,
        target: if external {
            target
        } else {
            resolve_part_path(source_part, &target)
        },
        external,
    })
}

/// The main document part a package declares in `_rels/.rels`, or `fallback` when it declares
/// none.
pub(crate) fn main_part_path(
    package: &mut ZipPackage<'_>,
    limits: &Limits,
    fallback: &str,
) -> Result<String, OfficeError> {
    let Some(xml) = read_text_member(package, "_rels/.rels", limits)? else {
        return Ok(fallback.to_owned());
    };
    Ok(parse_relationships(&xml, "")?
        .into_iter()
        .find(|relationship| !relationship.external && relationship.is_kind("officeDocument"))
        .map_or_else(|| fallback.to_owned(), |relationship| relationship.target))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/a.png"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://relybytes.com/?a=1&amp;b=2" TargetMode="External"/><Relationship Type="x" Target="y"/></Relationships>"#;

    #[test]
    fn resolves_internal_targets_and_keeps_external_ones() {
        let relationships = parse_relationships(RELS, "word/document.xml").unwrap();
        assert_eq!(relationships.len(), 2);
        assert_eq!(relationships[0].target, "word/media/a.png");
        assert!(relationships[0].is_kind("image"));
        assert!(!relationships[0].external);
        assert_eq!(relationships[1].target, "https://relybytes.com/?a=1&b=2");
        assert!(relationships[1].external);
        assert!(relationships[1].is_kind("hyperlink"));
    }

    #[test]
    fn finds_the_main_part_or_falls_back() {
        let bytes = crate::test_support::build_zip(&[(
            "_rels/.rels",
            br#"<Relationships><Relationship Id="r" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/main.xml"/></Relationships>"#.as_slice(),
        )]);
        let mut package = crate::archive::open_package(&bytes).unwrap();
        let limits = Limits::default();
        assert_eq!(
            main_part_path(&mut package, &limits, "word/document.xml").unwrap(),
            "word/main.xml"
        );

        let bytes = crate::test_support::build_zip(&[("a.xml", b"<a/>".as_slice())]);
        let mut package = crate::archive::open_package(&bytes).unwrap();
        assert_eq!(
            main_part_path(&mut package, &limits, "word/document.xml").unwrap(),
            "word/document.xml"
        );
    }

    #[test]
    fn reads_no_relationship_for_a_part_without_a_rels_file() {
        let bytes = crate::test_support::build_zip(&[("a.xml", b"<a/>".as_slice())]);
        let mut package = crate::archive::open_package(&bytes).unwrap();
        assert_eq!(
            read_relationships(&mut package, "a.xml", &Limits::default()).unwrap(),
            Vec::new()
        );
    }

    #[test]
    fn refuses_a_malformed_rels_file() {
        assert!(parse_relationships("<Relationships><a></b>", "x").is_err());
    }
}
