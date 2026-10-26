// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The Typst world of one render: the generated source, the files it names and the fonts, all
//! in memory.

use std::collections::HashMap;
use std::path::PathBuf;

use typst::Library;
use typst::World;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;

pub(crate) const MAIN_PATH: &str = "/main.typ";

pub(crate) struct OfficeWorld<'a> {
    library: &'a LazyHash<Library>,
    book: &'a LazyHash<FontBook>,
    fonts: &'a [Font],
    main: Source,
    files: HashMap<FileId, Bytes>,
}

impl<'a> OfficeWorld<'a> {
    pub(crate) fn new(
        library: &'a LazyHash<Library>,
        book: &'a LazyHash<FontBook>,
        fonts: &'a [Font],
        source: &str,
        files: &[(String, Vec<u8>)],
    ) -> Self {
        let main = Source::new(file_id(MAIN_PATH), source.to_owned());
        let files = files
            .iter()
            .map(|(path, bytes)| (file_id(path), Bytes::new(bytes.clone())))
            .collect();
        Self {
            library,
            book,
            fonts,
            main,
            files,
        }
    }
}

/// The id of a path inside the project; ids are interned for the life of the process, and the
/// paths this crate writes are a fixed few.
pub(crate) fn file_id(path: &str) -> FileId {
    let path = VirtualPath::new(path).unwrap_or_else(|_| {
        VirtualPath::new(MAIN_PATH).unwrap_or_else(|_| unreachable!("a constant path"))
    });
    RootedPath::new(VirtualRoot::Project, path).intern()
}

impl World for OfficeWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else {
            Err(FileError::NotSource)
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main.id() {
            return Ok(Bytes::from_string(self.main.text().to_owned()));
        }
        self.files
            .get(&id)
            .cloned()
            .ok_or_else(|| FileError::NotFound(PathBuf::from(id.vpath().get_with_slash())))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}
