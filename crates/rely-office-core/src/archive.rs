// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Bounded reads of the members of a zip package (OOXML and `OpenDocument`).

use std::io::{Cursor, Read};

use crate::{Limits, OfficeError};

pub(crate) type ZipPackage<'a> = zip::ZipArchive<Cursor<&'a [u8]>>;

/// Open the bytes as a zip package.
pub(crate) fn open_package(bytes: &[u8]) -> Result<ZipPackage<'_>, OfficeError> {
    zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| OfficeError::DamagedContainer(error.to_string()))
}

/// The names of the members, in the order the archive lists them.
pub(crate) fn member_names(package: &ZipPackage<'_>) -> Vec<String> {
    package
        .file_names()
        .filter_map(Result::ok)
        .map(std::borrow::Cow::into_owned)
        .collect()
}

/// A member read whole as UTF-8 through the member cap; a missing member is `None`, not an error.
pub(crate) fn read_text_member(
    package: &mut ZipPackage<'_>,
    name: &str,
    limits: &Limits,
) -> Result<Option<String>, OfficeError> {
    let Some(bytes) = read_member_bytes(package, name, limits.max_member_bytes)? else {
        return Ok(None);
    };
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| OfficeError::MalformedXml(format!("{name} is not UTF-8")))
}

/// A member read whole through `cap`; a missing member is `None`, a larger one an error.
pub(crate) fn read_member_bytes(
    package: &mut ZipPackage<'_>,
    name: &str,
    cap: u64,
) -> Result<Option<Vec<u8>>, OfficeError> {
    let member = match package.by_name(name) {
        Ok(member) => member,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(OfficeError::DamagedContainer(error.to_string())),
    };

    let mut bytes = Vec::new();
    // One byte past the cap tells a member at the limit from one above it.
    member
        .take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| OfficeError::DamagedContainer(error.to_string()))?;
    if bytes.len() as u64 > cap {
        return Err(OfficeError::TooLarge {
            what: name.to_owned(),
            limit: cap,
        });
    }

    Ok(Some(bytes))
}

/// Resolve a relationship target against the folder of the part that declares it, as OPC does:
/// an absolute target starts at the package root, `..` climbs one folder.
pub(crate) fn resolve_part_path(source_part: &str, target: &str) -> String {
    let target = target.split(['#', '?']).next().unwrap_or_default();
    if let Some(absolute) = target.strip_prefix('/') {
        return normalize_segments(absolute.split('/'));
    }
    let folder = source_part
        .rsplit_once('/')
        .map_or("", |(folder, _)| folder);
    normalize_segments(folder.split('/').chain(target.split('/')))
}

fn normalize_segments<'a>(segments: impl Iterator<Item = &'a str>) -> String {
    let mut resolved: Vec<&str> = Vec::new();
    for segment in segments {
        match segment {
            "" | "." => {}
            ".." => {
                resolved.pop();
            }
            other => resolved.push(other),
        }
    }
    resolved.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::build_docx;

    #[test]
    fn reads_a_member_and_misses_an_absent_one() {
        let bytes = build_docx(&["Ciao"]);
        let mut package = open_package(&bytes).unwrap();
        let limits = Limits::default();
        assert!(
            read_text_member(&mut package, "word/document.xml", &limits)
                .unwrap()
                .unwrap()
                .contains("Ciao")
        );
        assert!(
            read_text_member(&mut package, "word/missing.xml", &limits)
                .unwrap()
                .is_none()
        );
        assert!(member_names(&package).contains(&"word/document.xml".to_owned()));
    }

    #[test]
    fn refuses_a_member_above_the_cap() {
        let bytes = build_docx(&["Ciao"]);
        let mut package = open_package(&bytes).unwrap();
        let error = read_member_bytes(&mut package, "word/document.xml", 8).unwrap_err();
        assert!(matches!(error, OfficeError::TooLarge { limit: 8, .. }));
    }

    #[test]
    fn refuses_a_member_that_is_not_utf8() {
        let bytes = crate::test_support::build_zip(&[("a.xml", b"\xff\xfe".as_slice())]);
        let mut package = open_package(&bytes).unwrap();
        let error = read_text_member(&mut package, "a.xml", &Limits::default()).unwrap_err();
        assert!(matches!(error, OfficeError::MalformedXml(_)));
    }

    #[test]
    fn resolves_relationship_targets_like_opc() {
        assert_eq!(
            resolve_part_path("word/document.xml", "media/image1.png"),
            "word/media/image1.png"
        );
        assert_eq!(
            resolve_part_path("word/document.xml", "/word/media/a.png"),
            "word/media/a.png"
        );
        assert_eq!(
            resolve_part_path("ppt/slides/slide1.xml", "../notesSlides/notesSlide1.xml"),
            "ppt/notesSlides/notesSlide1.xml"
        );
        assert_eq!(
            resolve_part_path("ppt/presentation.xml", "slides/slide2.xml"),
            "ppt/slides/slide2.xml"
        );
        assert_eq!(resolve_part_path("a.xml", "../../b.xml#x"), "b.xml");
    }
}
