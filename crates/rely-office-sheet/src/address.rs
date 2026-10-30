// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Cell addresses: zero-based positions, and the A1 notation of formulas and files.

/// Excel's grid: 1,048,576 rows by 16,384 columns.
pub const MAX_ROWS: u32 = 1_048_576;
pub const MAX_COLUMNS: u32 = 16_384;

/// A cell position, zero-based: row 0 column 0 is A1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellRef {
    pub row: u32,
    pub column: u32,
}

impl CellRef {
    #[must_use]
    pub const fn new(row: u32, column: u32) -> Self {
        Self { row, column }
    }

    /// The position of an A1 address (`B7`, `$B$7`), if it is one inside the grid.
    #[must_use]
    pub fn parse(address: &str) -> Option<Self> {
        let (cell, _, _, rest) = parse_cell(address.as_bytes())?;
        rest.is_empty().then_some(cell)
    }

    /// The A1 address, relative.
    #[must_use]
    pub fn a1(self) -> String {
        format!("{}{}", column_name(self.column), self.row + 1)
    }
}

/// The letters of a zero-based column: 0 is A, 25 Z, 26 AA.
#[must_use]
pub fn column_name(column: u32) -> String {
    let mut column = column + 1;
    let mut letters = Vec::new();
    while column > 0 {
        let remainder = (column - 1) % 26;
        letters.push(char::from(b'A' + u8::try_from(remainder).unwrap_or(0)));
        column = (column - 1) / 26;
    }
    letters.iter().rev().collect()
}

/// The zero-based column of letters, case-insensitive, if inside the grid.
#[must_use]
pub fn column_index(letters: &str) -> Option<u32> {
    if letters.is_empty() || letters.len() > 3 {
        return None;
    }
    let mut column: u32 = 0;
    for letter in letters.bytes() {
        if !letter.is_ascii_alphabetic() {
            return None;
        }
        column = column * 26 + u32::from(letter.to_ascii_uppercase() - b'A') + 1;
    }
    (column <= MAX_COLUMNS).then(|| column - 1)
}

/// A cell address at the start of `text`: the cell, whether its column and row are absolute,
/// and the rest of the text.
pub(crate) fn parse_cell(text: &[u8]) -> Option<(CellRef, bool, bool, &[u8])> {
    let mut position = 0;
    let absolute_column = text.first() == Some(&b'$');
    if absolute_column {
        position += 1;
    }
    let letters_start = position;
    while position < text.len() && text[position].is_ascii_alphabetic() {
        position += 1;
    }
    let letters = std::str::from_utf8(&text[letters_start..position]).ok()?;
    let column = column_index(letters)?;
    let absolute_row = text.get(position) == Some(&b'$');
    if absolute_row {
        position += 1;
    }
    let digits_start = position;
    while position < text.len() && text[position].is_ascii_digit() {
        position += 1;
    }
    if digits_start == position || text[digits_start] == b'0' {
        return None;
    }
    let row: u32 = std::str::from_utf8(&text[digits_start..position])
        .ok()?
        .parse()
        .ok()?;
    if row > MAX_ROWS {
        return None;
    }
    Some((
        CellRef::new(row - 1, column),
        absolute_column,
        absolute_row,
        &text[position..],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_columns_both_ways() {
        for (index, name) in [
            (0, "A"),
            (25, "Z"),
            (26, "AA"),
            (701, "ZZ"),
            (702, "AAA"),
            (16_383, "XFD"),
        ] {
            assert_eq!(column_name(index), name);
            assert_eq!(column_index(name), Some(index));
        }
        assert_eq!(column_index("xfd"), Some(16_383));
        assert_eq!(column_index("XFE"), None);
        assert_eq!(column_index(""), None);
        assert_eq!(column_index("AAAA"), None);
        assert_eq!(column_index("A1"), None);
    }

    #[test]
    fn parses_a1_addresses() {
        assert_eq!(CellRef::parse("A1"), Some(CellRef::new(0, 0)));
        assert_eq!(CellRef::parse("$c$10"), Some(CellRef::new(9, 2)));
        assert_eq!(
            CellRef::parse("XFD1048576"),
            Some(CellRef::new(1_048_575, 16_383))
        );
        for wrong in ["", "A", "1", "A0", "A01", "A1048577", "A1B", "$$A1", "A$$1"] {
            assert_eq!(CellRef::parse(wrong), None, "{wrong}");
        }
        assert_eq!(CellRef::new(6, 1).a1(), "B7");
        let (cell, absolute_column, absolute_row, rest) = parse_cell(b"$B7:C9").unwrap();
        assert_eq!(
            (cell, absolute_column, absolute_row, rest),
            (CellRef::new(6, 1), true, false, &b":C9"[..])
        );
    }
}
