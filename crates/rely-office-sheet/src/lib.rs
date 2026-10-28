// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! An editable spreadsheet: the model behind a sheet editor.
//!
//! A [`Workbook`] holds sheets of cells; each cell keeps what was typed ([`Input`]) and how its
//! number is shown ([`NumberFormat`]). Every edit recalculates the formulas, so
//! [`Workbook::value`] and [`Workbook::display`] always answer with current results. Formulas
//! are parsed into expression trees, which is how inserting and deleting rows and columns,
//! renaming sheets and pasting move their references the way a spreadsheet does.
//!
//! [`read_spreadsheet`] opens xlsx, xls, ods, csv and tsv; [`write_xlsx`] saves as xlsx with the
//! computed values cached next to each formula.

mod address;
mod dates;
mod display;
mod edit;
mod evaluate;
mod formula;
mod history;
mod looks;
mod model;
mod ods;
mod rules;
mod styling;
mod value;
mod xlsx;

pub use address::{CellRef, MAX_COLUMNS, MAX_ROWS, column_index, column_name};
pub use dates::{date_from_serial, serial_from_date};
pub use formula::{
    BinaryOperator, CellAddress, Expr, FormulaError, MAX_FORMULA_CHARS, RangeAddress,
    UnaryOperator, parse_formula, parse_formula_in, print_formula, print_formula_in,
};
pub use history::{EditHistory, MAX_UNDO_CELLS, MAX_UNDO_STEPS};
pub use looks::{CellLook, DataBarLook, IconLook};
pub use model::{Cell, EditError, MAX_CELLS, MAX_SHEETS, NumberFormat, Sheet, Workbook};
pub use ods::write_ods;
pub use rely_office_core::{
    Border, BorderStyle, CellBorders, CellFont, CellRange, CellStyle, ComparisonOperator,
    Condition, ConditionalFormat, ConditionalRule, DataValidation, DifferentialLook, ErrorStyle,
    HorizontalAlignment, ScalePoint, ScaleValue, TextOperator, ValidationKind, VerticalAlignment,
};
pub use rules::MAX_RULES;
pub use styling::{
    BorderPlacement, MAX_MERGES, MAX_ROW_HEIGHT, MAX_STYLED_AREA, MAX_STYLED_CELLS, StylePatch,
};
pub use value::{ErrorKind, Input, Locale, Value};
pub use xlsx::{read_spreadsheet, write_xlsx};
