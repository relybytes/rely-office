// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The text of a `PowerPoint` 97-2003 presentation ([MS-PPT]): the `TextCharsAtom` and
//! `TextBytesAtom` records of the `PowerPoint Document` stream, in stream order.

mod persist;
mod slides;
#[cfg(test)]
mod test_ppt;
mod text_style;

pub(crate) use slides::read_ppt_presentation;

use crate::compound::{open_compound_file, read_stream, u16_at, u32_at};
use crate::text::TextBuffer;
use crate::{Limits, OfficeError};

const HEADER_LENGTH: usize = 8;
const CONTAINER_VERSION: u16 = 0xF;
const TEXT_CHARS_ATOM: u16 = 0x0FA0;
const TEXT_BYTES_ATOM: u16 = 0x0FA8;
/// The slide masters hold the template's placeholder prompts, not content.
const MAIN_MASTER: u16 = 0x03F8;
const HANDOUT: u16 = 0x0FC9;
const SLIDE_LIST_WITH_TEXT: u16 = 0x0FF0;
/// The `SlideListWithText` instance that lists the masters.
const MASTER_LIST_INSTANCE: u16 = 1;

/// The text of a .ppt: every text atom outside the masters, each on its own lines.
pub(crate) fn extract_ppt_text(
    bytes: &[u8],
    limits: &Limits,
    buffer: &mut TextBuffer,
) -> Result<(), OfficeError> {
    let mut compound_file = open_compound_file(bytes)?;
    if compound_file.is_stream("/EncryptedSummary") {
        return Err(OfficeError::Encrypted);
    }
    let stream = read_stream(
        &mut compound_file,
        "/PowerPoint Document",
        limits.max_member_bytes,
    )?
    .ok_or_else(|| {
        OfficeError::UnreadableDocument("the file has no PowerPoint Document stream".to_owned())
    })?;
    append_record_text(&stream, buffer)
}

/// Walk the records linearly: a container's children follow its header, so stepping into a
/// container is moving past its header, and skipping one is moving past its body.
fn append_record_text(stream: &[u8], buffer: &mut TextBuffer) -> Result<(), OfficeError> {
    match u16_at(stream, 0) {
        Some(version_and_instance) if version_and_instance & 0xF == CONTAINER_VERSION => {}
        _ => {
            return Err(OfficeError::UnreadableDocument(
                "the PowerPoint Document stream does not start with a record container".to_owned(),
            ));
        }
    }

    let mut position = 0_usize;
    while let (Some(version_and_instance), Some(kind), Some(length)) = (
        u16_at(stream, position),
        u16_at(stream, position + 2),
        u32_at(stream, position + 4),
    ) {
        let body_start = position + HEADER_LENGTH;
        let Some(body_end) = body_start
            .checked_add(length as usize)
            .filter(|end| *end <= stream.len())
        else {
            break;
        };
        let version = version_and_instance & 0xF;
        let instance = version_and_instance >> 4;

        if version == CONTAINER_VERSION {
            let skipped = kind == MAIN_MASTER
                || kind == HANDOUT
                || (kind == SLIDE_LIST_WITH_TEXT && instance == MASTER_LIST_INSTANCE);
            position = if skipped { body_end } else { body_start };
            continue;
        }

        let body = &stream[body_start..body_end];
        match kind {
            TEXT_CHARS_ATOM => {
                let units = body
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes(*pair));
                append_atom(
                    char::decode_utf16(units)
                        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER)),
                    buffer,
                );
            }
            // Each byte is the low byte of a UTF-16 code unit whose high byte is zero.
            TEXT_BYTES_ATOM => append_atom(body.iter().map(|byte| char::from(*byte)), buffer),
            _ => {}
        }
        if buffer.is_truncated() {
            break;
        }
        position = body_end;
    }
    Ok(())
}

/// One text atom: `\r` ends a paragraph and `\v` breaks a line; the atom ends a line itself.
/// An atom that is only `*` is the stand-in of a slide number or date field, not text.
fn append_atom(characters: impl Iterator<Item = char>, buffer: &mut TextBuffer) {
    let text: String = characters
        .filter_map(|character| match character {
            '\r' | '\u{0B}' => Some('\n'),
            '\t' => Some('\t'),
            control if control.is_control() => None,
            other => Some(other),
        })
        .collect();
    if text.is_empty() || text.trim() == "*" {
        return;
    }
    buffer.push_str(&text);
    if !buffer.ends_with_newline() {
        buffer.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{build_compound_file, build_ppt, build_ppt_from_stream, ppt_record};

    fn extract(bytes: &[u8], limits: &Limits) -> Result<String, OfficeError> {
        let mut buffer = TextBuffer::new(limits.max_text_chars);
        extract_ppt_text(bytes, limits, &mut buffer)?;
        Ok(buffer.into_text())
    }

    #[test]
    fn reads_bytes_and_chars_atoms_and_skips_the_masters() {
        let bytes = build_ppt(&["Titolo citt\u{e0}", "Punto \u{20ac}\rSecondo", "*", "Fine"]);
        assert_eq!(
            extract(&bytes, &Limits::default()).unwrap(),
            "Titolo citt\u{e0}\nPunto \u{20ac}\nSecondo\nFine\n"
        );
    }

    #[test]
    fn reads_text_inside_drawing_containers() {
        let text: Vec<u8> = "Casella"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let textbox = ppt_record(0xF, 0, 0xF00D, &ppt_record(0, 0, TEXT_CHARS_ATOM, &text));
        let slide = ppt_record(0xF, 0, 0x03EE, &ppt_record(0xF, 0, 0xF002, &textbox));
        let bytes = build_ppt_from_stream(&slide);
        assert_eq!(extract(&bytes, &Limits::default()).unwrap(), "Casella\n");
    }

    #[test]
    fn stops_at_a_record_that_claims_more_than_the_stream() {
        let mut stream = ppt_record(0xF, 0, 0x03E8, &ppt_record(0, 0, TEXT_BYTES_ATOM, b"Ok"));
        stream.extend(ppt_record(0, 0, TEXT_BYTES_ATOM, b"x"));
        let last = stream.len() - 5;
        stream[last..last + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let bytes = build_ppt_from_stream(&stream);
        assert_eq!(extract(&bytes, &Limits::default()).unwrap(), "Ok\n");
    }

    #[test]
    fn stops_at_the_character_limit() {
        let limits = Limits {
            max_text_chars: 4,
            ..Limits::default()
        };
        assert_eq!(
            extract(&build_ppt(&["Diapositiva"]), &limits).unwrap(),
            "Diap"
        );
    }

    #[test]
    fn refuses_a_stream_that_is_not_records() {
        let bytes = build_ppt_from_stream(b"garbage!");
        assert!(matches!(
            extract(&bytes, &Limits::default()).unwrap_err(),
            OfficeError::UnreadableDocument(_)
        ));
    }

    #[test]
    fn refuses_an_encrypted_presentation() {
        let bytes = build_compound_file(&[
            (
                "/PowerPoint Document",
                ppt_record(0xF, 0, 0x03E8, &[]).as_slice(),
            ),
            ("/EncryptedSummary", b"x".as_slice()),
        ]);
        assert!(matches!(
            extract(&bytes, &Limits::default()).unwrap_err(),
            OfficeError::Encrypted
        ));
    }

    #[test]
    fn refuses_a_compound_file_without_the_presentation_stream() {
        let bytes = build_compound_file(&[("/Altro", b"x".as_slice())]);
        assert!(matches!(
            extract(&bytes, &Limits::default()).unwrap_err(),
            OfficeError::UnreadableDocument(_)
        ));
    }
}
