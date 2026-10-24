// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use crate::DocumentFormat;

/// Why a document could not be read.
#[derive(Debug, thiserror::Error)]
pub enum OfficeError {
    /// The bytes are not a format this library reads for that operation.
    #[error("unsupported format: {0:?}")]
    UnsupportedFormat(DocumentFormat),

    /// The container (zip or compound file) is damaged or not what its signature says.
    #[error("damaged container: {0}")]
    DamagedContainer(String),

    /// A member of the container, or the whole input, is larger than the configured cap.
    #[error("{what} exceeds the limit of {limit} bytes")]
    TooLarge { what: String, limit: u64 },

    /// The XML inside the document is malformed.
    #[error("malformed xml: {0}")]
    MalformedXml(String),

    /// The spreadsheet reader refused the workbook.
    #[error("unreadable workbook: {0}")]
    UnreadableWorkbook(String),
}
