// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::archive::{ZipPackage, member_names, open_package, read_text_member};
use crate::relationships::{main_part_path, read_relationships};
use crate::text::TextBuffer;
use crate::xml_text::{TextRules, append_xml_text, attribute_value};
use crate::{Limits, OfficeError};

const MAIN_PART: &str = "ppt/presentation.xml";

/// `DrawingML` text: runs (`a:t`) inside paragraphs (`a:p`); fields (`a:fld`, the slide number
/// and the date) are layout, not content.
const SLIDE_RULES: TextRules<'static> = TextRules {
    text_containers: &["t"],
    newline_after: &["p"],
    tab_after: &["tc"],
    tab_elements: &[],
    break_elements: &["br"],
    skipped: &["fld", "Fallback"],
    space_element: None,
};

/// The text of a pptx: each slide in presentation order, followed by its speaker notes.
pub(crate) fn extract_pptx_text(
    bytes: &[u8],
    limits: &Limits,
    buffer: &mut TextBuffer,
) -> Result<(), OfficeError> {
    let mut package = open_package(bytes)?;
    let main_part = main_part_path(&mut package, limits, MAIN_PART)?;

    for slide in slide_parts(&mut package, &main_part, limits)? {
        let Some(xml) = read_text_member(&mut package, &slide, limits)? else {
            continue;
        };
        append_xml_text(&xml, &SLIDE_RULES, buffer)?;

        let notes = read_relationships(&mut package, &slide, limits)?
            .into_iter()
            .find(|relationship| !relationship.external && relationship.is_kind("notesSlide"));
        if let Some(notes) = notes
            && let Some(xml) = read_text_member(&mut package, &notes.target, limits)?
        {
            append_xml_text(&xml, &SLIDE_RULES, buffer)?;
        }
        if buffer.is_truncated() {
            break;
        }
    }

    Ok(())
}

/// The slide parts in the order `p:sldIdLst` lists them; when the presentation part is missing
/// or lists none, every `ppt/slides/slideN.xml` by its number.
pub(crate) fn slide_parts(
    package: &mut ZipPackage<'_>,
    main_part: &str,
    limits: &Limits,
) -> Result<Vec<String>, OfficeError> {
    if let Some(xml) = read_text_member(package, main_part, limits)? {
        let relationship_ids = slide_relationship_ids(&xml)?;
        let relationships = read_relationships(package, main_part, limits)?;
        let ordered: Vec<String> = relationship_ids
            .iter()
            .filter_map(|id| {
                relationships
                    .iter()
                    .find(|relationship| &relationship.id == id && !relationship.external)
                    .map(|relationship| relationship.target.clone())
            })
            .collect();
        if !ordered.is_empty() {
            return Ok(ordered);
        }
    }

    let mut numbered: Vec<(u32, String)> = member_names(package)
        .into_iter()
        .filter_map(|name| {
            let number = name
                .strip_prefix("ppt/slides/slide")?
                .strip_suffix(".xml")?
                .parse()
                .ok()?;
            Some((number, name))
        })
        .collect();
    numbered.sort();
    Ok(numbered.into_iter().map(|(_, name)| name).collect())
}

/// The `r:id` of every `p:sldId`, in document order.
fn slide_relationship_ids(xml: &str) -> Result<Vec<String>, OfficeError> {
    let mut reader = Reader::from_str(xml);
    let mut ids = Vec::new();
    loop {
        match reader
            .read_event()
            .map_err(|error| OfficeError::MalformedXml(error.to_string()))?
        {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == "sldId" =>
            {
                let id = element
                    .attributes()
                    .filter_map(Result::ok)
                    .find(|attribute| {
                        attribute.key.prefix().is_some()
                            && attribute.key.local_name().as_ref() == "id"
                    });
                if let Some(value) = id.as_ref().and_then(attribute_value) {
                    ids.push(value);
                }
            }
            Event::Eof => return Ok(ids),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{build_pptx, build_zip};

    fn extract(bytes: &[u8], limits: &Limits) -> Result<String, OfficeError> {
        let mut buffer = TextBuffer::new(limits.max_text_chars);
        extract_pptx_text(bytes, limits, &mut buffer)?;
        Ok(buffer.into_text())
    }

    #[test]
    fn reads_slides_in_presentation_order_with_their_notes() {
        let bytes = build_pptx(
            &[&["Primo", "punto"], &["Secondo"]],
            &[(0, "Nota del relatore")],
        );
        assert_eq!(
            extract(&bytes, &Limits::default()).unwrap(),
            "Primo\npunto\nNota del relatore\nSecondo\n"
        );
    }

    #[test]
    fn follows_the_slide_list_not_the_file_names() {
        let bytes = crate::test_support::build_pptx_in_order(&["Uno", "Due"], &[2, 1]);
        assert_eq!(extract(&bytes, &Limits::default()).unwrap(), "Due\nUno\n");
    }

    #[test]
    fn falls_back_to_slide_numbers_without_a_presentation_part() {
        let slide = |text: &str| {
            format!(
                r#"<p:sld xmlns:p="p" xmlns:a="a"><a:p><a:r><a:t>{text}</a:t></a:r></a:p></p:sld>"#
            )
        };
        let (two, ten) = (slide("Due"), slide("Dieci"));
        let bytes = build_zip(&[
            ("ppt/slides/slide10.xml", ten.as_bytes()),
            ("ppt/slides/slide2.xml", two.as_bytes()),
        ]);
        assert_eq!(extract(&bytes, &Limits::default()).unwrap(), "Due\nDieci\n");
    }

    #[test]
    fn leaves_out_slide_number_fields() {
        let bytes = crate::test_support::build_pptx_from_paragraph_xml(&[
            r#"<a:p><a:r><a:t>Testo</a:t></a:r><a:fld id="x" type="slidenum"><a:t>3</a:t></a:fld></a:p>"#,
        ]);
        assert_eq!(extract(&bytes, &Limits::default()).unwrap(), "Testo\n");
    }

    #[test]
    fn stops_at_the_character_limit() {
        let limits = Limits {
            max_text_chars: 3,
            ..Limits::default()
        };
        let bytes = build_pptx(&[&["Primo"], &["Secondo"]], &[]);
        assert_eq!(extract(&bytes, &limits).unwrap(), "Pri");
    }

    #[test]
    fn refuses_a_slide_above_the_member_cap() {
        let limits = Limits {
            max_member_bytes: 64,
            ..Limits::default()
        };
        let bytes = build_pptx(&[&["Primo"]], &[]);
        assert!(matches!(
            extract(&bytes, &limits).unwrap_err(),
            OfficeError::TooLarge { .. }
        ));
    }

    #[test]
    fn refuses_malformed_slide_xml() {
        let bytes = build_zip(&[("ppt/slides/slide1.xml", b"<a:p><a:t>x</a:p>".as_slice())]);
        assert!(matches!(
            extract(&bytes, &Limits::default()).unwrap_err(),
            OfficeError::MalformedXml(_)
        ));
    }
}
