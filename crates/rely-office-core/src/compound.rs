// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! Bounded reads of the streams of a compound file (the container of doc, xls and ppt).

use std::io::{Cursor, Read};

use crate::OfficeError;

pub(crate) type CompoundFile<'a> = cfb::CompoundFile<Cursor<&'a [u8]>>;

/// Open the bytes as a compound file.
pub(crate) fn open_compound_file(bytes: &[u8]) -> Result<CompoundFile<'_>, OfficeError> {
    cfb::CompoundFile::open(Cursor::new(bytes))
        .map_err(|error| OfficeError::DamagedContainer(error.to_string()))
}

/// A stream read whole through `cap`; a missing stream is `None`, a larger one an error.
pub(crate) fn read_stream(
    compound_file: &mut CompoundFile<'_>,
    path: &str,
    cap: u64,
) -> Result<Option<Vec<u8>>, OfficeError> {
    if !compound_file.is_stream(path) {
        return Ok(None);
    }
    let too_large = || OfficeError::TooLarge {
        what: path.trim_start_matches('/').to_owned(),
        limit: cap,
    };
    let declared = compound_file
        .entry(path)
        .map_err(|error| OfficeError::DamagedContainer(error.to_string()))?
        .len();
    if declared > cap {
        return Err(too_large());
    }

    let stream = compound_file
        .open_stream(path)
        .map_err(|error| OfficeError::DamagedContainer(error.to_string()))?;
    let mut bytes = Vec::new();
    stream
        .take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| OfficeError::DamagedContainer(error.to_string()))?;
    if bytes.len() as u64 > cap {
        return Err(too_large());
    }
    Ok(Some(bytes))
}

/// A little-endian `u16` at `offset`, or `None` past the end.
pub(crate) fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

/// A little-endian `u32` at `offset`, or `None` past the end.
pub(crate) fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::build_compound_file;

    #[test]
    fn reads_a_stream_and_misses_an_absent_one() {
        let bytes = build_compound_file(&[("/Dati", b"abc".as_slice())]);
        let mut compound_file = open_compound_file(&bytes).unwrap();
        assert_eq!(
            read_stream(&mut compound_file, "/Dati", 3).unwrap(),
            Some(b"abc".to_vec())
        );
        assert_eq!(read_stream(&mut compound_file, "/Altro", 3).unwrap(), None);
    }

    #[test]
    fn refuses_a_stream_above_the_cap() {
        let bytes = build_compound_file(&[("/Dati", b"abcd".as_slice())]);
        let mut compound_file = open_compound_file(&bytes).unwrap();
        assert!(matches!(
            read_stream(&mut compound_file, "/Dati", 3).unwrap_err(),
            OfficeError::TooLarge { limit: 3, .. }
        ));
    }

    #[test]
    fn refuses_bytes_that_are_not_a_compound_file() {
        assert!(matches!(
            open_compound_file(b"not a compound file").unwrap_err(),
            OfficeError::DamagedContainer(_)
        ));
    }

    #[test]
    fn reads_integers_without_reading_past_the_end() {
        assert_eq!(u16_at(&[1, 2, 3], 1), Some(0x0302));
        assert_eq!(u16_at(&[1, 2], 1), None);
        assert_eq!(u32_at(&[1, 0, 0, 0], 0), Some(1));
        assert_eq!(u32_at(&[1, 0, 0], 0), None);
        assert_eq!(u32_at(&[], usize::MAX), None);
    }
}
