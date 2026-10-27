// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The text of a Word 97-2003 document ([MS-DOC]): the piece table (`Clx`) in the table stream
//! says where each run of characters lies in the `WordDocument` stream and how it is encoded.

mod comments;
mod fib;
mod fkp;
mod fonts;
mod lists;
mod pictures;
mod properties;
mod sections;
mod sprm;
mod styles;
#[cfg(test)]
mod test_doc;
mod tree;

pub(crate) use tree::read_doc_tree;

use crate::compound::{open_compound_file, read_stream, u16_at, u32_at};
use crate::text::TextBuffer;
use crate::{Limits, OfficeError};
use fib::{Fib, pair};

const COMPRESSED_FLAG: u32 = 0x4000_0000;
const FC_MASK: u32 = 0x3FFF_FFFF;

/// One piece of the piece table: characters `[start, end)` stored from `offset`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Piece {
    start: usize,
    end: usize,
    offset: usize,
    compressed: bool,
}

/// The text of every story of a .doc, paragraphs on their own lines and table cells tab-ended;
/// a field keeps its result and drops its code.
pub(crate) fn extract_doc_text(
    bytes: &[u8],
    limits: &Limits,
    buffer: &mut TextBuffer,
) -> Result<(), OfficeError> {
    let mut compound_file = open_compound_file(bytes)?;
    let word_document = read_stream(&mut compound_file, "/WordDocument", limits.max_member_bytes)?
        .ok_or_else(|| unreadable("the file has no WordDocument stream"))?;
    let fib = Fib::read(&word_document)?;
    if fib.encrypted {
        return Err(OfficeError::Encrypted);
    }
    let table = read_stream(
        &mut compound_file,
        fib.table_stream,
        limits.max_member_bytes,
    )?
    .ok_or_else(|| unreadable("the file has no table stream"))?;
    let clx = fib
        .slice(&table, pair::PIECE_TABLE)
        .ok_or_else(|| unreadable("the piece table lies outside the table stream"))?;
    let total_characters = fib.stories.total();

    let mut filter = SpecialCharacterFilter::default();
    for piece in read_piece_table(clx)? {
        if piece.start >= total_characters {
            break;
        }
        let count = piece.end.min(total_characters) - piece.start;
        for character in decode_piece(&word_document, &piece, count)? {
            if let Some(character) = filter.translate(character) {
                buffer.push(character);
            }
        }
        if buffer.is_truncated() {
            break;
        }
    }
    Ok(())
}

fn unreadable(reason: &str) -> OfficeError {
    OfficeError::UnreadableDocument(reason.to_owned())
}

/// The `Clx`: property runs (`Prc`, skipped) followed by the piece table (`Pcdt`).
fn read_piece_table(clx: &[u8]) -> Result<Vec<Piece>, OfficeError> {
    let mut position = 0_usize;
    loop {
        match clx.get(position) {
            Some(0x01) => {
                let length = u16_at(clx, position + 1)
                    .ok_or_else(|| unreadable("a property run of the piece table is cut short"))?;
                position += 3 + usize::from(length);
            }
            Some(0x02) => {
                let length = u32_at(clx, position + 1)
                    .ok_or_else(|| unreadable("the piece table is cut short"))?
                    as usize;
                let start = position + 5;
                let table = start
                    .checked_add(length)
                    .and_then(|end| clx.get(start..end))
                    .ok_or_else(|| unreadable("the piece table is cut short"))?;
                return read_pieces(table);
            }
            _ => return Err(unreadable("the document has no piece table")),
        }
    }
}

/// `PlcPcd`: `n + 1` character positions, then `n` piece descriptors of eight bytes.
fn read_pieces(table: &[u8]) -> Result<Vec<Piece>, OfficeError> {
    if table.len() < 4 || !(table.len() - 4).is_multiple_of(12) {
        return Err(unreadable("the piece table has an impossible size"));
    }
    let count = (table.len() - 4) / 12;
    let descriptors = (count + 1) * 4;
    let mut pieces = Vec::with_capacity(count);
    for index in 0..count {
        let start = u32_at(table, index * 4).map(|value| value as usize);
        let end = u32_at(table, index * 4 + 4).map(|value| value as usize);
        let raw_offset = u32_at(table, descriptors + index * 8 + 2);
        let (Some(start), Some(end), Some(raw_offset)) = (start, end, raw_offset) else {
            return Err(unreadable("the piece table is cut short"));
        };
        if end < start {
            return Err(unreadable("the piece table goes backwards"));
        }
        let compressed = raw_offset & COMPRESSED_FLAG != 0;
        let offset = (raw_offset & FC_MASK) as usize;
        pieces.push(Piece {
            start,
            end,
            offset: if compressed { offset / 2 } else { offset },
            compressed,
        });
    }
    Ok(pieces)
}

/// The first `count` characters of a piece: Windows-1252 bytes when compressed, UTF-16LE
/// otherwise.
fn decode_piece(
    word_document: &[u8],
    piece: &Piece,
    count: usize,
) -> Result<Vec<char>, OfficeError> {
    let width = if piece.compressed { 1 } else { 2 };
    let bytes = count
        .checked_mul(width)
        .and_then(|length| piece.offset.checked_add(length))
        .and_then(|end| word_document.get(piece.offset..end))
        .ok_or_else(|| unreadable("a text piece lies outside the WordDocument stream"))?;

    if piece.compressed {
        return Ok(encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(bytes)
            .0
            .chars()
            .collect());
    }
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair));
    Ok(char::decode_utf16(units)
        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect())
}

/// Word's in-band marks: paragraph and cell ends, breaks, and fields (`0x13` code `0x14` result
/// `0x15`), whose code is dropped and whose result is kept.
#[derive(Debug, Default)]
struct SpecialCharacterFilter {
    /// One entry per open field: whether its result has started.
    fields: Vec<bool>,
}

impl SpecialCharacterFilter {
    fn translate(&mut self, character: char) -> Option<char> {
        match character {
            '\u{13}' => {
                self.fields.push(false);
                return None;
            }
            '\u{14}' => {
                if let Some(in_result) = self.fields.last_mut() {
                    *in_result = true;
                }
                return None;
            }
            '\u{15}' => {
                self.fields.pop();
                return None;
            }
            _ => {}
        }
        if self.fields.last() == Some(&false) {
            return None;
        }
        match character {
            '\r' | '\u{0B}' | '\u{0C}' => Some('\n'),
            '\u{07}' | '\t' => Some('\t'),
            '\u{1E}' => Some('-'),
            control if control.is_control() => None,
            other => Some(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{DocOptions, build_compound_file, build_doc, build_doc_with};

    fn extract(bytes: &[u8], limits: &Limits) -> Result<String, OfficeError> {
        let mut buffer = TextBuffer::new(limits.max_text_chars);
        extract_doc_text(bytes, limits, &mut buffer)?;
        Ok(buffer.into_text())
    }

    #[test]
    fn reads_compressed_and_unicode_pieces_in_order() {
        let bytes = build_doc(&[
            ("Citt\u{e0} di Arona\r", true),
            ("\u{20ac} 10 \u{3b1}\r", false),
        ]);
        assert_eq!(
            extract(&bytes, &Limits::default()).unwrap(),
            "Citt\u{e0} di Arona\n\u{20ac} 10 \u{3b1}\n"
        );
    }

    #[test]
    fn decodes_compressed_pieces_as_windows_1252() {
        let bytes = build_doc(&[("\u{201c}virgolette\u{201d} \u{2013} \u{20ac}\r", true)]);
        assert_eq!(
            extract(&bytes, &Limits::default()).unwrap(),
            "\u{201c}virgolette\u{201d} \u{2013} \u{20ac}\n"
        );
    }

    #[test]
    fn keeps_field_results_and_drops_field_codes() {
        let bytes = build_doc(&[(
            "Vedi \u{13} HYPERLINK \"https://x\" \u{14}il sito\u{15}.\u{13}PAGE\u{15}\r",
            false,
        )]);
        assert_eq!(
            extract(&bytes, &Limits::default()).unwrap(),
            "Vedi il sito.\n"
        );
    }

    #[test]
    fn turns_cells_and_breaks_into_tabs_and_lines() {
        let bytes = build_doc(&[("A\u{07}B\u{07}\u{07}riga\u{0B}due\u{0C}\u{01}fine\r", true)]);
        assert_eq!(
            extract(&bytes, &Limits::default()).unwrap(),
            "A\tB\t\triga\ndue\nfine\n"
        );
    }

    #[test]
    fn finds_the_piece_table_in_the_0table_stream() {
        let options = DocOptions {
            zero_table: true,
            ..DocOptions::default()
        };
        let bytes = build_doc_with(&[("Zero\r", true)], options);
        assert_eq!(extract(&bytes, &Limits::default()).unwrap(), "Zero\n");
    }

    #[test]
    fn stops_at_the_character_limit() {
        let limits = Limits {
            max_text_chars: 3,
            ..Limits::default()
        };
        assert_eq!(
            extract(&build_doc(&[("Contratto\r", false)]), &limits).unwrap(),
            "Con"
        );
    }

    #[test]
    fn refuses_an_encrypted_document() {
        let options = DocOptions {
            encrypted: true,
            ..DocOptions::default()
        };
        let bytes = build_doc_with(&[("Segreto\r", true)], options);
        assert!(matches!(
            extract(&bytes, &Limits::default()).unwrap_err(),
            OfficeError::Encrypted
        ));
    }

    #[test]
    fn refuses_a_piece_table_outside_the_table_stream() {
        let options = DocOptions {
            broken_clx: true,
            ..DocOptions::default()
        };
        let bytes = build_doc_with(&[("Testo\r", true)], options);
        assert!(matches!(
            extract(&bytes, &Limits::default()).unwrap_err(),
            OfficeError::UnreadableDocument(_)
        ));
    }

    #[test]
    fn refuses_a_stream_that_is_not_word_97() {
        let bytes = build_compound_file(&[("/WordDocument", [0_u8; 64].as_slice())]);
        assert!(matches!(
            extract(&bytes, &Limits::default()).unwrap_err(),
            OfficeError::UnreadableDocument(_)
        ));

        let mut word_95 = vec![0_u8; 64];
        word_95[0..2].copy_from_slice(&0xA5EC_u16.to_le_bytes());
        word_95[2..4].copy_from_slice(&0x0065_u16.to_le_bytes());
        let bytes = build_compound_file(&[("/WordDocument", word_95.as_slice())]);
        assert!(matches!(
            extract(&bytes, &Limits::default()).unwrap_err(),
            OfficeError::UnreadableDocument(_)
        ));
    }

    #[test]
    fn refuses_a_cut_file_information_block_without_panicking() {
        let full = build_doc(&[("Testo\r", true)]);
        let mut compound_file = open_compound_file(&full).unwrap();
        let word_document = read_stream(&mut compound_file, "/WordDocument", u64::MAX)
            .unwrap()
            .unwrap();
        for length in [4, 33, 63, 100, 153, 400] {
            let bytes = build_compound_file(&[("/WordDocument", &word_document[..length])]);
            assert!(
                extract(&bytes, &Limits::default()).is_err(),
                "length {length}"
            );
        }
    }

    #[test]
    fn refuses_a_piece_outside_the_word_document_stream() {
        let pieces = read_pieces(&[0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0]).unwrap();
        assert!(matches!(
            decode_piece(&[0; 16], &pieces[0], 9).unwrap_err(),
            OfficeError::UnreadableDocument(_)
        ));
    }

    #[test]
    fn refuses_malformed_piece_tables() {
        assert!(read_piece_table(&[]).is_err());
        assert!(read_piece_table(&[0x07]).is_err());
        assert!(read_piece_table(&[0x01, 0xFF]).is_err());
        assert!(read_piece_table(&[0x02, 0xFF, 0xFF, 0, 0]).is_err());
        assert!(read_pieces(&[0, 0, 0]).is_err());
        assert!(read_pieces(&[5, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).is_err());
    }

    #[test]
    fn refuses_a_compound_file_without_a_word_document_stream() {
        let bytes = build_compound_file(&[("/Altro", b"x".as_slice())]);
        assert!(matches!(
            extract(&bytes, &Limits::default()).unwrap_err(),
            OfficeError::UnreadableDocument(_)
        ));
    }

    #[test]
    fn refuses_a_word_document_stream_above_the_member_cap() {
        let limits = Limits {
            max_member_bytes: 100,
            ..Limits::default()
        };
        assert!(matches!(
            extract(&build_doc(&[("Testo\r", true)]), &limits).unwrap_err(),
            OfficeError::TooLarge { .. }
        ));
    }
}
