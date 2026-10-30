// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

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

    /// The container is sound but the document inside it cannot be read: a structure this library
    /// does not follow, or one that points outside its own data.
    #[error("unreadable document: {0}")]
    UnreadableDocument(String),

    /// The document is password protected; its content is encrypted.
    #[error("the document is encrypted")]
    Encrypted,
}

impl OfficeError {
    /// A stable code for the error, for callers that branch on it instead of on the message.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedFormat(_) => "UNSUPPORTED_FORMAT",
            Self::DamagedContainer(_) => "DAMAGED_CONTAINER",
            Self::TooLarge { .. } => "TOO_LARGE",
            Self::MalformedXml(_) => "MALFORMED_XML",
            Self::UnreadableWorkbook(_) => "UNREADABLE_WORKBOOK",
            Self::UnreadableDocument(_) => "UNREADABLE_DOCUMENT",
            Self::Encrypted => "ENCRYPTED",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gives_every_error_a_stable_code() {
        let codes = [
            OfficeError::UnsupportedFormat(DocumentFormat::Unknown).code(),
            OfficeError::DamagedContainer(String::new()).code(),
            OfficeError::TooLarge {
                what: String::new(),
                limit: 0,
            }
            .code(),
            OfficeError::MalformedXml(String::new()).code(),
            OfficeError::UnreadableWorkbook(String::new()).code(),
            OfficeError::UnreadableDocument(String::new()).code(),
            OfficeError::Encrypted.code(),
        ];
        assert_eq!(
            codes,
            [
                "UNSUPPORTED_FORMAT",
                "DAMAGED_CONTAINER",
                "TOO_LARGE",
                "MALFORMED_XML",
                "UNREADABLE_WORKBOOK",
                "UNREADABLE_DOCUMENT",
                "ENCRYPTED",
            ]
        );
    }
}
