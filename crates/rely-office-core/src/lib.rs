// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Reads office documents for previews and search, without an office suite.
//!
//! Three entry points, each bounded so an uploaded file can never exhaust memory:
//!
//! - [`detect_format`] tells what a file is from its bytes, never from its name alone;
//! - [`extract_text`] returns the text a search index needs;
//! - [`read_workbook`] returns the sheets of a spreadsheet as tables of displayed values.
//!
//! Every input is treated as untrusted: archive members are read through a size cap, and no
//! function panics on malformed bytes.

mod error;
mod format;
mod limits;
mod sheet;
mod text;
mod word;

#[cfg(test)]
mod test_support;

pub use error::OfficeError;
pub use format::{DocumentFormat, detect_format};
pub use limits::Limits;
pub use sheet::{Sheet, Workbook, read_workbook};
pub use text::extract_text;
