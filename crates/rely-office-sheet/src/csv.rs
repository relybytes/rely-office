// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! A sheet saved as delimited text, the way a spreadsheet exports one: each cell as it shows,
//! formulas as their results.

use crate::address::CellRef;
use crate::model::{EditError, Workbook};
use crate::value::Locale;

/// The separator a locale's spreadsheets write: `;` where the comma is the decimal mark.
#[must_use]
pub fn default_separator(locale: Locale) -> char {
    match locale {
        Locale::Italian => ';',
        Locale::English => ',',
    }
}

/// One sheet of the workbook as UTF-8 text with a byte order mark, so Excel reads its accents,
/// and CRLF line ends. Each cell is written as it shows in `locale`, a formula as its result,
/// and quoted when it holds the separator, a quote or a line break. A spilled cell is written
/// with its value; the rows and columns run to the last cell that shows something.
///
/// # Errors
///
/// [`EditError::NoSuchSheet`] for a sheet the workbook does not have.
pub fn write_csv(
    workbook: &Workbook,
    sheet: usize,
    locale: Locale,
    separator: char,
) -> Result<Vec<u8>, EditError> {
    let content = workbook
        .sheets()
        .get(sheet)
        .ok_or(EditError::NoSuchSheet(sheet))?;
    let mut last_row = None;
    let mut last_column = 0;
    let mut extend = |cell: CellRef| {
        last_row = Some(last_row.map_or(cell.row, |row: u32| row.max(cell.row)));
        last_column = last_column.max(cell.column);
    };
    for (cell, _) in content.cells() {
        extend(cell);
    }
    for (_, range) in workbook.spills(sheet) {
        extend(CellRef::new(range.last_row, range.last_column));
    }
    let mut text = String::from('\u{feff}');
    let Some(last_row) = last_row else {
        return Ok(text.into_bytes());
    };
    for row in 0..=last_row {
        let mut line = Vec::with_capacity(last_column as usize + 1);
        for column in 0..=last_column {
            line.push(quoted(
                &workbook.display(sheet, CellRef::new(row, column), locale),
                separator,
            ));
        }
        while line.last().is_some_and(String::is_empty) {
            line.pop();
        }
        text.push_str(&line.join(&separator.to_string()));
        text.push_str("\r\n");
    }
    Ok(text.into_bytes())
}

/// A cell's text, quoted with its quotes doubled when it would otherwise split the line.
fn quoted(text: &str, separator: char) -> String {
    let needs_quotes = text.contains(separator)
        || text.contains('"')
        || text.contains('\n')
        || text.contains('\r')
        || text.starts_with(' ')
        || text.ends_with(' ');
    if needs_quotes {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests;
