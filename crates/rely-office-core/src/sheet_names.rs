// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The names a workbook defines and the comments its cells carry, as an xlsx and an ods keep
//! them: `definedName` in the workbook part and `table:named-expressions` in the content, the
//! `comments` part of a sheet and the `office:annotation` of a cell.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::archive::{ZipPackage, open_package, read_text_member};
use crate::opendocument::excel_formula_from_odf;
use crate::relationships::{main_part_path, read_relationships};
use crate::sheet_layout::CellRange;
use crate::xml_text::attribute_value;
use crate::xml_tree::{Element, Namespace, parse_tree};
use crate::{DocumentFormat, Limits, OfficeError, detect_format};

/// No workbook keeps more names than this, nor a sheet more comments.
const MAX_NAMES: usize = 10_000;
const MAX_COMMENTS: usize = 100_000;
/// The longest comment kept, in characters.
const MAX_COMMENT_CHARS: usize = 32_767;
const WORKBOOK_PART: &str = "xl/workbook.xml";

/// A name a workbook defines: what it refers to in Excel's notation without the `=`, the sheet
/// it belongs to by position, `None` for the whole workbook, and whether the file hides it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbookName {
    pub name: String,
    pub sheet: Option<usize>,
    pub formula: String,
    pub hidden: bool,
}

/// A comment on a cell: who wrote it and its text, paragraphs separated by line breaks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellComment {
    pub row: u32,
    pub column: u32,
    pub author: String,
    pub text: String,
}

/// The names an xlsx or an ods defines, those Excel keeps for itself (`_xlnm.`) left out; a file
/// it cannot read gives none.
#[must_use]
pub fn read_workbook_names(bytes: &[u8], limits: &Limits) -> Vec<WorkbookName> {
    match detect_format(bytes, None) {
        DocumentFormat::Xlsx => read_xlsx_names(bytes, limits).unwrap_or_default(),
        DocumentFormat::Ods => read_ods_names(bytes, limits).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn read_xlsx_names(bytes: &[u8], limits: &Limits) -> Result<Vec<WorkbookName>, OfficeError> {
    let mut package = open_package(bytes)?;
    let workbook_part = main_part_path(&mut package, limits, WORKBOOK_PART)?;
    let Some(workbook) = read_text_member(&mut package, &workbook_part, limits)? else {
        return Ok(Vec::new());
    };
    xlsx_names(&workbook)
}

fn xlsx_names(workbook: &str) -> Result<Vec<WorkbookName>, OfficeError> {
    let mut reader = Reader::from_str(workbook);
    let mut names = Vec::new();
    let mut reading: Option<WorkbookName> = None;
    loop {
        match reader.read_event().map_err(malformed)? {
            Event::Start(element) if element.local_name().as_ref() == "definedName" => {
                let attribute = |name: &str| {
                    element
                        .attributes()
                        .flatten()
                        .find(|attribute| attribute.key.as_ref() == name)
                        .and_then(|attribute| attribute_value(&attribute))
                };
                reading = attribute("name").map(|name| WorkbookName {
                    name,
                    sheet: attribute("localSheetId").and_then(|index| index.parse().ok()),
                    formula: String::new(),
                    hidden: matches!(attribute("hidden").as_deref(), Some("1" | "true")),
                });
            }
            Event::Text(text) => {
                if let Some(name) = &mut reading {
                    name.formula.push_str(&text.xml10_content());
                }
            }
            Event::GeneralRef(reference) => {
                if let Some(name) = &mut reading {
                    let resolved = match reference.resolve_char_ref() {
                        Ok(Some(character)) => Some(character.to_string()),
                        _ => quick_xml::escape::resolve_xml_entity(&reference).map(str::to_owned),
                    };
                    if let Some(resolved) = resolved {
                        name.formula.push_str(&resolved);
                    }
                }
            }
            Event::End(element) if element.local_name().as_ref() == "definedName" => {
                if let Some(name) = reading.take()
                    && !name.name.starts_with("_xlnm.")
                    && names.len() < MAX_NAMES
                {
                    names.push(name);
                }
            }
            Event::Eof => return Ok(names),
            _ => {}
        }
    }
}

fn read_ods_names(bytes: &[u8], limits: &Limits) -> Result<Vec<WorkbookName>, OfficeError> {
    let mut package = open_package(bytes)?;
    let Some(content) = read_text_member(&mut package, "content.xml", limits)? else {
        return Ok(Vec::new());
    };
    let content = parse_tree(&content, |_, _| false)?;
    let Some(spreadsheet) = content
        .child(Namespace::OdfOffice, "body")
        .and_then(|body| body.child(Namespace::OdfOffice, "spreadsheet"))
    else {
        return Ok(Vec::new());
    };
    let mut names = Vec::new();
    ods_named_expressions(spreadsheet, None, &mut names);
    for (index, table) in spreadsheet
        .elements()
        .filter(|table| table.is(Namespace::OdfTable, "table"))
        .enumerate()
    {
        ods_named_expressions(table, Some(index), &mut names);
    }
    Ok(names)
}

/// The names of a `table:named-expressions` list: a named range refers to its cells, a named
/// expression to its formula.
fn ods_named_expressions(parent: &Element, sheet: Option<usize>, names: &mut Vec<WorkbookName>) {
    let Some(list) = parent.child(Namespace::OdfTable, "named-expressions") else {
        return;
    };
    for named in list.elements() {
        if names.len() >= MAX_NAMES {
            return;
        }
        let Some(name) = named.attribute(Namespace::OdfTable, "name") else {
            continue;
        };
        let formula = if named.is(Namespace::OdfTable, "named-range") {
            named
                .attribute(Namespace::OdfTable, "cell-range-address")
                .map(|address| excel_formula_from_odf(&format!("[{address}]")))
        } else if named.is(Namespace::OdfTable, "named-expression") {
            named
                .attribute(Namespace::OdfTable, "expression")
                .map(excel_formula_from_odf)
        } else {
            None
        };
        if let Some(formula) = formula.filter(|formula| !formula.is_empty()) {
            names.push(WorkbookName {
                name: name.to_owned(),
                sheet,
                formula,
                hidden: false,
            });
        }
    }
}

/// The comments of a worksheet part of an xlsx, from the `comments` part it relates to.
pub(crate) fn xlsx_sheet_comments(
    package: &mut ZipPackage<'_>,
    sheet_part: &str,
    limits: &Limits,
) -> Result<Vec<CellComment>, OfficeError> {
    let Some(target) = read_relationships(package, sheet_part, limits)?
        .into_iter()
        .find(|relationship| !relationship.external && relationship.is_kind("comments"))
        .map(|relationship| relationship.target)
    else {
        return Ok(Vec::new());
    };
    let Some(xml) = read_text_member(package, &target, limits)? else {
        return Ok(Vec::new());
    };
    xlsx_comments(&xml)
}

fn xlsx_comments(xml: &str) -> Result<Vec<CellComment>, OfficeError> {
    let root = parse_tree(xml, |_, name| name == "t")?;
    let local = |element: &Element, name: &str| element.name == name;
    let authors: Vec<String> = root
        .elements()
        .find(|element| local(element, "authors"))
        .map(|authors| authors.elements().map(Element::text).collect())
        .unwrap_or_default();
    let Some(list) = root
        .elements()
        .find(|element| local(element, "commentList"))
    else {
        return Ok(Vec::new());
    };
    let mut comments = Vec::new();
    for comment in list.elements().filter(|element| local(element, "comment")) {
        if comments.len() >= MAX_COMMENTS {
            break;
        }
        let Some(range) = attribute_of(comment, "ref").and_then(|cell| CellRange::parse(&cell))
        else {
            continue;
        };
        let author = attribute_of(comment, "authorId")
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| authors.get(index))
            .cloned()
            .unwrap_or_default();
        let mut text = String::new();
        if let Some(body) = comment.elements().find(|element| local(element, "text")) {
            run_text(body, &mut text);
        }
        // Excel opens a comment with its author's name in bold, a line of its own.
        let text = text
            .strip_prefix(&format!("{author}:\n"))
            .or_else(|| text.strip_prefix(&format!("{author}:")))
            .map_or_else(|| text.clone(), str::to_owned);
        comments.push(CellComment {
            row: range.first_row,
            column: range.first_column,
            author,
            text: truncate(text.trim_end_matches('\n')),
        });
    }
    Ok(comments)
}

/// The text of a comment body: the `t` of each run, phonetic guides left out.
fn run_text(element: &Element, text: &mut String) {
    for child in element.elements() {
        match child.name.as_str() {
            "t" => text.push_str(&child.text()),
            "rPh" => {}
            _ => run_text(child, text),
        }
    }
}

/// The comment of an ods cell, its `office:annotation`.
pub(crate) fn ods_cell_comment(cell: &Element, row: u32, column: u32) -> Option<CellComment> {
    let annotation = cell.child(Namespace::OdfOffice, "annotation")?;
    let author = annotation
        .elements()
        .find(|child| child.name == "creator")
        .map(Element::text)
        .unwrap_or_default();
    let paragraphs: Vec<String> = annotation
        .elements()
        .filter(|child| child.is(Namespace::OdfText, "p"))
        .map(paragraph_text)
        .collect();
    Some(CellComment {
        row,
        column,
        author,
        text: truncate(paragraphs.join("\n").trim_end_matches('\n')),
    })
}

/// The text of a paragraph, spans and line breaks included.
fn paragraph_text(element: &Element) -> String {
    let mut text = String::new();
    for node in &element.children {
        match node {
            crate::xml_tree::Node::Text(value) => text.push_str(value),
            crate::xml_tree::Node::Element(child) => match child.name.as_str() {
                "line-break" => text.push('\n'),
                "tab" => text.push('\t'),
                "s" => {
                    let count = child
                        .attribute(Namespace::OdfText, "c")
                        .and_then(|count| count.parse::<usize>().ok())
                        .unwrap_or(1)
                        .min(1_000);
                    text.push_str(&" ".repeat(count));
                }
                _ => text.push_str(&paragraph_text(child)),
            },
        }
    }
    text
}

fn attribute_of(element: &Element, name: &str) -> Option<String> {
    element.attribute(Namespace::None, name).map(str::to_owned)
}

fn truncate(text: &str) -> String {
    text.chars().take(MAX_COMMENT_CHARS).collect()
}

fn malformed(error: impl std::fmt::Display) -> OfficeError {
    OfficeError::MalformedXml(error.to_string())
}

#[cfg(test)]
mod tests;
