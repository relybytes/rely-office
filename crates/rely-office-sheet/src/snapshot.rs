// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! A whole sheet as it is, to put back where it was: what undoing its removal needs. The sheet
//! travels as a workbook of its own, written as xlsx, so everything a file keeps comes back with
//! it: cells, looks, merges, rules, validations, comments and threads, charts, pictures, pivot
//! tables, its own names, sizes, frozen panes and filter.

use std::collections::{BTreeMap, HashMap, HashSet};

use rely_office_core::{CarriedPart, CellStyle, ConditionalFormat, DataValidation, Limits};

use crate::address::CellRef;
use crate::model::{EditError, MAX_SHEETS, Sheet, Workbook};
use crate::value::Locale;
use crate::xlsx::{read_spreadsheet, write_xlsx};

/// The most bytes a snapshot takes, so an undo of a removal stays a bounded message.
pub const MAX_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;

/// A sheet written to put back later: the xlsx of a workbook holding only it, with what a file
/// does not say exactly: the language of the captions of each of its pivot tables, and the look
/// of each cell, the conditional formats and the validations as the workbook kept them rather
/// than as a file spells them out.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetSnapshot {
    pub xlsx: Vec<u8>,
    pub pivot_locales: Vec<Locale>,
    /// The looks its cells use, each once.
    pub styles: Vec<CellStyle>,
    /// Each cell with a look, as `(row, column, look)`, the look an index into `styles`.
    pub cell_styles: Vec<(u32, u32, u32)>,
    pub conditional_formats: Vec<ConditionalFormat>,
    pub validations: Vec<DataValidation>,
}

impl Workbook {
    /// A sheet as it is, with its own names, for [`Workbook::restore_sheet`].
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] past [`MAX_SNAPSHOT_BYTES`].
    pub fn sheet_snapshot(&self, sheet: usize) -> Result<SheetSnapshot, EditError> {
        let Some(source) = self.sheets.get(sheet) else {
            return Err(EditError::NoSuchSheet(sheet));
        };
        let mut alone = Workbook::from_parts(vec![source.clone()], self.styles.clone());
        alone.theme.clone_from(&self.theme);
        alone.names = self
            .names
            .iter()
            .filter(|defined| defined.sheet == Some(sheet))
            .map(|defined| crate::model::DefinedName {
                sheet: Some(0),
                ..defined.clone()
            })
            .collect();
        let xlsx = write_xlsx(&alone).map_err(|_| EditError::OutOfBounds)?;
        if xlsx.len() > MAX_SNAPSHOT_BYTES {
            return Err(EditError::OutOfBounds);
        }
        let mut styles: Vec<CellStyle> = Vec::new();
        let mut index_of: HashMap<u32, u32> = HashMap::new();
        let cell_styles = source
            .styles
            .iter()
            .map(|(cell, id)| {
                let index = *index_of.entry(*id).or_insert_with(|| {
                    styles.push(self.styles.get(*id as usize).cloned().unwrap_or_default());
                    u32::try_from(styles.len() - 1).unwrap_or(u32::MAX)
                });
                (cell.row, cell.column, index)
            })
            .collect();
        Ok(SheetSnapshot {
            xlsx,
            pivot_locales: source
                .pivot_tables
                .iter()
                .map(|table| table.locale)
                .collect(),
            styles,
            cell_styles,
            conditional_formats: source.conditional_formats.clone(),
            validations: source.validations.clone(),
        })
    }

    /// Put a sheet back at `index`, as [`Workbook::sheet_snapshot`] took it: the sheets from
    /// `index` on move one place right, and the formulas of the workbook that name it read it
    /// again. Parts of its drawing or pivot tables whose names another sheet took since are
    /// renamed.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`] for an index past the end, [`EditError::OutOfBounds`] past
    /// [`MAX_SHEETS`] or for a snapshot that is not one sheet, and
    /// [`EditError::DuplicateSheetName`] when a sheet took its name since.
    pub fn restore_sheet(
        &mut self,
        index: usize,
        snapshot: &SheetSnapshot,
    ) -> Result<(), EditError> {
        if index > self.sheets.len() {
            return Err(EditError::NoSuchSheet(index));
        }
        if self.sheets.len() >= MAX_SHEETS || snapshot.xlsx.len() > MAX_SNAPSHOT_BYTES {
            return Err(EditError::OutOfBounds);
        }
        let limits = Limits {
            max_input_bytes: MAX_SNAPSHOT_BYTES as u64,
            ..Limits::default()
        };
        let read = read_spreadsheet(&snapshot.xlsx, Some("sheet.xlsx"), &limits, Locale::English)
            .map_err(|_| EditError::OutOfBounds)?;
        let Workbook { sheets, names, .. } = read;
        let Ok([mut sheet]) = <[Sheet; 1]>::try_from(sheets) else {
            return Err(EditError::OutOfBounds);
        };
        self.check_new_name(&sheet.name, None)?;
        let mut looks = BTreeMap::new();
        for (row, column, index) in &snapshot.cell_styles {
            let Some(style) = snapshot.styles.get(*index as usize) else {
                return Err(EditError::OutOfBounds);
            };
            looks.insert(
                CellRef::new(*row, *column),
                self.intern_style(style.clone()),
            );
        }
        sheet.styles = looks;
        sheet
            .conditional_formats
            .clone_from(&snapshot.conditional_formats);
        sheet.validations.clone_from(&snapshot.validations);
        for (table, locale) in sheet.pivot_tables.iter_mut().zip(&snapshot.pivot_locales) {
            table.locale = *locale;
        }
        freshen_parts(&mut sheet, self);
        for defined in &mut self.names {
            if let Some(scope) = &mut defined.sheet
                && *scope >= index
            {
                *scope += 1;
            }
        }
        self.names.extend(
            names
                .into_iter()
                .filter(|defined| defined.sheet == Some(0))
                .map(|defined| crate::model::DefinedName {
                    sheet: Some(index),
                    ..defined
                }),
        );
        self.sheets.insert(index, sheet);
        self.values.insert(index, HashMap::new());
        self.spills.insert(index, BTreeMap::new());
        self.recalculate();
        Ok(())
    }
}

/// Give the parts a restored sheet carries names no other sheet uses, and its pivot caches ids
/// no other uses, rewriting the relationships between its parts to follow.
fn freshen_parts(sheet: &mut Sheet, workbook: &Workbook) {
    let mut taken: HashSet<String> = workbook
        .sheets
        .iter()
        .flat_map(|other| {
            other
                .drawing
                .iter()
                .flat_map(|drawing| drawing.parts.iter())
                .chain(other.pivots.iter().flat_map(|pivot| pivot.parts.iter()))
                .map(|part| part.name.clone())
        })
        .collect();
    let own: Vec<String> = parts_of(sheet).map(|part| part.name.clone()).collect();
    taken.extend(own.iter().cloned());
    let mut renamed: HashMap<String, String> = HashMap::new();
    for name in &own {
        if is_relationships(name) || !collides(name, workbook) {
            continue;
        }
        let fresh = fresh_name(name, &taken);
        taken.insert(fresh.clone());
        renamed.insert(name.clone(), fresh);
    }
    let ids: HashSet<u32> = workbook
        .sheets
        .iter()
        .flat_map(|other| other.pivots.iter().map(|pivot| pivot.cache_id))
        .collect();
    let mut next_id = ids.iter().max().map_or(1, |id| id + 1);
    let mut reid: Vec<(String, u32, u32)> = Vec::new();
    for pivot in &mut sheet.pivots {
        if ids.contains(&pivot.cache_id) {
            reid.push((pivot.table_part.clone(), pivot.cache_id, next_id));
            pivot.cache_id = next_id;
            next_id += 1;
        }
    }
    if renamed.is_empty() && reid.is_empty() {
        return;
    }
    let rename = |name: &str| renamed.get(name).cloned();
    if let Some(drawing) = sheet.drawing.as_mut() {
        if let Some(fresh) = rename(&drawing.drawing_part) {
            drawing.drawing_part = fresh;
        }
        rewrite_parts(&mut drawing.parts, &renamed);
    }
    for pivot in &mut sheet.pivots {
        if let Some(fresh) = rename(&pivot.table_part) {
            pivot.table_part = fresh;
        }
        if let Some(fresh) = rename(&pivot.cache_part) {
            pivot.cache_part = fresh;
        }
        rewrite_parts(&mut pivot.parts, &renamed);
    }
    for (table_part, old, new) in reid {
        let table_part = rename(&table_part).unwrap_or(table_part);
        for part in sheet
            .pivots
            .iter_mut()
            .flat_map(|pivot| pivot.parts.iter_mut())
            .filter(|part| part.name == table_part)
        {
            if let Ok(xml) = std::str::from_utf8(&part.bytes) {
                part.bytes = xml
                    .replace(
                        &format!(r#"cacheId="{old}""#),
                        &format!(r#"cacheId="{new}""#),
                    )
                    .into_bytes();
            }
        }
    }
}

fn parts_of(sheet: &Sheet) -> impl Iterator<Item = &CarriedPart> {
    sheet
        .drawing
        .iter()
        .flat_map(|drawing| drawing.parts.iter())
        .chain(sheet.pivots.iter().flat_map(|pivot| pivot.parts.iter()))
}

fn collides(name: &str, workbook: &Workbook) -> bool {
    workbook
        .sheets
        .iter()
        .any(|other| parts_of(other).any(|part| part.name == name))
}

fn is_relationships(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rels"))
}

/// `xl/charts/chart3.xml` numbered past every name taken: `xl/charts/chart7.xml`.
fn fresh_name(name: &str, taken: &HashSet<String>) -> String {
    let (folder, file) = name.rsplit_once('/').unwrap_or(("", name));
    let (stem, extension) = file.rsplit_once('.').unwrap_or((file, ""));
    let stem = stem.trim_end_matches(|character: char| character.is_ascii_digit());
    // One of the first `taken.len() + 1` numbers is free.
    (1..=taken.len() + 1)
        .map(|number| {
            let file = if extension.is_empty() {
                format!("{stem}{number}")
            } else {
                format!("{stem}{number}.{extension}")
            };
            if folder.is_empty() {
                file
            } else {
                format!("{folder}/{file}")
            }
        })
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or_else(|| name.to_owned())
}

/// The relationships part of a part, `xl/charts/_rels/chart1.xml.rels`.
fn relationships_of(name: &str) -> String {
    let (folder, file) = name.rsplit_once('/').unwrap_or(("", name));
    if folder.is_empty() {
        format!("_rels/{file}.rels")
    } else {
        format!("{folder}/_rels/{file}.rels")
    }
}

/// Rename the parts of a group and point their relationships at the new names: a target names
/// the part it leads to by its file, which renaming within its folder changes alone.
fn rewrite_parts(parts: &mut [CarriedPart], renamed: &HashMap<String, String>) {
    let files: Vec<(String, String)> = renamed
        .iter()
        .map(|(old, new)| {
            let file = |name: &str| {
                name.rsplit_once('/')
                    .map_or(name, |(_, file)| file)
                    .to_owned()
            };
            (file(old), file(new))
        })
        .collect();
    let relationships: HashMap<String, String> = renamed
        .iter()
        .map(|(old, new)| (relationships_of(old), relationships_of(new)))
        .collect();
    for part in parts {
        if let Some(fresh) = renamed
            .get(&part.name)
            .or_else(|| relationships.get(&part.name))
        {
            part.name.clone_from(fresh);
        }
        if !is_relationships(&part.name) {
            continue;
        }
        let Ok(xml) = std::str::from_utf8(&part.bytes) else {
            continue;
        };
        let mut xml = xml.to_owned();
        for (old, new) in &files {
            xml = xml
                .replace(&format!(r#"/{old}""#), &format!(r#"/{new}""#))
                .replace(&format!(r#"Target="{old}""#), &format!(r#"Target="{new}""#));
        }
        part.bytes = xml.into_bytes();
    }
}

#[cfg(test)]
mod tests;
