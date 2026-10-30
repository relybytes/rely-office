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
//! computed values cached next to each formula, [`write_ods`] as ods and [`write_csv`] one sheet
//! as delimited text.

mod address;
mod anchors;
mod arrange;
mod charts;
mod csv;
mod dates;
mod display;
mod edit;
mod evaluate;
mod fill;
mod formula;
mod history;
mod looks;
mod model;
mod ods;
mod pivots;
mod region;
mod rules;
mod styling;
mod value;
mod xlsx;

pub use address::{CellRef, MAX_COLUMNS, MAX_ROWS, column_index, column_name};
pub use arrange::{AutoFilter, MAX_FILTER_CHOICES, MAX_SORT_KEYS, SortKey};
pub use charts::{ChartPlacement, MAX_CHARTS, SheetChart};
pub use csv::{default_separator, write_csv};
pub use dates::{date_from_serial, serial_from_date};
pub use fill::MAX_FILL_CELLS;
pub use formula::{
    BinaryOperator, CellAddress, Expr, FUNCTION_NAMES, FormulaError, MAX_FORMULA_CHARS,
    RangeAddress, UnaryOperator, function_names, parse_formula, parse_formula_in, print_formula,
    print_formula_in,
};
pub use history::{EditHistory, MAX_UNDO_CELLS, MAX_UNDO_STEPS};
pub use looks::{CellLook, DataBarLook, IconLook};
pub use model::{
    Cell, Comment, CommentReply, DefinedName, EditError, MAX_CELLS, MAX_COMMENT_CHARS,
    MAX_COMMENT_REPLIES, MAX_SHEETS, NumberFormat, Sheet, Workbook,
};
pub use ods::write_ods;
pub use pivots::{
    MAX_PIVOT_COLUMN_KEYS, MAX_PIVOT_FIELDS, MAX_PIVOT_ROW_KEYS, MAX_PIVOT_SOURCE_CELLS,
    MAX_PIVOT_TABLES, PivotField, PivotFunction, PivotLayout, PivotSubtotals, PivotTable,
    PivotValue, SourceField, item_label,
};
pub use region::{MAX_REGION_CELLS, Region, RegionCell, RegionComment, RegionReply};
pub use rely_office_core::{
    Border, BorderStyle, CellBorders, CellFont, CellRange, CellStyle, ChartSeriesSpec, ChartSpec,
    ChartType, ComparisonOperator, Condition, ConditionalFormat, ConditionalRule, DataValidation,
    DifferentialLook, ErrorStyle, HorizontalAlignment, ScalePoint, ScaleValue, TextOperator,
    ValidationKind, VerticalAlignment,
};
pub use rules::MAX_RULES;
pub use styling::{
    BorderPlacement, MAX_MERGES, MAX_ROW_HEIGHT, MAX_STYLED_AREA, MAX_STYLED_CELLS, StylePatch,
};
pub use value::{ErrorKind, Input, Locale, Value};
pub use xlsx::{read_spreadsheet, write_xlsx};
