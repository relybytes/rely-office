// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A document tree written as an `OpenDocument` text, the odt `LibreOffice` opens and the reader
//! of this crate reads back to the same tree.
//!
//! Each look a paragraph or a run sets itself becomes an automatic style, written once and
//! named by every paragraph or span that shares it; headings, quotes, code and code runs take
//! the named styles `LibreOffice` uses for them. The page and the headers and footers of each
//! section are a master page; notes, comments with their replies, tracked insertions and
//! deletions and page number fields are written as `OpenDocument` has them. An equation is
//! written as its LaTeX, and an embedded object as its picture.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Cursor, Write as _};

use zip::write::SimpleFileOptions;

use crate::OfficeError;
use crate::document::{
    Alignment, Anchor, AnchorAlign, AnchorBase, Block, Comment, DocumentTree, HeadersFooters,
    Inline, LineSpacing, Mark, Media, NoteKind, PageField, PageLayout, ParagraphLook, TableCell,
    TableRow, Wrap,
};

const NAMESPACES: &str = concat!(
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
    r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" "#,
    r#"xmlns:xlink="http://www.w3.org/1999/xlink" "#,
    r#"xmlns:dc="http://purl.org/dc/elements/1.1/" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
    r#"xmlns:config="urn:oasis:names:tc:opendocument:xmlns:config:1.0" "#,
    r#"xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" "#,
    r#"xmlns:loext="urn:org:documentfoundation:names:experimental:office:xmlns:loext:1.0" "#,
    r#"office:version="1.3""#,
);

/// The named styles every written document carries, as `LibreOffice` names them.
const NAMED_STYLES: &str = concat!(
    r#"<style:style style:name="Standard" style:family="paragraph" style:class="text"/>"#,
    r#"<style:style style:name="Heading" style:family="paragraph" style:parent-style-name="Standard" style:next-style-name="Standard" style:class="text">"#,
    r#"<style:paragraph-properties fo:margin-top="12pt" fo:margin-bottom="6pt" fo:keep-with-next="always"/>"#,
    r#"<style:text-properties fo:font-weight="bold"/></style:style>"#,
    r#"<style:style style:name="Quotations" style:family="paragraph" style:parent-style-name="Standard" style:class="html">"#,
    r#"<style:paragraph-properties fo:margin-left="28.35pt" fo:margin-right="28.35pt" fo:margin-bottom="8.5pt"/></style:style>"#,
    r#"<style:style style:name="Preformatted_20_Text" style:display-name="Preformatted Text" style:family="paragraph" style:parent-style-name="Standard" style:class="html">"#,
    r#"<style:paragraph-properties fo:margin-top="0pt" fo:margin-bottom="0pt"/>"#,
    r#"<style:text-properties fo:font-family="'Liberation Mono'" style:font-family-generic="modern" style:font-pitch="fixed" fo:font-size="10pt"/></style:style>"#,
    r#"<style:style style:name="Horizontal_20_Line" style:display-name="Horizontal Line" style:family="paragraph" style:parent-style-name="Standard" style:class="html">"#,
    r##"<style:paragraph-properties fo:margin-top="0pt" fo:margin-bottom="8.5pt" fo:border-bottom="0.5pt solid #808080" fo:padding="0pt"/>"##,
    r#"<style:text-properties fo:font-size="6pt"/></style:style>"#,
    r#"<style:style style:name="Table_20_Contents" style:display-name="Table Contents" style:family="paragraph" style:parent-style-name="Standard" style:class="extra"/>"#,
    r#"<style:style style:name="Footnote" style:family="paragraph" style:parent-style-name="Standard" style:class="extra">"#,
    r#"<style:text-properties fo:font-size="10pt"/></style:style>"#,
    r#"<style:style style:name="Source_20_Text" style:display-name="Source Text" style:family="text">"#,
    r#"<style:text-properties fo:font-family="'Liberation Mono'" style:font-family-generic="modern" style:font-pitch="fixed"/></style:style>"#,
    r#"<style:style style:name="Internet_20_link" style:display-name="Internet link" style:family="text">"#,
    r##"<style:text-properties fo:color="#000080" style:text-underline-style="solid" style:text-underline-width="auto" style:text-underline-color="font-color"/></style:style>"##,
);

/// Where the automatic styles a piece of text names are declared: `content.xml` for the body
/// and its notes, `styles.xml` for headers and footers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Content,
    Styles,
}

/// The named style a paragraph's look builds on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Base {
    Standard,
    Heading(u8),
    Quote,
    Code,
    TableContents,
    Footnote,
}

impl Base {
    fn name(self) -> String {
        match self {
            Self::Standard => "Standard".to_owned(),
            Self::Heading(level) => format!("Heading_20_{level}"),
            Self::Quote => "Quotations".to_owned(),
            Self::Code => "Preformatted_20_Text".to_owned(),
            Self::TableContents => "Table_20_Contents".to_owned(),
            Self::Footnote => "Footnote".to_owned(),
        }
    }
}

/// Automatic styles written once and named by everything that shares them.
#[derive(Default)]
struct StyleRegistry {
    xml: String,
    names: HashMap<String, String>,
    count: usize,
}

impl StyleRegistry {
    /// The name of the style whose body is `key`, declaring it the first time.
    fn name(&mut self, prefix: &str, key: String, declare: impl FnOnce(&str) -> String) -> String {
        if let Some(name) = self.names.get(&key) {
            return name.clone();
        }
        self.count += 1;
        let name = format!("{prefix}{}", self.count);
        self.xml.push_str(&declare(&name));
        self.names.insert(key, name.clone());
        name
    }
}

struct OdtWriter<'t> {
    tree: &'t DocumentTree,
    content_styles: StyleRegistry,
    styles_styles: StyleRegistry,
    /// Where each image of the tree is stored in the package, written on first use.
    pictures: Vec<Option<String>>,
    picture_files: Vec<(String, &'static str, &'t [u8])>,
    notes: usize,
    frames: usize,
    tables: usize,
    lists: usize,
    changes: String,
    change_count: usize,
    /// The comments whose range is open, by index, and the name each was written with.
    open_comments: Vec<usize>,
    comment_names: HashMap<usize, String>,
    written_comments: Vec<bool>,
}

/// A document tree as an odt.
///
/// # Errors
///
/// [`OfficeError::DamagedContainer`] when the package cannot be assembled.
#[expect(
    clippy::too_many_lines,
    reason = "the parts of the package are assembled in the order they depend on each other"
)]
pub fn write_odt(tree: &DocumentTree) -> Result<Vec<u8>, OfficeError> {
    let mut writer = OdtWriter {
        tree,
        content_styles: StyleRegistry::default(),
        styles_styles: StyleRegistry::default(),
        pictures: vec![None; tree.media.len()],
        picture_files: Vec::new(),
        notes: 0,
        frames: 0,
        tables: 0,
        lists: 0,
        changes: String::new(),
        change_count: 0,
        open_comments: Vec::new(),
        comment_names: HashMap::new(),
        written_comments: vec![false; tree.comments.len()],
    };

    // The sections, each with its page and headers: the last one is the tree's own.
    let mut sections: Vec<(usize, &PageLayout, &HeadersFooters)> = tree
        .sections
        .iter()
        .map(|section| (section.end_block, &section.layout, &section.headers_footers))
        .collect();
    if sections.is_empty() {
        sections.push((tree.blocks.len(), &tree.layout, &tree.headers_footers));
    }
    let mut body = String::new();
    let mut masters = String::new();
    let mut page_layouts = String::new();
    let mut start = 0;
    for (index, (end, layout, headers)) in sections.iter().enumerate() {
        let master = if index == 0 {
            "Standard".to_owned()
        } else {
            format!("MP{index}")
        };
        let layout_name = format!("pm{}", index + 1);
        page_layouts.push_str(&page_layout_xml(&layout_name, layout, headers));
        masters.push_str(&writer.master_page(&master, &layout_name, headers));
        let end = (*end).min(tree.blocks.len()).max(start);
        let first_master = (index > 0).then_some(master.as_str());
        body.push_str(&writer.blocks(
            &tree.blocks[start..end],
            Part::Content,
            Base::Standard,
            first_master,
        ));
        start = end;
    }
    if start < tree.blocks.len() {
        body.push_str(&writer.blocks(&tree.blocks[start..], Part::Content, Base::Standard, None));
    }
    body.push_str(&writer.close_all_comments());

    let mut content = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = write!(
        content,
        "<office:document-content {NAMESPACES}><office:automatic-styles>{}</office:automatic-styles><office:body><office:text>",
        writer.content_styles.xml
    );
    if !writer.changes.is_empty() {
        let _ = write!(
            content,
            "<text:tracked-changes>{}</text:tracked-changes>",
            writer.changes
        );
    }
    content.push_str(&body);
    content.push_str("</office:text></office:body></office:document-content>");

    let styles = styles_xml(
        &tree.layout,
        &page_layouts,
        &writer.styles_styles.xml,
        &masters,
    );
    let settings = settings_xml(tree.layout.spacing_adds_up);
    let mut manifest = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">"#,
        r#"<manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.text"/>"#,
        r#"<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>"#,
        r#"<manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>"#,
        r#"<manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/>"#,
        r#"<manifest:file-entry manifest:full-path="settings.xml" manifest:media-type="text/xml"/>"#,
    ));
    for (path, content_type, _) in &writer.picture_files {
        let _ = write!(
            manifest,
            r#"<manifest:file-entry manifest:full-path="{path}" manifest:media-type="{content_type}"/>"#
        );
    }
    manifest.push_str("</manifest:manifest>");
    let meta = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-meta {NAMESPACES}><office:meta><meta:generator>rely-office/{}</meta:generator></office:meta></office:document-meta>"#,
        env!("CARGO_PKG_VERSION")
    );

    let mut members: Vec<(String, Vec<u8>)> = vec![
        ("content.xml".to_owned(), content.into_bytes()),
        ("styles.xml".to_owned(), styles.into_bytes()),
        ("meta.xml".to_owned(), meta.into_bytes()),
        ("settings.xml".to_owned(), settings.into_bytes()),
        ("META-INF/manifest.xml".to_owned(), manifest.into_bytes()),
    ];
    for (path, _, bytes) in &writer.picture_files {
        members.push((path.clone(), bytes.to_vec()));
    }
    package(&members)
}

/// The package: `mimetype` first and stored, as `OpenDocument` asks, then the parts.
fn package(members: &[(String, Vec<u8>)]) -> Result<Vec<u8>, OfficeError> {
    let damaged = |error: &dyn std::fmt::Display| OfficeError::DamagedContainer(error.to_string());
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        "mimetype",
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .map_err(|error| damaged(&error))?;
    zip.write_all(b"application/vnd.oasis.opendocument.text")
        .map_err(|error| damaged(&error))?;
    for (name, bytes) in members {
        zip.start_file(name.as_str(), SimpleFileOptions::default())
            .map_err(|error| damaged(&error))?;
        zip.write_all(bytes).map_err(|error| damaged(&error))?;
    }
    zip.finish()
        .map(Cursor::into_inner)
        .map_err(|error| damaged(&error))
}

impl OdtWriter<'_> {
    fn registry(&mut self, part: Part) -> &mut StyleRegistry {
        match part {
            Part::Content => &mut self.content_styles,
            Part::Styles => &mut self.styles_styles,
        }
    }

    fn master_page(&mut self, name: &str, layout: &str, headers: &HeadersFooters) -> String {
        let mut xml =
            format!(r#"<style:master-page style:name="{name}" style:page-layout-name="{layout}">"#);
        let mut part =
            |element: &str, blocks: Option<&Vec<crate::document::Block>>, writer: &mut Self| {
                if let Some(blocks) = blocks.filter(|blocks| !blocks.is_empty()) {
                    let content = writer.blocks(blocks, Part::Styles, Base::Standard, None);
                    let _ = write!(xml, "<style:{element}>{content}</style:{element}>");
                }
            };
        part("header", Some(&headers.header), self);
        part("header-first", headers.first_header.as_ref(), self);
        part("header-left", headers.even_header.as_ref(), self);
        part("footer", Some(&headers.footer), self);
        part("footer-first", headers.first_footer.as_ref(), self);
        part("footer-left", headers.even_footer.as_ref(), self);
        xml.push_str("</style:master-page>");
        xml
    }

    fn blocks(&mut self, blocks: &[Block], part: Part, base: Base, master: Option<&str>) -> String {
        let mut xml = String::new();
        let mut index = 0;
        let mut master = master;
        while index < blocks.len() {
            if let Block::ListItem { list_id, .. } = &blocks[index] {
                let end = index
                    + blocks[index..]
                        .iter()
                        .take_while(|block| matches!(block, Block::ListItem { list_id: id, .. } if id == list_id))
                        .count();
                xml.push_str(&self.list(&blocks[index..end], part, master.take()));
                index = end;
                continue;
            }
            xml.push_str(&self.block(&blocks[index], part, base, master.take()));
            index += 1;
        }
        xml
    }

    fn block(&mut self, block: &Block, part: Part, base: Base, master: Option<&str>) -> String {
        match block {
            Block::Heading {
                level,
                content,
                align,
                look,
            } => {
                let level = (*level).clamp(1, 6);
                let style = self.paragraph_style(part, Base::Heading(level), *align, look, master);
                let content = self.inlines(content, part);
                format!(
                    r#"<text:h text:style-name="{style}" text:outline-level="{level}">{content}</text:h>"#
                )
            }
            Block::Paragraph {
                content,
                align,
                look,
            } => {
                let style = self.paragraph_style(part, base, *align, look, master);
                let content = self.inlines(content, part);
                format!(r#"<text:p text:style-name="{style}">{content}</text:p>"#)
            }
            Block::ListItem { content, look, .. } => {
                let style = self.paragraph_style(part, base, None, look, master);
                let content = self.inlines(content, part);
                format!(r#"<text:p text:style-name="{style}">{content}</text:p>"#)
            }
            Block::Table { rows, look } => {
                self.table(rows, &look.column_widths_twips, part, master)
            }
            Block::Blockquote { blocks } => self.blocks(blocks, part, Base::Quote, master),
            Block::CodeBlock { text } => {
                let style =
                    self.paragraph_style(part, Base::Code, None, &ParagraphLook::default(), master);
                let mut xml = String::new();
                for line in text.split('\n') {
                    let _ = write!(
                        xml,
                        r#"<text:p text:style-name="{style}">{}</text:p>"#,
                        text_xml(line)
                    );
                }
                xml
            }
            Block::HorizontalRule => {
                let style = match master {
                    Some(master) => self.master_style(part, "Horizontal_20_Line", master),
                    None => "Horizontal_20_Line".to_owned(),
                };
                format!(r#"<text:p text:style-name="{style}"/>"#)
            }
        }
    }

    /// The automatic style of a paragraph that only starts a new master page.
    fn master_style(&mut self, part: Part, parent: &str, master: &str) -> String {
        let key = format!("master|{parent}|{master}");
        self.registry(part).name("P", key, |name| {
            format!(
                r#"<style:style style:name="{name}" style:family="paragraph" style:parent-style-name="{parent}" style:master-page-name="{master}"/>"#
            )
        })
    }

    /// The style a paragraph names: its base when it sets nothing itself, else an automatic
    /// style over its base with what it sets.
    fn paragraph_style(
        &mut self,
        part: Part,
        base: Base,
        align: Option<Alignment>,
        look: &ParagraphLook,
        master: Option<&str>,
    ) -> String {
        let properties = paragraph_properties(align, look);
        let text = run_look_properties(&look.font);
        if properties.is_empty() && text.is_empty() && master.is_none() {
            return base.name();
        }
        let parent = base.name();
        let master_attribute = master
            .map(|master| format!(r#" style:master-page-name="{master}""#))
            .unwrap_or_default();
        let key = format!("{parent}|{properties}|{text}|{master_attribute}");
        self.registry(part).name("P", key, |name| {
            let mut xml = format!(
                r#"<style:style style:name="{name}" style:family="paragraph" style:parent-style-name="{parent}"{master_attribute}>"#
            );
            if !properties.is_empty() {
                let _ = write!(xml, "<style:paragraph-properties{properties}/>");
            }
            if !text.is_empty() {
                let _ = write!(xml, "<style:text-properties{text}/>");
            }
            xml.push_str("</style:style>");
            xml
        })
    }

    fn list(&mut self, items: &[Block], part: Part, master: Option<&str>) -> String {
        // Each level is numbered or bulleted as its first item says.
        let mut ordered_by_level = [None; 10];
        for item in items {
            if let Block::ListItem { ordered, level, .. } = item {
                let level = usize::from(*level).min(9);
                ordered_by_level[level].get_or_insert(*ordered);
            }
        }
        let signature: String = ordered_by_level
            .iter()
            .map(|ordered| if *ordered == Some(true) { '1' } else { '-' })
            .collect();
        let style = self
            .registry(part)
            .name("L", format!("list|{signature}"), |name| {
                list_style(name, &ordered_by_level)
            });
        self.lists += 1;
        let mut xml = format!(r#"<text:list text:style-name="{style}">"#);
        // The open levels: whether an item of the level is open, its nested list going inside it.
        let mut depth = 1;
        let mut item_open = vec![false];
        let mut master = master;
        for item in items {
            let Block::ListItem { level, .. } = item else {
                continue;
            };
            let target = usize::from(*level).min(9) + 1;
            while depth > target {
                if item_open.pop() == Some(true) {
                    xml.push_str("</text:list-item>");
                }
                xml.push_str("</text:list>");
                depth -= 1;
            }
            if item_open.last() == Some(&true) && depth == target {
                xml.push_str("</text:list-item>");
                if let Some(last) = item_open.last_mut() {
                    *last = false;
                }
            }
            while depth < target {
                if item_open.last() != Some(&true) {
                    xml.push_str("<text:list-item>");
                    if let Some(last) = item_open.last_mut() {
                        *last = true;
                    }
                }
                xml.push_str("<text:list>");
                item_open.push(false);
                depth += 1;
            }
            xml.push_str("<text:list-item>");
            if let Some(last) = item_open.last_mut() {
                *last = true;
            }
            xml.push_str(&self.block(item, part, Base::Standard, master.take()));
        }
        while let Some(open) = item_open.pop() {
            if open {
                xml.push_str("</text:list-item>");
            }
            xml.push_str("</text:list>");
        }
        xml
    }

    fn table(
        &mut self,
        rows: &[TableRow],
        widths: &[u32],
        part: Part,
        master: Option<&str>,
    ) -> String {
        self.tables += 1;
        let columns = rows
            .iter()
            .map(|row| {
                row.cells
                    .iter()
                    .map(|cell| cell.col_span.max(1) as usize)
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0)
            .max(widths.len())
            .max(1);
        let total: u32 = widths.iter().sum();
        let table_style = {
            let width = (total > 0).then(|| format!(r#" style:width="{}""#, length(total)));
            let master_attribute = master
                .map(|master| format!(r#" style:master-page-name="{master}""#))
                .unwrap_or_default();
            let width = width.unwrap_or_default();
            let key = format!("table|{width}|{master_attribute}");
            self.registry(part).name("Table", key, |name| {
                format!(
                    r#"<style:style style:name="{name}" style:family="table"{master_attribute}><style:table-properties{width} table:align="margins"/></style:style>"#
                )
            })
        };
        let mut xml = format!(
            r#"<table:table table:name="Tabella{}" table:style-name="{table_style}">"#,
            self.tables
        );
        for column in 0..columns {
            match widths.get(column) {
                Some(width) if *width > 0 => {
                    let width = *width;
                    let style = self.registry(part).name("Col", format!("column|{width}"), |name| {
                        format!(
                            r#"<style:style style:name="{name}" style:family="table-column"><style:table-column-properties style:column-width="{}"/></style:style>"#,
                            length(width)
                        )
                    });
                    let _ = write!(xml, r#"<table:table-column table:style-name="{style}"/>"#);
                }
                _ => xml.push_str("<table:table-column/>"),
            }
        }
        let cell_style = self.registry(part).name("Cell", "cell".to_owned(), |name| {
            format!(
                r#"<style:style style:name="{name}" style:family="table-cell"><style:table-cell-properties fo:padding="2pt" fo:border="0.5pt solid #000000"/></style:style>"#
            )
        });
        // The cells a span above or beside covers, by row and column.
        let mut covered: Vec<Vec<bool>> = vec![vec![false; columns]; rows.len()];
        let header_rows = rows.iter().take_while(|row| row.header).count();
        for (row_index, row) in rows.iter().enumerate() {
            if row_index == 0 && header_rows > 0 {
                xml.push_str("<table:table-header-rows>");
            }
            xml.push_str("<table:table-row>");
            let mut column = 0;
            let mut cells = row.cells.iter();
            while column < columns {
                if covered[row_index][column] {
                    xml.push_str("<table:covered-table-cell/>");
                    column += 1;
                    continue;
                }
                let Some(cell) = cells.next() else {
                    let _ = write!(
                        xml,
                        r#"<table:table-cell table:style-name="{cell_style}" office:value-type="string"/>"#
                    );
                    column += 1;
                    continue;
                };
                xml.push_str(&self.cell(cell, part, &cell_style));
                let col_span = (cell.col_span.max(1) as usize).min(columns - column);
                let row_span = (cell.row_span.max(1) as usize).min(rows.len() - row_index);
                for covered_row in covered.iter_mut().skip(row_index).take(row_span) {
                    for slot in covered_row.iter_mut().skip(column).take(col_span) {
                        *slot = true;
                    }
                }
                covered[row_index][column] = false;
                for _ in 1..col_span {
                    xml.push_str("<table:covered-table-cell/>");
                }
                column += col_span;
            }
            xml.push_str("</table:table-row>");
            if row_index + 1 == header_rows {
                xml.push_str("</table:table-header-rows>");
            }
        }
        xml.push_str("</table:table>");
        xml
    }

    fn cell(&mut self, cell: &TableCell, part: Part, style: &str) -> String {
        let mut attributes = String::new();
        if cell.col_span > 1 {
            let _ = write!(
                attributes,
                r#" table:number-columns-spanned="{}""#,
                cell.col_span
            );
        }
        if cell.row_span > 1 {
            let _ = write!(
                attributes,
                r#" table:number-rows-spanned="{}""#,
                cell.row_span
            );
        }
        let mut content = self.blocks(&cell.blocks, part, Base::TableContents, None);
        if content.is_empty() {
            r#"<text:p text:style-name="Table_20_Contents"/>"#.clone_into(&mut content);
        }
        format!(
            r#"<table:table-cell table:style-name="{style}" office:value-type="string"{attributes}>{content}</table:table-cell>"#
        )
    }

    fn inlines(&mut self, content: &[Inline], part: Part) -> String {
        let mut xml = String::new();
        for inline in content {
            match inline {
                Inline::Text { text, marks, look } => {
                    if part == Part::Content {
                        xml.push_str(&self.switch_comments(marks));
                    }
                    if let Some(Mark::Deletion { author, date }) = marks
                        .iter()
                        .find(|mark| matches!(mark, Mark::Deletion { .. }))
                    {
                        if part == Part::Content {
                            let id = self.change(
                                "deletion",
                                author,
                                date.as_deref(),
                                &format!("<text:p>{}</text:p>", text_xml(text)),
                            );
                            let _ = write!(xml, r#"<text:change text:change-id="{id}"/>"#);
                        }
                        continue;
                    }
                    let inserted = marks.iter().find_map(|mark| match mark {
                        Mark::Insertion { author, date } => Some((author.clone(), date.clone())),
                        _ => None,
                    });
                    let change = match (&inserted, part) {
                        (Some((author, date)), Part::Content) => {
                            let id = self.change("insertion", author, date.as_deref(), "");
                            let _ = write!(xml, r#"<text:change-start text:change-id="{id}"/>"#);
                            Some(id)
                        }
                        _ => None,
                    };
                    let body = match look.field {
                        Some(PageField::Page) => format!(
                            r#"<text:page-number text:select-page="current">{}</text:page-number>"#,
                            text_xml(text)
                        ),
                        Some(PageField::Pages) => {
                            format!("<text:page-count>{}</text:page-count>", text_xml(text))
                        }
                        None => text_xml(text),
                    };
                    let style = self.text_style(part, marks);
                    let span = match style {
                        Some(style) => {
                            format!(r#"<text:span text:style-name="{style}">{body}</text:span>"#)
                        }
                        None => body,
                    };
                    match marks.iter().find_map(|mark| match mark {
                        Mark::Link { href } => Some(href),
                        _ => None,
                    }) {
                        Some(href) => {
                            let _ = write!(
                                xml,
                                r#"<text:a xlink:type="simple" xlink:href="{}" text:style-name="Internet_20_link">{span}</text:a>"#,
                                attribute_xml(href)
                            );
                        }
                        None => xml.push_str(&span),
                    }
                    if let Some(id) = change {
                        let _ = write!(xml, r#"<text:change-end text:change-id="{id}"/>"#);
                    }
                }
                Inline::HardBreak => xml.push_str("<text:line-break/>"),
                Inline::Image {
                    media,
                    alt,
                    width,
                    height,
                    anchor,
                } => {
                    let image = self.image(*media, alt.as_deref(), *width, *height);
                    match anchor {
                        Some(anchor) => xml.push_str(&self.floating(&image, anchor, part)),
                        None => xml.push_str(&image),
                    }
                }
                Inline::NoteReference { note } => xml.push_str(&self.note(*note)),
                Inline::CommentAnchor { comment } => {
                    if part == Part::Content {
                        xml.push_str(&self.annotation(*comment, false));
                    }
                }
                Inline::Math { latex, .. } => xml.push_str(&text_xml(latex)),
            }
        }
        xml
    }

    /// Close the comments the run is no longer under and open the ones it starts.
    fn switch_comments(&mut self, marks: &[Mark]) -> String {
        let under: Vec<usize> = marks
            .iter()
            .filter_map(|mark| match mark {
                Mark::Comment { comment } => Some(*comment),
                _ => None,
            })
            .filter(|comment| *comment < self.tree.comments.len())
            .collect();
        let mut xml = String::new();
        let closing: Vec<usize> = self
            .open_comments
            .iter()
            .copied()
            .filter(|open| !under.contains(open))
            .collect();
        for comment in closing {
            self.open_comments.retain(|open| *open != comment);
            if let Some(name) = self.comment_names.get(&comment) {
                let _ = write!(xml, r#"<office:annotation-end office:name="{name}"/>"#);
            }
        }
        for comment in under {
            if !self.open_comments.contains(&comment) && !self.written_comments[comment] {
                xml.push_str(&self.annotation(comment, true));
                self.open_comments.push(comment);
            }
        }
        xml
    }

    fn close_all_comments(&mut self) -> String {
        let mut xml = String::new();
        for comment in std::mem::take(&mut self.open_comments) {
            if let Some(name) = self.comment_names.get(&comment) {
                let _ = write!(xml, r#"<office:annotation-end office:name="{name}"/>"#);
            }
        }
        // A comment the text never refers to goes in a paragraph of its own at the end.
        let mut orphans = String::new();
        for index in 0..self.tree.comments.len() {
            if !self.written_comments[index] && self.tree.comments[index].parent.is_none() {
                orphans.push_str(&self.annotation(index, false));
            }
        }
        if !orphans.is_empty() {
            let _ = write!(xml, "<text:p>{orphans}</text:p>");
        }
        xml
    }

    fn annotation(&mut self, index: usize, ranged: bool) -> String {
        let Some(comment) = self.tree.comments.get(index) else {
            return String::new();
        };
        if self.written_comments.get(index) == Some(&true) {
            return String::new();
        }
        self.written_comments[index] = true;
        let name = format!("__Annotation__{index}");
        self.comment_names.insert(index, name.clone());
        let mut attributes = String::new();
        if ranged
            || comment.parent.is_some()
            || self
                .tree
                .comments
                .iter()
                .any(|other| other.parent == Some(index))
        {
            let _ = write!(attributes, r#" office:name="{name}""#);
        }
        if comment.resolved {
            attributes.push_str(r#" loext:resolved="true""#);
        }
        if let Some(parent) = comment.parent {
            let _ = write!(attributes, r#" loext:parent-name="__Annotation__{parent}""#);
        }
        let body = self.comment_body(comment);
        let mut xml = format!("<office:annotation{attributes}>");
        let _ = write!(
            xml,
            "<dc:creator>{}</dc:creator>",
            text_xml(&comment.author)
        );
        if let Some(date) = &comment.date {
            let _ = write!(xml, "<dc:date>{}</dc:date>", text_xml(date));
        }
        if let Some(initials) = &comment.initials {
            let _ = write!(
                xml,
                "<meta:creator-initials>{}</meta:creator-initials>",
                text_xml(initials)
            );
        }
        xml.push_str(&body);
        xml.push_str("</office:annotation>");
        if !ranged && attributes.contains("office:name") {
            let _ = write!(xml, r#"<office:annotation-end office:name="{name}"/>"#);
        }
        // Replies follow the comment they answer, at its place, marking no text of their own.
        let replies: Vec<usize> = (0..self.tree.comments.len())
            .filter(|other| self.tree.comments[*other].parent == Some(index))
            .collect();
        for reply in replies {
            xml.push_str(&self.annotation(reply, false));
        }
        xml
    }

    fn comment_body(&mut self, comment: &Comment) -> String {
        let open = std::mem::take(&mut self.open_comments);
        let body = self.blocks(&comment.blocks, Part::Content, Base::Standard, None);
        self.open_comments = open;
        if body.is_empty() {
            "<text:p/>".to_owned()
        } else {
            body
        }
    }

    /// Declare a tracked change and return its id.
    fn change(&mut self, kind: &str, author: &str, date: Option<&str>, content: &str) -> String {
        self.change_count += 1;
        let id = format!("ct{}", self.change_count);
        let mut info = format!("<dc:creator>{}</dc:creator>", text_xml(author));
        if let Some(date) = date {
            let _ = write!(info, "<dc:date>{}</dc:date>", text_xml(date));
        }
        let _ = write!(
            self.changes,
            r#"<text:changed-region text:id="{id}"><text:{kind}><office:change-info>{info}</office:change-info>{content}</text:{kind}></text:changed-region>"#
        );
        id
    }

    fn note(&mut self, note: usize) -> String {
        let Some(content) = self.tree.notes.get(note) else {
            return String::new();
        };
        self.notes += 1;
        let (class, id) = match content.kind {
            NoteKind::Footnote => ("footnote", format!("ftn{}", self.notes)),
            NoteKind::Endnote => ("endnote", format!("edn{}", self.notes)),
        };
        let open = std::mem::take(&mut self.open_comments);
        let mut body = self.blocks(&content.blocks, Part::Content, Base::Footnote, None);
        self.open_comments = open;
        if body.is_empty() {
            r#"<text:p text:style-name="Footnote"/>"#.clone_into(&mut body);
        }
        format!(
            r#"<text:note text:id="{id}" text:note-class="{class}"><text:note-citation>{}</text:note-citation><text:note-body>{body}</text:note-body></text:note>"#,
            self.notes
        )
    }

    fn image(
        &mut self,
        media: usize,
        alt: Option<&str>,
        width: Option<u32>,
        height: Option<u32>,
    ) -> String {
        let Some(item) = self.tree.media.get(media) else {
            return String::new();
        };
        let Some(path) = self.picture_path(media, item) else {
            return String::new();
        };
        let (width, height) = image_size(item, width, height);
        self.frames += 1;
        let description = alt
            .filter(|alt| !alt.is_empty())
            .map(|alt| format!("<svg:desc>{}</svg:desc>", text_xml(alt)))
            .unwrap_or_default();
        format!(
            r#"<draw:frame draw:name="Immagine{}" text:anchor-type="as-char" svg:width="{}" svg:height="{}" draw:z-index="0"><draw:image xlink:href="{path}" xlink:type="simple" xlink:show="embed" xlink:actuate="onLoad"/>{description}</draw:frame>"#,
            self.frames,
            css_pixels(width),
            css_pixels(height)
        )
    }

    /// A picture's frame made to float: anchored to the paragraph, placed as its anchor says
    /// with a graphic style giving how the text goes around it.
    fn floating(&mut self, frame: &str, anchor: &Anchor, part: Part) -> String {
        if frame.is_empty() {
            return String::new();
        }
        let horizontal_rel = match anchor.horizontal.base {
            AnchorBase::Page => "page",
            AnchorBase::Margin => "page-content",
            AnchorBase::Line => "char",
            AnchorBase::Column | AnchorBase::Paragraph => "paragraph",
        };
        let vertical_rel = match anchor.vertical.base {
            AnchorBase::Page => "page",
            AnchorBase::Margin => "page-content",
            AnchorBase::Line => "line",
            AnchorBase::Column | AnchorBase::Paragraph => "paragraph",
        };
        let horizontal_pos = match anchor.horizontal.align {
            Some(AnchorAlign::Start) => "left",
            Some(AnchorAlign::Center) => "center",
            Some(AnchorAlign::End) => "right",
            None => "from-left",
        };
        let vertical_pos = match anchor.vertical.align {
            Some(AnchorAlign::Start) => "top",
            Some(AnchorAlign::Center) => "middle",
            Some(AnchorAlign::End) => "bottom",
            None => "from-top",
        };
        let wrap = match anchor.wrap {
            Wrap::Around | Wrap::Contour(_) => r#"style:wrap="parallel""#,
            Wrap::TopAndBottom => r#"style:wrap="none""#,
            Wrap::InFront => r#"style:wrap="run-through" style:run-through="foreground""#,
            Wrap::Behind => r#"style:wrap="run-through" style:run-through="background""#,
        };
        let [top, bottom, left, right] = anchor.distance_emu.map(emu_length);
        let key = format!(
            "graphic|{horizontal_rel}|{vertical_rel}|{horizontal_pos}|{vertical_pos}|{wrap}|{top}|{bottom}|{left}|{right}"
        );
        let style = self.registry(part).name("fr", key, |name| {
            format!(
                concat!(
                    r#"<style:style style:name="{}" style:family="graphic"><style:graphic-properties {} "#,
                    r#"style:horizontal-pos="{}" style:horizontal-rel="{}" style:vertical-pos="{}" style:vertical-rel="{}" "#,
                    r#"fo:margin-top="{}" fo:margin-bottom="{}" fo:margin-left="{}" fo:margin-right="{}"/></style:style>"#
                ),
                name, wrap, horizontal_pos, horizontal_rel, vertical_pos, vertical_rel, top, bottom, left, right
            )
        });
        let position = format!(
            r#"draw:style-name="{style}" text:anchor-type="paragraph" svg:x="{}" svg:y="{}""#,
            emu_length(anchor.horizontal.offset_emu),
            emu_length(anchor.vertical.offset_emu)
        );
        let size = (anchor.width_emu > 0 && anchor.height_emu > 0).then(|| {
            format!(
                r#"svg:width="{}" svg:height="{}""#,
                emu_length(anchor.width_emu),
                emu_length(anchor.height_emu)
            )
        });
        let mut floated = frame.replacen(r#"text:anchor-type="as-char""#, &position, 1);
        if let Some(size) = size
            && let (Some(start), Some(end)) =
                (floated.find("svg:width=\""), floated.find(" draw:z-index"))
            && start < end
        {
            floated.replace_range(start..end, &size);
        }
        floated
    }

    fn picture_path(&mut self, media: usize, item: &Media) -> Option<String> {
        if let Some(Some(path)) = self.pictures.get(media) {
            return Some(path.clone());
        }
        let extension = crate::document::image_extension(item.content_type)?;
        let path = format!("Pictures/image{}.{extension}", self.picture_files.len() + 1);
        let tree = self.tree;
        let bytes = tree
            .media
            .get(media)
            .map_or(&[][..], |found| found.bytes.as_slice());
        self.picture_files
            .push((path.clone(), item.content_type, bytes));
        if let Some(slot) = self.pictures.get_mut(media) {
            *slot = Some(path.clone());
        }
        Some(path)
    }

    /// The automatic text style of a run's marks, or `None` when it carries none.
    fn text_style(&mut self, part: Part, marks: &[Mark]) -> Option<String> {
        let code = marks.contains(&Mark::Code);
        let mut properties = String::new();
        for mark in marks {
            match mark {
                Mark::Bold => properties.push_str(r#" fo:font-weight="bold" style:font-weight-asian="bold" style:font-weight-complex="bold""#),
                Mark::Italic => properties.push_str(r#" fo:font-style="italic" style:font-style-asian="italic" style:font-style-complex="italic""#),
                Mark::Underline => properties.push_str(r#" style:text-underline-style="solid" style:text-underline-width="auto" style:text-underline-color="font-color""#),
                Mark::Strike => properties.push_str(r#" style:text-line-through-style="solid""#),
                Mark::Superscript => properties.push_str(r#" style:text-position="super 58%""#),
                Mark::Subscript => properties.push_str(r#" style:text-position="sub 58%""#),
                Mark::TextStyle {
                    font_family,
                    font_size,
                    color,
                } => {
                    if let Some(family) = font_family.as_ref().filter(|_| !code) {
                        let _ = write!(properties, r#" fo:font-family="'{}'""#, attribute_xml(&family.replace('\'', "")));
                    }
                    if let Some(size) = font_size {
                        let _ = write!(properties, r#" fo:font-size="{}""#, attribute_xml(size));
                    }
                    if let Some(color) = color {
                        let _ = write!(properties, r#" fo:color="{}""#, attribute_xml(color));
                    }
                }
                Mark::Highlight { color } => {
                    let _ = write!(properties, r#" fo:background-color="{}""#, attribute_xml(color));
                }
                _ => {}
            }
        }
        if properties.is_empty() && !code {
            return None;
        }
        if properties.is_empty() {
            return Some("Source_20_Text".to_owned());
        }
        let parent = if code {
            r#" style:parent-style-name="Source_20_Text""#
        } else {
            ""
        };
        let key = format!("text|{parent}|{properties}");
        Some(self.registry(part).name("T", key, |name| {
            format!(
                r#"<style:style style:name="{name}" style:family="text"{parent}><style:text-properties{properties}/></style:style>"#
            )
        }))
    }
}

/// The paragraph properties a look and an alignment set, as attributes.
fn paragraph_properties(align: Option<Alignment>, look: &ParagraphLook) -> String {
    let mut properties = String::new();
    if let Some(align) = align {
        let value = match align {
            Alignment::Left => "start",
            Alignment::Center => "center",
            Alignment::Right => "end",
            Alignment::Justify => "justify",
        };
        let _ = write!(properties, r#" fo:text-align="{value}""#);
    }
    if let Some(before) = look.space_before_twips {
        let _ = write!(properties, r#" fo:margin-top="{}""#, length(before));
    }
    if let Some(after) = look.space_after_twips {
        let _ = write!(properties, r#" fo:margin-bottom="{}""#, length(after));
    }
    if let Some(left) = look.indent_left_twips {
        let _ = write!(properties, r#" fo:margin-left="{}""#, signed_length(left));
    }
    if let Some(right) = look.indent_right_twips {
        let _ = write!(properties, r#" fo:margin-right="{}""#, signed_length(right));
    }
    if let Some(first) = look.indent_first_line_twips {
        let _ = write!(properties, r#" fo:text-indent="{}""#, signed_length(first));
    }
    match look.line_spacing {
        Some(LineSpacing::Auto(multiple)) => {
            let percent = f64::from(multiple) / 2.4;
            let _ = write!(properties, r#" fo:line-height="{}%""#, trim_number(percent));
        }
        Some(LineSpacing::Exact(height)) => {
            let _ = write!(properties, r#" fo:line-height="{}""#, length(height));
        }
        Some(LineSpacing::AtLeast(height)) => {
            let _ = write!(
                properties,
                r#" style:line-height-at-least="{}""#,
                length(height)
            );
        }
        None => {}
    }
    if look.page_break_before {
        properties.push_str(r#" fo:break-before="page""#);
    }
    if let Some(shading) = &look.shading {
        let _ = write!(
            properties,
            r##" fo:background-color="#{}""##,
            attribute_xml(shading)
        );
    }
    properties
}

/// The text properties of the font a paragraph sets for its runs.
fn run_look_properties(font: &crate::document::RunLook) -> String {
    let mut properties = String::new();
    if let Some(family) = &font.family {
        let _ = write!(
            properties,
            r#" fo:font-family="'{}'""#,
            attribute_xml(&family.replace('\'', ""))
        );
    }
    if let Some(size) = font.size_half_points {
        let _ = write!(
            properties,
            r#" fo:font-size="{}pt""#,
            trim_number(f64::from(size) / 2.0)
        );
    }
    if let Some(colour) = &font.colour {
        let _ = write!(properties, r##" fo:color="#{}""##, attribute_xml(colour));
    }
    if font.bold == Some(true) {
        properties.push_str(r#" fo:font-weight="bold""#);
    }
    if font.italic == Some(true) {
        properties.push_str(r#" fo:font-style="italic""#);
    }
    properties
}

/// A list style numbering or bulleting each level as `ordered` says.
fn list_style(name: &str, ordered: &[Option<bool>; 10]) -> String {
    const BULLETS: [char; 3] = ['•', '◦', '▪'];
    let mut xml = format!(r#"<text:list-style style:name="{name}">"#);
    for (index, ordered) in ordered.iter().enumerate() {
        let level = index + 1;
        #[expect(clippy::cast_precision_loss, reason = "a list level is below ten")]
        let indent = 0.635 * (index as f64 + 1.0);
        let position = format!(
            r#"<style:list-level-properties text:list-level-position-and-space-mode="label-alignment"><style:list-level-label-alignment text:label-followed-by="listtab" text:list-tab-stop-position="{indent:.3}cm" fo:text-indent="-0.635cm" fo:margin-left="{indent:.3}cm"/></style:list-level-properties>"#
        );
        if *ordered == Some(true) {
            let _ = write!(
                xml,
                r#"<text:list-level-style-number text:level="{level}" style:num-suffix="." style:num-format="1">{position}</text:list-level-style-number>"#
            );
        } else {
            let _ = write!(
                xml,
                r#"<text:list-level-style-bullet text:level="{level}" text:bullet-char="{}">{position}</text:list-level-style-bullet>"#,
                BULLETS[index % BULLETS.len()]
            );
        }
    }
    xml.push_str("</text:list-style>");
    xml
}

fn page_layout_xml(name: &str, layout: &PageLayout, headers: &HeadersFooters) -> String {
    let has_header = !headers.header.is_empty() || headers.first_header.is_some();
    let has_footer = !headers.footer.is_empty() || headers.first_footer.is_some();
    let [top, right, bottom, left] = layout.margins_twips;
    // A header sits in the top margin: the page margin is its distance from the edge, and the
    // room between it and the body is the header's own spacing.
    let page_top = if has_header {
        layout.header_distance_twips.min(top)
    } else {
        top
    };
    let page_bottom = if has_footer {
        layout.footer_distance_twips.min(bottom)
    } else {
        bottom
    };
    let mut xml = format!(
        r#"<style:page-layout style:name="{name}"><style:page-layout-properties fo:page-width="{}" fo:page-height="{}" style:print-orientation="{}" fo:margin-top="{}" fo:margin-right="{}" fo:margin-bottom="{}" fo:margin-left="{}">"#,
        length(layout.width_twips),
        length(layout.height_twips),
        if layout.width_twips > layout.height_twips {
            "landscape"
        } else {
            "portrait"
        },
        length(page_top),
        length(right),
        length(page_bottom),
        length(left)
    );
    if layout.columns > 1 {
        let _ = write!(
            xml,
            r#"<style:columns fo:column-count="{}" fo:column-gap="{}"/>"#,
            layout.columns,
            length(layout.column_gap_twips)
        );
    }
    xml.push_str("</style:page-layout-properties>");
    if has_header {
        let _ = write!(
            xml,
            r#"<style:header-style><style:header-footer-properties fo:min-height="0pt" fo:margin-bottom="{}"/></style:header-style>"#,
            length(top - page_top)
        );
    }
    if has_footer {
        let _ = write!(
            xml,
            r#"<style:footer-style><style:header-footer-properties fo:min-height="0pt" fo:margin-top="{}"/></style:footer-style>"#,
            length(bottom - page_bottom)
        );
    }
    xml.push_str("</style:page-layout>");
    xml
}

fn styles_xml(layout: &PageLayout, page_layouts: &str, automatic: &str, masters: &str) -> String {
    let mut default = String::new();
    if let Some(family) = &layout.font_family {
        let _ = write!(
            default,
            r#" fo:font-family="'{}'""#,
            attribute_xml(&family.replace('\'', ""))
        );
    }
    let size = layout.font_size_half_points.unwrap_or(22);
    let _ = write!(
        default,
        r#" fo:font-size="{}pt""#,
        trim_number(f64::from(size) / 2.0)
    );
    let mut headings = String::new();
    for (level, points) in [
        (1, 20.0),
        (2, 16.0),
        (3, 14.0),
        (4, 13.0),
        (5, 12.0),
        (6, 11.0),
    ] {
        let _ = write!(
            headings,
            r#"<style:style style:name="Heading_20_{level}" style:display-name="Heading {level}" style:family="paragraph" style:parent-style-name="Heading" style:next-style-name="Standard" style:default-outline-level="{level}" style:class="text"><style:text-properties fo:font-size="{}pt"/></style:style>"#,
            trim_number(points)
        );
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NAMESPACES}><office:styles><style:default-style style:family="paragraph"><style:paragraph-properties style:tab-stop-distance="{}"/><style:text-properties{default}/></style:default-style>{NAMED_STYLES}{headings}</office:styles><office:automatic-styles>{page_layouts}{automatic}</office:automatic-styles><office:master-styles>{masters}</office:master-styles></office:document-styles>"#,
        length(layout.default_tab_twips.max(1))
    )
}

fn settings_xml(spacing_adds_up: bool) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-settings {NAMESPACES}><office:settings><config:config-item-set config:name="ooo:configuration-settings"><config:config-item config:name="AddParaTableSpacing" config:type="boolean">{spacing_adds_up}</config:config-item></config:config-item-set></office:settings></office:document-settings>"#
    )
}

/// The size an image is drawn at: its own, or its pixels at 96 dots an inch, the other side
/// following its proportions; nothing known, a 6 cm square.
fn image_size(item: &Media, width: Option<u32>, height: Option<u32>) -> (u32, u32) {
    let natural = crate::document::image_pixel_size(&item.bytes);
    match (width, height, natural) {
        (Some(width), Some(height), _) => (width, height),
        (Some(width), None, Some((natural_width, natural_height))) if natural_width > 0 => {
            (width, width * natural_height / natural_width)
        }
        (None, Some(height), Some((natural_width, natural_height))) if natural_height > 0 => {
            (height * natural_width / natural_height, height)
        }
        (None, None, Some(natural)) => natural,
        (Some(side), None, _) | (None, Some(side), _) => (side, side),
        _ => (227, 227),
    }
}

/// English Metric Units, 914,400 to the inch, as inches precise to the EMU.
fn emu_length(emu: i64) -> String {
    #[expect(clippy::cast_precision_loss, reason = "lengths of a few pages")]
    let inches = emu as f64 / 914_400.0;
    let text = format!("{inches:.7}");
    format!("{}in", text.trim_end_matches('0').trim_end_matches('.'))
}

/// CSS pixels, a ninety-sixth of an inch, as centimetres.
fn css_pixels(pixels: u32) -> String {
    format!("{}cm", trim_number(f64::from(pixels) * 2.54 / 96.0))
}

/// Twentieths of a point as points.
fn length(twips: u32) -> String {
    format!("{}pt", trim_number(f64::from(twips) / 20.0))
}

fn signed_length(twips: i32) -> String {
    format!("{}pt", trim_number(f64::from(twips) / 20.0))
}

/// A number with at most three decimals and no trailing zeros.
fn trim_number(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// Text as the content of an `OpenDocument` paragraph: runs of spaces as `text:s`, tabs as
/// `text:tab`, and the characters XML 1.0 cannot hold left out.
fn text_xml(text: &str) -> String {
    let mut xml = String::with_capacity(text.len());
    let mut spaces = 0usize;
    let mut at_start = true;
    let flush = |xml: &mut String, spaces: &mut usize, at_start: bool| {
        match *spaces {
            0 => {}
            1 if !at_start => xml.push(' '),
            count if at_start => {
                let _ = write!(xml, r#"<text:s text:c="{count}"/>"#);
            }
            count => {
                xml.push(' ');
                if count > 1 {
                    let _ = write!(xml, r#"<text:s text:c="{}"/>"#, count - 1);
                }
            }
        }
        *spaces = 0;
    };
    for character in text.chars() {
        match character {
            ' ' => {
                spaces += 1;
                continue;
            }
            _ => flush(&mut xml, &mut spaces, at_start),
        }
        at_start = false;
        match character {
            '\t' => xml.push_str("<text:tab/>"),
            '\n' => xml.push_str("<text:line-break/>"),
            '&' => xml.push_str("&amp;"),
            '<' => xml.push_str("&lt;"),
            '>' => xml.push_str("&gt;"),
            '\u{FFFE}' | '\u{FFFF}' => {}
            character if (character as u32) < 0x20 => {}
            character => xml.push(character),
        }
    }
    // Spaces at the end are kept too: a paragraph's trailing spaces are its own.
    if spaces > 0 {
        let _ = write!(xml, r#"<text:s text:c="{spaces}"/>"#);
    }
    xml
}

fn attribute_xml(text: &str) -> String {
    text.chars()
        .filter(|character| (*character as u32) >= 0x20 || *character == '\t')
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests;
