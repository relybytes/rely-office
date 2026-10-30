// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A document tree written out as a docx: the inverse of [`super::read_document_tree`], so an
//! editor's document can leave as a file Word opens and this crate reads back to the same tree.
//!
//! The package carries Word's built-in styles by their English names (`Heading 1`, `Quote`,
//! `List Paragraph`, `Hyperlink`, `Table Grid`, `footnote text`), a bullet and a numbered list
//! definition with one instance per list of the tree, the notes, headers and footers, and the
//! page of the tree's layout. The looks a tree read from a docx carries are written as direct
//! formatting, so a document read and written again keeps its spacing, fonts and table colours;
//! a tree from an editor has no looks and takes the styles' own.

use super::format::NamedStyle;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;

use super::edges::{Border, BorderStyle, Borders, CellLook, TabKind, TabLeader, VerticalAlignment};
use super::look::{LineSpacing, PageField, ParagraphLook, RunLook, TableLook};
use super::objects::{self, EmbeddedObject};
use super::{
    Alignment, Anchor, Block, Comment, DocumentTree, Inline, Mark, Media, Note, NoteKind,
    PageLayout, Section, TableRow, TextFrame,
};
use crate::OfficeError;

const EMUS_PER_PIXEL: u64 = 9_525;
/// The deepest list level Word numbers.
const MAX_LIST_LEVEL: u8 = 8;
const MAX_HEADING_LEVEL: u8 = 6;
/// Half points: Word's smallest and largest font sizes.
const MIN_HALF_POINTS: u32 = 2;
const MAX_HALF_POINTS: u32 = 3_276;
const RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
/// The relationships of the main part the writer always declares: styles, numbering, settings,
/// footnotes and endnotes.
const MAIN_FIXED_RELATIONSHIPS: usize = 6;
const W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
const COMMENTS_EXTENDED_RELATIONSHIP: &str =
    "http://schemas.microsoft.com/office/2011/relationships/commentsExtended";
const NAMESPACES: &str = concat!(
    r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" "#,
    r#"xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" "#,
    r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math" "#,
    r#"xmlns:v="urn:schemas-microsoft-com:vml" "#,
    r#"xmlns:o="urn:schemas-microsoft-com:office:office""#,
);

/// Write a document tree as a docx. An image whose type is not one Word shows (PNG, JPEG, GIF,
/// BMP, TIFF, WebP, SVG) is left out; its alternative text, if any, stays as text.
///
/// # Errors
///
/// [`OfficeError::DamagedContainer`] when the zip cannot be written, which only a failing
/// allocator causes.
#[expect(
    clippy::too_many_lines,
    reason = "the parts of a docx are written in the order their relationships number them"
)]
pub fn write_docx(tree: &DocumentTree) -> Result<Vec<u8>, OfficeError> {
    let mut writer = DocxWriter {
        media: &tree.media,
        objects: &tree.objects,
        object_parts: BTreeMap::new(),
        notes: &tree.notes,
        comments: &tree.comments,
        in_body: false,
        open_comments: Vec::new(),
        comment_ids: HashMap::new(),
        comments_xml: String::new(),
        comment_relationships: Vec::new(),
        threads: Vec::new(),
        next_revision: 1,
        relationships: Vec::new(),
        relationship_base: MAIN_FIXED_RELATIONSHIPS,
        image_parts: BTreeMap::new(),
        lists: HashMap::new(),
        next_drawing_id: 1,
        text_width_twips: text_width(&tree.layout),
        written_notes: HashMap::new(),
        footnotes: String::new(),
        endnotes: String::new(),
        footnote_relationships: Vec::new(),
        endnote_relationships: Vec::new(),
        note_counts: [0, 0],
    };
    let single = [Section {
        end_block: tree.blocks.len(),
        layout: tree.layout.clone(),
        headers_footers: tree.headers_footers.clone(),
        continuous: false,
    }];
    let sections: &[Section] = if tree.sections.is_empty() {
        &single
    } else {
        &tree.sections
    };
    let mut bodies = Vec::new();
    let mut start = 0;
    for (index, section) in sections.iter().enumerate() {
        let end = if index + 1 == sections.len() {
            tree.blocks.len()
        } else {
            section.end_block.clamp(start, tree.blocks.len())
        };
        writer.text_width_twips = text_width(&section.layout);
        let mut xml = String::new();
        writer.in_body = true;
        writer.blocks(&tree.blocks[start..end], None, &mut xml);
        if index + 1 == sections.len() && !writer.open_comments.is_empty() {
            // A comment still open at the end of the text closes in a paragraph of its own.
            xml.push_str("<w:p>");
            writer.close_comments(&[], &mut xml);
            xml.push_str("</w:p>");
        }
        writer.in_body = false;
        bodies.push(xml);
        start = end;
    }
    let main_relationships = std::mem::take(&mut writer.relationships);
    let mut header_parts = Vec::new();
    let mut extra_relationships = Vec::new();
    let mut body = String::new();
    for (index, (section, xml)) in sections.iter().zip(&bodies).enumerate() {
        // Word carries a header on into a section that names none, so an empty one is written
        // where the section before had one.
        let keep_empty = index.checked_sub(1).map_or([false; 2], |previous| {
            let before = &sections[previous].headers_footers;
            [!before.header.is_empty(), !before.footer.is_empty()]
        });
        let parts = writer.header_parts(&section.headers_footers, keep_empty);
        let (references, relationships) = section_references(
            &parts,
            MAIN_FIXED_RELATIONSHIPS + main_relationships.len() + extra_relationships.len(),
            header_parts.len(),
        );
        header_parts.extend(parts);
        extra_relationships.extend(relationships);
        body.push_str(xml);
        let properties = section_properties(section, &references);
        if index + 1 == sections.len() {
            body.push_str(&properties);
        } else {
            // A section before the last ends with a paragraph that carries its properties.
            let _ = write!(body, "<w:p><w:pPr>{properties}</w:pPr></w:p>");
        }
    }

    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {NAMESPACES}><w:body>{body}</w:body></w:document>"#
    );
    let mut lists: Vec<(u32, bool, u32)> = writer
        .lists
        .iter()
        .map(|(&(list_id, ordered), &num_id)| (num_id, ordered, list_id))
        .collect();
    lists.sort_unstable();

    let header_names: Vec<String> = extra_relationships
        .iter()
        .map(|relationship| relationship.target.clone())
        .collect();
    let mut all_main = main_relationships;
    all_main.extend(extra_relationships);
    let comment_parts = writer.comment_parts(&mut all_main);
    let mut members: Vec<(String, Vec<u8>)> = Vec::new();
    let mut add = |name: &str, bytes: &[u8]| members.push((name.to_owned(), bytes.to_vec()));
    let mut content_parts = header_names;
    content_parts.extend(comment_parts.iter().map(|(name, _)| name.clone()));
    let object_types: Vec<(String, String)> = writer.object_parts.values().cloned().collect();
    add(
        "[Content_Types].xml",
        content_types(&content_parts, &object_types).as_bytes(),
    );
    add("_rels/.rels", package_relationships().as_bytes());
    add("word/document.xml", document.as_bytes());
    add("word/styles.xml", styles_part(&tree.layout).as_bytes());
    add("word/numbering.xml", numbering(&lists).as_bytes());
    let (font_table, font_relationships, font_parts) = super::docx_font_table(tree);
    add(
        "word/settings.xml",
        settings(tree, sections, !font_parts.is_empty()).as_bytes(),
    );
    add("word/fontTable.xml", font_table.as_bytes());
    if !font_relationships.is_empty() {
        let mut xml = String::from(concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        ));
        for (id, target) in &font_relationships {
            let _ = write!(
                xml,
                r#"<Relationship Id="{id}" Type="{RELATIONSHIP_TYPE}/font" Target="{target}"/>"#
            );
        }
        xml.push_str("</Relationships>");
        add("word/_rels/fontTable.xml.rels", xml.as_bytes());
    }
    for (target, bytes) in &font_parts {
        add(&format!("word/{target}"), bytes);
    }
    add(
        "word/footnotes.xml",
        notes_part("footnotes", "footnote", &writer.footnotes).as_bytes(),
    );
    add(
        "word/endnotes.xml",
        notes_part("endnotes", "endnote", &writer.endnotes).as_bytes(),
    );
    add(
        "word/_rels/document.xml.rels",
        relationships_part(&all_main, true).as_bytes(),
    );
    for (name, relationships) in [
        ("footnotes.xml", &writer.footnote_relationships),
        ("endnotes.xml", &writer.endnote_relationships),
    ] {
        if !relationships.is_empty() {
            add(
                &format!("word/_rels/{name}.rels"),
                relationships_part(relationships, false).as_bytes(),
            );
        }
    }
    for (index, (_, header, xml, relationships)) in header_parts.iter().enumerate() {
        let (root, name) = if *header {
            ("hdr", "header")
        } else {
            ("ftr", "footer")
        };
        let part = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:{root} {NAMESPACES}>{xml}</w:{root}>"#
        );
        add(&format!("word/{name}{}.xml", index + 1), part.as_bytes());
        if !relationships.is_empty() {
            add(
                &format!("word/_rels/{name}{}.xml.rels", index + 1),
                relationships_part(relationships, false).as_bytes(),
            );
        }
    }
    for (index, part) in &writer.image_parts {
        add(&format!("word/{part}"), &tree.media[*index].bytes);
    }
    for (index, (part, _)) in &writer.object_parts {
        add(&format!("word/{part}"), &tree.objects[*index].bytes);
    }
    for (name, bytes) in &comment_parts {
        add(&format!("word/{name}"), bytes);
    }
    let package = zip_members(&members)?;
    match &tree.macro_project {
        Some(project) => crate::insert_macro_project(&package, project),
        None => Ok(package),
    }
}

/// The references of a section to its header and footer parts, and the relationships of the
/// main part that name them, numbered from after `previous` relationships; the parts are
/// numbered from after the `written` parts of the sections before.
fn section_references(
    header_parts: &[HeaderPart],
    previous: usize,
    written: usize,
) -> (String, Vec<Relationship>) {
    let mut references = String::new();
    let mut relationships = Vec::new();
    for (index, (kind, header, _, _)) in header_parts.iter().enumerate() {
        let id = format!("rId{}", previous + index + 1);
        let (element, part_kind) = if *header {
            ("headerReference", "header")
        } else {
            ("footerReference", "footer")
        };
        let _ = write!(references, r#"<w:{element} w:type="{kind}" r:id="{id}"/>"#);
        relationships.push(Relationship {
            id,
            kind: part_kind,
            target: format!("{part_kind}{}.xml", written + index + 1),
            external: false,
        });
    }
    (references, relationships)
}

/// A zip of these members, deflated.
pub(crate) fn zip_members(members: &[(String, Vec<u8>)]) -> Result<Vec<u8>, OfficeError> {
    let damaged = |error: &dyn std::fmt::Display| OfficeError::DamagedContainer(error.to_string());
    let mut package = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in members {
        package
            .start_file(name.as_str(), SimpleFileOptions::default())
            .map_err(|error| damaged(&error))?;
        package.write_all(bytes).map_err(|error| damaged(&error))?;
    }
    package
        .finish()
        .map(Cursor::into_inner)
        .map_err(|error| damaged(&error))
}

/// A relationship of a part: id, type name, target, whether external.
struct Relationship {
    id: String,
    kind: &'static str,
    target: String,
    external: bool,
}

struct DocxWriter<'a> {
    media: &'a [Media],
    objects: &'a [EmbeddedObject],
    /// Object index to its part path and content type.
    object_parts: BTreeMap<usize, (String, String)>,
    notes: &'a [Note],
    comments: &'a [Comment],
    /// The body is being written, where comments are anchored; notes and headers have none.
    in_body: bool,
    /// The comments whose range is open, by their index in the tree.
    open_comments: Vec<usize>,
    /// Comment index of the tree to the id written for it.
    comment_ids: HashMap<usize, u32>,
    comments_xml: String,
    comment_relationships: Vec<Relationship>,
    /// The paragraph id of each written comment and of the comment it answers, and whether it
    /// is done, for `commentsExtended`.
    threads: Vec<(u32, Option<u32>, bool)>,
    next_revision: u32,
    /// The relationships of the part being written.
    relationships: Vec<Relationship>,
    /// How many relationship ids the part being written reserves before its own.
    relationship_base: usize,
    /// Media index to its part path.
    image_parts: BTreeMap<usize, String>,
    /// `(list id, ordered)` of the tree to the `w:num` id written for it.
    lists: HashMap<(u32, bool), u32>,
    next_drawing_id: u32,
    text_width_twips: u32,
    /// Note index of the tree to the kind and id written for it.
    written_notes: HashMap<usize, (NoteKind, u32)>,
    footnotes: String,
    endnotes: String,
    footnote_relationships: Vec<Relationship>,
    endnote_relationships: Vec<Relationship>,
    /// Notes written so far, footnotes and endnotes.
    note_counts: [u32; 2],
}

/// A header or footer part: its `w:type`, whether it is a header, its XML and relationships.
type HeaderPart = (&'static str, bool, String, Vec<Relationship>);

impl DocxWriter<'_> {
    /// The header and footer parts of the tree, each with relationships of its own; a default one
    /// with no content is left out, a first-page or even-page one is kept to stay empty.
    fn header_parts(
        &mut self,
        parts: &super::HeadersFooters,
        keep_empty: [bool; 2],
    ) -> Vec<HeaderPart> {
        let mut written = Vec::new();
        for (kind, header, blocks) in [
            ("default", true, Some(&parts.header)),
            ("default", false, Some(&parts.footer)),
            ("first", true, parts.first_header.as_ref()),
            ("first", false, parts.first_footer.as_ref()),
            ("even", true, parts.even_header.as_ref()),
            ("even", false, parts.even_footer.as_ref()),
        ] {
            let Some(blocks) = blocks else { continue };
            let kept = if header { keep_empty[0] } else { keep_empty[1] };
            if blocks.is_empty() && kind == "default" && !kept {
                continue;
            }
            self.relationship_base = 0;
            let mut xml = String::new();
            self.blocks(blocks, None, &mut xml);
            if !xml.ends_with("</w:p>") {
                xml.push_str("<w:p/>");
            }
            written.push((kind, header, xml, std::mem::take(&mut self.relationships)));
        }
        written
    }

    /// Blocks as body XML; `paragraph_style` names the style of plain paragraphs, a quote's or a
    /// note's.
    fn blocks(&mut self, blocks: &[Block], paragraph_style: Option<&str>, xml: &mut String) {
        let mut index = 0;
        while let Some(block) = blocks.get(index) {
            if block_frame(block).is_some() {
                // A text box with no paragraph before it to hold it gets one of its own.
                xml.push_str("<w:p>");
                index = self.text_boxes(blocks, index, xml);
                xml.push_str("</w:p>");
                continue;
            }
            index += 1;
            match block {
                Block::Heading {
                    level,
                    content,
                    align,
                    look,
                } => {
                    // The title of Word reads as a top heading and is written back as its style.
                    let style = look.format.style.map_or_else(
                        || format!("Heading{}", (*level).clamp(1, MAX_HEADING_LEVEL)),
                        |style| style.style_id().to_owned(),
                    );
                    self.paragraph(xml, Some(&style), *align, None, content, look);
                }
                Block::Paragraph {
                    content,
                    align,
                    look,
                } => {
                    // A named style is the paragraph's own: an intense quote stays one inside a quote.
                    let style = look.format.style.map(NamedStyle::style_id).or(paragraph_style);
                    self.paragraph(xml, style, *align, None, content, look);
                }
                Block::ListItem {
                    ordered,
                    level,
                    list_id,
                    content,
                    look,
                } => {
                    let next = u32::try_from(self.lists.len()).unwrap_or(u32::MAX - 1) + 1;
                    let num_id = *self.lists.entry((*list_id, *ordered)).or_insert(next);
                    let level = (*level).min(MAX_LIST_LEVEL);
                    let look = ParagraphLook {
                        list_label: None,
                        ..look.clone()
                    };
                    self.paragraph(
                        xml,
                        Some("ListParagraph"),
                        None,
                        Some((num_id, level)),
                        content,
                        &look,
                    );
                }
                Block::Table { rows, look } => self.table(rows, look, xml),
                Block::Blockquote { blocks } => self.blocks(blocks, Some("Quote"), xml),
                Block::CodeBlock { text } => {
                    for line in text.split('\n') {
                        xml.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Code"/></w:pPr>"#);
                        push_text_runs(xml, line, "");
                        xml.push_str("</w:p>");
                    }
                }
                Block::HorizontalRule => xml.push_str(concat!(
                    r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6" w:space="1" w:color="auto"/>"#,
                    "</w:pBdr></w:pPr></w:p>"
                )),
            }
            // The text boxes the tree sets after a paragraph are anchored in it, as they were read.
            if blocks.get(index).and_then(block_frame).is_some() && xml.ends_with("</w:p>") {
                xml.truncate(xml.len() - "</w:p>".len());
                index = self.text_boxes(blocks, index, xml);
                xml.push_str("</w:p>");
            }
        }
    }

    /// The text boxes from `start` on, one run each, the consecutive blocks of a frame together;
    /// the index of the first block past them.
    fn text_boxes(&mut self, blocks: &[Block], start: usize, xml: &mut String) -> usize {
        let mut index = start;
        while let Some(frame) = blocks.get(index).and_then(block_frame).cloned() {
            let end = blocks[index..]
                .iter()
                .position(|block| block_frame(block).is_none_or(|other| other.id != frame.id))
                .map_or(blocks.len(), |offset| index + offset);
            let content: Vec<Block> = blocks[index..end].iter().map(without_frame).collect();
            let mut inner = String::new();
            self.blocks(&content, None, &mut inner);
            let id = self.next_drawing_id;
            self.next_drawing_id += 1;
            xml.push_str(&floating::text_box_run(&frame, &inner, id));
            index = end;
        }
        index
    }

    fn paragraph(
        &mut self,
        xml: &mut String,
        style: Option<&str>,
        align: Option<Alignment>,
        list: Option<(u32, u8)>,
        content: &[Inline],
        look: &ParagraphLook,
    ) {
        xml.push_str("<w:p>");
        let mut properties = String::new();
        if let Some(style) = style {
            let _ = write!(properties, r#"<w:pStyle w:val="{style}"/>"#);
        }
        if look.page_break_before {
            properties.push_str("<w:pageBreakBefore/>");
        }
        if let Some((num_id, level)) = list {
            let _ = write!(
                properties,
                r#"<w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{num_id}"/></w:numPr>"#
            );
        }
        paragraph_look_properties(look, &mut properties);
        if let Some(alignment) = align.map(|align| match align {
            Alignment::Left => "left",
            Alignment::Center => "center",
            Alignment::Right => "right",
            Alignment::Justify => "both",
        }) {
            let _ = write!(properties, r#"<w:jc w:val="{alignment}"/>"#);
        }
        if !properties.is_empty() {
            let _ = write!(xml, "<w:pPr>{properties}</w:pPr>");
        }
        // A numbered heading keeps its label as text: the writer's lists do not number headings.
        if let Some(label) = &look.list_label {
            push_text_runs(xml, &format!("{}\t", label.text), "");
        }
        self.inlines(content, &look.font, xml);
        xml.push_str("</w:p>");
    }

    fn inlines(&mut self, content: &[Inline], paragraph_font: &RunLook, xml: &mut String) {
        for inline in content {
            if self.in_body
                && let Inline::Text { marks, .. } = inline
            {
                let marked: Vec<usize> = marks
                    .iter()
                    .filter_map(|mark| match mark {
                        Mark::Comment { comment } => Some(*comment),
                        _ => None,
                    })
                    .collect();
                self.close_comments(&marked, xml);
                for comment in marked {
                    if !self.open_comments.contains(&comment)
                        && let Some(id) = self.comment_id(comment)
                    {
                        let _ = write!(xml, r#"<w:commentRangeStart w:id="{id}"/>"#);
                        self.open_comments.push(comment);
                    }
                }
            }
            match inline {
                Inline::Text { text, marks, look } => {
                    self.text_run(text, marks, look, paragraph_font, xml);
                }
                Inline::HardBreak => xml.push_str("<w:r><w:br/></w:r>"),
                Inline::Image {
                    media,
                    alt,
                    width,
                    height,
                    anchor,
                } => self.image(
                    *media,
                    alt.as_deref(),
                    (*width, *height),
                    anchor.as_deref(),
                    xml,
                ),
                Inline::NoteReference { note } => self.note_reference(*note, xml),
                Inline::Math { latex, display } => {
                    let math = crate::math::parse_latex(latex);
                    xml.push_str(&crate::math::math_to_omml(&math, *display));
                }
                Inline::CommentAnchor { comment } => {
                    if self.in_body
                        && let Some(id) = self.comment_id(*comment)
                    {
                        let _ = write!(
                            xml,
                            r#"<w:commentRangeStart w:id="{id}"/><w:commentRangeEnd w:id="{id}"/>"#
                        );
                        comment_reference(id, xml);
                    }
                }
            }
        }
    }

    /// Close the open comments that are not among `kept`, each with its reference.
    fn close_comments(&mut self, kept: &[usize], xml: &mut String) {
        let closing: Vec<usize> = self
            .open_comments
            .iter()
            .copied()
            .filter(|comment| !kept.contains(comment))
            .collect();
        for comment in closing {
            self.open_comments.retain(|open| *open != comment);
            if let Some(id) = self.comment_ids.get(&comment).copied() {
                let _ = write!(xml, r#"<w:commentRangeEnd w:id="{id}"/>"#);
                comment_reference(id, xml);
            }
        }
    }

    /// The id of a comment, writing it into the comments part the first time; a reply writes
    /// the comment it answers first.
    fn comment_id(&mut self, comment: usize) -> Option<u32> {
        if let Some(id) = self.comment_ids.get(&comment) {
            return Some(*id);
        }
        let comments = self.comments;
        let content = comments.get(comment)?;
        let id = u32::try_from(self.comment_ids.len()).ok()?;
        self.comment_ids.insert(comment, id);
        let parent = content
            .parent
            .filter(|parent| *parent != comment)
            .and_then(|parent| self.comment_id(parent));
        let outer_relationships = std::mem::take(&mut self.relationships);
        let outer_base = std::mem::replace(
            &mut self.relationship_base,
            self.comment_relationships.len(),
        );
        let outer_body = std::mem::replace(&mut self.in_body, false);
        let mut blocks = String::new();
        self.blocks(&content.blocks, Some("CommentText"), &mut blocks);
        self.in_body = outer_body;
        let relationships = std::mem::replace(&mut self.relationships, outer_relationships);
        self.relationship_base = outer_base;
        self.comment_relationships.extend(relationships);
        let mark =
            r#"<w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:annotationRef/></w:r>"#;
        let blocks = with_leading_run(&blocks, mark, "CommentText");
        let paragraph = paragraph_id(id);
        let blocks = match blocks
            .rfind("<w:p>")
            .into_iter()
            .chain(blocks.rfind("<w:p "))
            .max()
        {
            Some(position) => format!(
                r#"{}<w:p w14:paraId="{paragraph:08X}"{}"#,
                &blocks[..position],
                &blocks[position + 4..]
            ),
            None => blocks,
        };
        let mut attributes = format!(r#" w:id="{id}" w:author="{}""#, escape(&content.author));
        if let Some(date) = content.date.as_deref().filter(|date| is_date(date)) {
            let _ = write!(attributes, r#" w:date="{date}""#);
        }
        if let Some(initials) = &content.initials {
            let _ = write!(attributes, r#" w:initials="{}""#, escape(initials));
        }
        let _ = write!(
            self.comments_xml,
            "<w:comment{attributes}>{blocks}</w:comment>"
        );
        self.threads
            .push((paragraph, parent.map(paragraph_id), content.resolved));
        Some(id)
    }

    /// The comments part, its relationships and the threads of the comments, when the text
    /// anchored any; their relationships are added to the main part's.
    fn comment_parts(&mut self, main: &mut Vec<Relationship>) -> Vec<(String, Vec<u8>)> {
        if self.comment_ids.is_empty() {
            return Vec::new();
        }
        let mut parts = vec![(
            "comments.xml".to_owned(),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments {NAMESPACES} xmlns:w14="{W14}">{}</w:comments>"#,
                self.comments_xml
            )
            .into_bytes(),
        )];
        if !self.comment_relationships.is_empty() {
            parts.push((
                "_rels/comments.xml.rels".to_owned(),
                relationships_part(&self.comment_relationships, false).into_bytes(),
            ));
        }
        let id = format!("rId{}", MAIN_FIXED_RELATIONSHIPS + main.len() + 1);
        main.push(Relationship {
            id,
            kind: "comments",
            target: "comments.xml".to_owned(),
            external: false,
        });
        if self
            .threads
            .iter()
            .any(|(_, parent, done)| parent.is_some() || *done)
        {
            let mut entries = String::new();
            for (paragraph, parent, done) in &self.threads {
                let _ = write!(entries, r#"<w15:commentEx w15:paraId="{paragraph:08X}""#);
                if let Some(parent) = parent {
                    let _ = write!(entries, r#" w15:paraIdParent="{parent:08X}""#);
                }
                let _ = write!(entries, r#" w15:done="{}"/>"#, u8::from(*done));
            }
            parts.push((
                "commentsExtended.xml".to_owned(),
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w15:commentsEx xmlns:w15="{W15}">{entries}</w15:commentsEx>"#
                )
                .into_bytes(),
            ));
            let id = format!("rId{}", MAIN_FIXED_RELATIONSHIPS + main.len() + 1);
            main.push(Relationship {
                id,
                kind: COMMENTS_EXTENDED_RELATIONSHIP,
                target: "commentsExtended.xml".to_owned(),
                external: false,
            });
        }
        parts
    }

    fn text_run(
        &mut self,
        text: &str,
        marks: &[Mark],
        look: &RunLook,
        paragraph_font: &RunLook,
        xml: &mut String,
    ) {
        let (mut properties, link) = run_properties(marks, look, paragraph_font);
        if let Some(Mark::FormatChange {
            author,
            date,
            previous,
        }) = marks
            .iter()
            .find(|mark| matches!(mark, Mark::FormatChange { .. }))
        {
            let id = self.next_revision;
            self.next_revision += 1;
            let (before, _) = run_properties(previous, &RunLook::default(), paragraph_font);
            let mut attributes = format!(r#" w:id="{id}" w:author="{}""#, escape(author));
            if let Some(date) = date.as_deref().filter(|date| is_date(date)) {
                let _ = write!(attributes, r#" w:date="{date}""#);
            }
            let before = if before.is_empty() {
                "<w:rPr/>".to_owned()
            } else {
                before
            };
            let change = format!("<w:rPrChange{attributes}>{before}</w:rPrChange>");
            properties = match properties.strip_suffix("</w:rPr>") {
                Some(open) => format!("{open}{change}</w:rPr>"),
                None => format!("<w:rPr>{change}</w:rPr>"),
            };
        }
        let run_properties = properties;
        let revision = marks.iter().find_map(|mark| match mark {
            Mark::Insertion { author, date } => Some(("ins", author, date)),
            Mark::Deletion { author, date } => Some(("del", author, date)),
            _ => None,
        });
        if let Some((element, author, date)) = revision {
            let id = self.next_revision;
            self.next_revision += 1;
            let mut attributes = format!(r#" w:id="{id}" w:author="{}""#, escape(author));
            if let Some(date) = date.as_deref().filter(|date| is_date(date)) {
                let _ = write!(attributes, r#" w:date="{date}""#);
            }
            let deleted = element == "del";
            let mut runs = String::new();
            push_runs(&mut runs, text, &run_properties, deleted);
            let changed = format!("<w:{element}{attributes}>{runs}</w:{element}>");
            match link {
                Some(href) => {
                    let id = self.relationship("hyperlink", href.to_owned(), true);
                    let _ = write!(
                        xml,
                        r#"<w:hyperlink r:id="{id}" w:history="1">{changed}</w:hyperlink>"#
                    );
                }
                None => xml.push_str(&changed),
            }
            return;
        }
        if let Some(field) = look.field {
            let instruction = match field {
                PageField::Page => "PAGE",
                PageField::Pages => "NUMPAGES",
            };
            let _ = write!(xml, r#"<w:fldSimple w:instr=" {instruction} ">"#);
            push_text_runs(xml, text, &run_properties);
            xml.push_str("</w:fldSimple>");
            return;
        }
        match link {
            Some(href) => {
                let id = self.relationship("hyperlink", href.to_owned(), true);
                let _ = write!(xml, r#"<w:hyperlink r:id="{id}" w:history="1">"#);
                push_text_runs(xml, text, &run_properties);
                xml.push_str("</w:hyperlink>");
            }
            None => push_text_runs(xml, text, &run_properties),
        }
    }

    /// The reference to a note, writing the note into its part the first time.
    fn note_reference(&mut self, note: usize, xml: &mut String) {
        let Some(written) = self.written_note(note) else {
            return;
        };
        let (element, style) = match written.0 {
            NoteKind::Footnote => ("footnoteReference", "FootnoteReference"),
            NoteKind::Endnote => ("endnoteReference", "EndnoteReference"),
        };
        let _ = write!(
            xml,
            r#"<w:r><w:rPr><w:rStyle w:val="{style}"/></w:rPr><w:{element} w:id="{}"/></w:r>"#,
            written.1
        );
    }

    fn written_note(&mut self, note: usize) -> Option<(NoteKind, u32)> {
        if let Some(written) = self.written_notes.get(&note) {
            return Some(*written);
        }
        let content = self.notes.get(note)?;
        let slot = usize::from(content.kind == NoteKind::Endnote);
        self.note_counts[slot] += 1;
        let id = self.note_counts[slot];
        self.written_notes.insert(note, (content.kind, id));

        let (element, text_style, reference_style, reference) = match content.kind {
            NoteKind::Footnote => (
                "footnote",
                "FootnoteText",
                "FootnoteReference",
                "footnoteRef",
            ),
            NoteKind::Endnote => ("endnote", "EndnoteText", "EndnoteReference", "endnoteRef"),
        };
        // The notes of a kind share one relationships part, so this note's ids follow on.
        let part_length = match content.kind {
            NoteKind::Footnote => self.footnote_relationships.len(),
            NoteKind::Endnote => self.endnote_relationships.len(),
        };
        let outer_relationships = std::mem::take(&mut self.relationships);
        let outer_base = std::mem::replace(&mut self.relationship_base, part_length);
        let mut blocks = String::new();
        self.blocks(&content.blocks, Some(text_style), &mut blocks);
        let mark = format!(
            r#"<w:r><w:rPr><w:rStyle w:val="{reference_style}"/></w:rPr><w:{reference}/></w:r>"#
        );
        let blocks = with_leading_run(&blocks, &mark, text_style);
        let relationships = std::mem::replace(&mut self.relationships, outer_relationships);
        self.relationship_base = outer_base;
        let (part, part_relationships) = match content.kind {
            NoteKind::Footnote => (&mut self.footnotes, &mut self.footnote_relationships),
            NoteKind::Endnote => (&mut self.endnotes, &mut self.endnote_relationships),
        };
        part_relationships.extend(relationships);
        let _ = write!(part, r#"<w:{element} w:id="{id}">{blocks}</w:{element}>"#);
        Some((content.kind, id))
    }

    fn image(
        &mut self,
        media: usize,
        alt: Option<&str>,
        (width, height): (Option<u32>, Option<u32>),
        anchor: Option<&Anchor>,
        xml: &mut String,
    ) {
        let Some(relationship_id) = self.image_relationship(media) else {
            if let Some(alt) = alt.filter(|alt| !alt.is_empty()) {
                push_text_runs(xml, alt, "");
            }
            return;
        };
        if let Some(object) = self.objects.iter().position(|object| object.media == media) {
            self.object(object, &relationship_id, (width, height), xml);
            return;
        }
        // A width larger than the text is cut to it, the proportions kept.
        let max_pixels = u64::from(self.text_width_twips) * 96 / 1_440;
        let width = u64::from(width.unwrap_or(300).max(1));
        let height = u64::from(height.unwrap_or(200).max(1));
        let (width, height) = if width > max_pixels && max_pixels > 0 {
            (max_pixels, (height * max_pixels / width).max(1))
        } else {
            (width, height)
        };
        let (cx, cy) = match anchor {
            Some(anchor) if anchor.width_emu > 0 && anchor.height_emu > 0 => {
                (anchor.width_emu, anchor.height_emu)
            }
            _ => (
                i64::try_from(width * EMUS_PER_PIXEL).unwrap_or(i64::MAX),
                i64::try_from(height * EMUS_PER_PIXEL).unwrap_or(i64::MAX),
            ),
        };
        let drawing_id = self.next_drawing_id;
        self.next_drawing_id += 1;
        let description = escape(alt.unwrap_or_default());
        let doc_properties = format!(
            r#"<wp:docPr id="{drawing_id}" name="Picture {drawing_id}" descr="{description}"/>"#
        );
        let (opening, closing) = match anchor {
            Some(anchor) => (
                floating::anchor_start(anchor, cx, cy, &doc_properties, drawing_id),
                "</wp:anchor>",
            ),
            None => (
                format!(
                    r#"<wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/>{doc_properties}"#
                ),
                "</wp:inline>",
            ),
        };
        let _ = write!(
            xml,
            concat!(
                r#"<w:r><w:drawing>{opening}"#,
                r#"<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
                r#"<pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="Picture {id}" descr="{descr}"/><pic:cNvPicPr/></pic:nvPicPr>"#,
                r#"<pic:blipFill><a:blip r:embed="{rel}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>"#,
                r#"<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
                r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>"#,
                r#"</a:graphicData></a:graphic>{closing}</w:drawing></w:r>"#
            ),
            opening = opening,
            closing = closing,
            cx = cx,
            cy = cy,
            id = drawing_id,
            descr = description,
            rel = relationship_id,
        );
    }

    /// The relationship of the part being written to a picture's part, the part named the first
    /// time; `None` for a picture of a type Word does not show.
    fn image_relationship(&mut self, media: usize) -> Option<String> {
        let extension = image_extension(self.media.get(media)?.content_type)?;
        let next_part = self.image_parts.len() + 1;
        let part = self
            .image_parts
            .entry(media)
            .or_insert_with(|| format!("media/image{next_part}.{extension}"))
            .clone();
        Some(self.relationship("image", part, false))
    }

    /// An embedded object as Word writes one: a VML shape of its picture, and the `OLEObject`
    /// that names its part.
    fn object(
        &mut self,
        object: usize,
        picture: &str,
        (width, height): (Option<u32>, Option<u32>),
        xml: &mut String,
    ) {
        let content = &self.objects[object];
        let next_part = self.object_parts.len() + 1;
        let (part, kind, content_type) = objects::object_part(content, next_part);
        let part = self
            .object_parts
            .entry(object)
            .or_insert((part, content_type))
            .0
            .clone();
        let program = content
            .program
            .clone()
            .or_else(|| objects::program_of_package(&content.name).map(str::to_owned))
            .map(|program| format!(r#" ProgID="{}""#, escape(&program)))
            .unwrap_or_default();
        let relationship_id = self.relationship(kind, part, false);
        let width = width.unwrap_or(300).max(1);
        let height = height.unwrap_or(200).max(1);
        let shape = self.next_drawing_id;
        self.next_drawing_id += 1;
        let _ = write!(
            xml,
            concat!(
                r#"<w:r><w:object w:dxaOrig="{dxa}" w:dyaOrig="{dya}">"#,
                r#"<v:shape id="_x0000_i{shape}" style="width:{width}pt;height:{height}pt" stroked="f" o:ole="">"#,
                r#"<v:imagedata r:id="{picture}" o:title=""/></v:shape>"#,
                r#"<o:OLEObject Type="Embed"{program} ShapeID="_x0000_i{shape}" DrawAspect="Content" ObjectID="_{object_id}" r:id="{relationship}"/>"#,
                "</w:object></w:r>"
            ),
            dxa = width * 15,
            dya = height * 15,
            shape = 1024 + shape,
            width = points_of_pixels(width),
            height = points_of_pixels(height),
            picture = picture,
            program = program,
            object_id = 1_000_000_000 + shape,
            relationship = relationship_id,
        );
    }

    fn table(&mut self, rows: &[TableRow], look: &TableLook, xml: &mut String) {
        let placed = place_cells(rows);
        let columns = placed.columns.max(1);
        let widths: Vec<u32> = if look.column_widths_twips.len() == columns
            && !look.column_widths_twips.contains(&0)
        {
            look.column_widths_twips.clone()
        } else {
            vec![self.text_width_twips / u32::try_from(columns).unwrap_or(1).max(1); columns]
        };
        // A table from an editor has no look and takes the grid; one read from a docx keeps the
        // borders it had.
        let from_editor = *look == TableLook::default();
        xml.push_str("<w:tbl><w:tblPr>");
        if from_editor || look.borders {
            xml.push_str(r#"<w:tblStyle w:val="TableGrid"/>"#);
        }
        if from_editor {
            xml.push_str(r#"<w:tblW w:w="5000" w:type="pct"/>"#);
        } else {
            let total: u32 = widths.iter().sum();
            let _ = write!(xml, r#"<w:tblW w:w="{total}" w:type="dxa"/>"#);
        }
        if look.edges != Borders::default() {
            let _ = write!(
                xml,
                "<w:tblBorders>{}</w:tblBorders>",
                edges_xml(&look.edges, true)
            );
        }
        if look.cell_margins_twips.iter().any(Option::is_some) {
            xml.push_str("<w:tblCellMar>");
            for (side, name) in ["top", "left", "bottom", "right"].iter().enumerate() {
                if let Some(margin) = look.cell_margins_twips[side] {
                    let _ = write!(xml, r#"<w:{name} w:w="{margin}" w:type="dxa"/>"#);
                }
            }
            xml.push_str("</w:tblCellMar>");
        }
        xml.push_str(concat!(
            r#"<w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/>"#,
            "</w:tblPr><w:tblGrid>"
        ));
        for width in &widths {
            let _ = write!(xml, r#"<w:gridCol w:w="{width}"/>"#);
        }
        xml.push_str("</w:tblGrid>");
        for (row_index, row) in rows.iter().enumerate() {
            xml.push_str("<w:tr>");
            if row.header {
                xml.push_str("<w:trPr><w:tblHeader/></w:trPr>");
            }
            let mut column = 0;
            for slot in &placed.rows[row_index] {
                let span = usize::try_from(slot.col_span).unwrap_or(1);
                let width: u32 = widths.iter().skip(column).take(span).sum();
                column += span;
                xml.push_str("<w:tc><w:tcPr>");
                let _ = write!(xml, r#"<w:tcW w:w="{width}" w:type="dxa"/>"#);
                if span > 1 {
                    let _ = write!(xml, r#"<w:gridSpan w:val="{span}"/>"#);
                }
                match slot.kind {
                    SlotKind::Cell {
                        merge_start: true, ..
                    } => {
                        xml.push_str(r#"<w:vMerge w:val="restart"/>"#);
                    }
                    SlotKind::Continuation => xml.push_str("<w:vMerge/>"),
                    SlotKind::Cell { .. } => {}
                }
                let cell_look = match slot.kind {
                    SlotKind::Cell { cell, .. } => &row.cells[cell].look,
                    SlotKind::Continuation => &CellLook {
                        fill: None,
                        borders: Borders::default(),
                        vertical: None,
                    },
                };
                cell_look_properties(cell_look, xml);
                xml.push_str("</w:tcPr>");
                let mut content = String::new();
                if let SlotKind::Cell { cell, .. } = slot.kind {
                    self.blocks(&row.cells[cell].blocks, None, &mut content);
                }
                // A cell must end with a paragraph.
                if !content.ends_with("</w:p>") {
                    content.push_str("<w:p/>");
                }
                xml.push_str(&content);
                xml.push_str("</w:tc>");
            }
            xml.push_str("</w:tr>");
        }
        xml.push_str("</w:tbl>");
    }

    fn relationship(&mut self, kind: &'static str, target: String, external: bool) -> String {
        if let Some(existing) = self
            .relationships
            .iter()
            .find(|relationship| relationship.kind == kind && relationship.target == target)
        {
            return existing.id.clone();
        }
        let id = format!(
            "rId{}",
            self.relationship_base + self.relationships.len() + 1
        );
        self.relationships.push(Relationship {
            id: id.clone(),
            kind,
            target,
            external,
        });
        id
    }
}

/// The `w:rPr` of a run, in schema order, and the target of its link: the marks over the
/// paragraph font, which a tree read from a docx carries.
fn run_properties<'m>(
    marks: &'m [Mark],
    look: &RunLook,
    paragraph_font: &RunLook,
) -> (String, Option<&'m str>) {
    let mut link = None;
    let mut character_style = None;
    let mut family = paragraph_font.family.clone();
    let mut size = paragraph_font.size_half_points;
    let mut colour = paragraph_font.colour.clone();
    let mut highlight = paragraph_font.highlight.clone();
    for mark in marks {
        match mark {
            Mark::Link { href } => link = Some(href.as_str()),
            Mark::Code => {
                character_style = Some("CodeChar");
                family = None;
            }
            Mark::TextStyle {
                font_family,
                font_size,
                color,
            } => {
                if let Some(font_family) = font_family {
                    family = Some(font_family.clone());
                }
                if let Some(points) = font_size.as_deref().and_then(half_points) {
                    size = Some(points);
                }
                if let Some(color) = color.as_deref().and_then(css_hex) {
                    colour = Some(color);
                }
            }
            Mark::Highlight { color } => highlight = css_hex(color),
            _ => {}
        }
    }
    if link.is_some() {
        character_style = character_style.or(Some("Hyperlink"));
        colour = None;
    }
    let has = |mark: &Mark| marks.contains(mark);
    let bold = has(&Mark::Bold) || paragraph_font.bold == Some(true);
    let italic = has(&Mark::Italic) || paragraph_font.italic == Some(true);

    // Word wants the run properties in schema order.
    let mut properties = String::new();
    if let Some(style) = character_style {
        let _ = write!(properties, r#"<w:rStyle w:val="{style}"/>"#);
    }
    if let Some(family) = &family {
        let family = escape(family);
        let _ = write!(
            properties,
            r#"<w:rFonts w:ascii="{family}" w:hAnsi="{family}" w:cs="{family}"/>"#
        );
    }
    if bold {
        properties.push_str("<w:b/>");
    }
    if italic {
        properties.push_str("<w:i/>");
    }
    if look.caps == Some(true) || paragraph_font.caps == Some(true) {
        properties.push_str("<w:caps/>");
    }
    if look.small_caps == Some(true) || paragraph_font.small_caps == Some(true) {
        properties.push_str("<w:smallCaps/>");
    }
    if has(&Mark::Strike) {
        properties.push_str("<w:strike/>");
    }
    if let Some(colour) = &colour {
        let _ = write!(properties, r#"<w:color w:val="{colour}"/>"#);
    }
    if let Some(size) = size {
        let _ = write!(
            properties,
            r#"<w:sz w:val="{size}"/><w:szCs w:val="{size}"/>"#
        );
    }
    if has(&Mark::Underline) {
        properties.push_str(r#"<w:u w:val="single"/>"#);
    }
    if let Some(highlight) = &highlight {
        let _ = write!(
            properties,
            r#"<w:shd w:val="clear" w:color="auto" w:fill="{highlight}"/>"#
        );
    }
    if has(&Mark::Superscript) {
        properties.push_str(r#"<w:vertAlign w:val="superscript"/>"#);
    } else if has(&Mark::Subscript) {
        properties.push_str(r#"<w:vertAlign w:val="subscript"/>"#);
    }
    let run_properties = if properties.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{properties}</w:rPr>")
    };
    (run_properties, link)
}

/// The paragraph look as direct formatting, in schema order after `w:numPr` and before `w:jc`.
fn paragraph_look_properties(look: &ParagraphLook, properties: &mut String) {
    if look.borders != Borders::default() {
        let _ = write!(
            properties,
            "<w:pBdr>{}</w:pBdr>",
            edges_xml(&look.borders, false)
        );
    }
    if let Some(fill) = &look.shading {
        let _ = write!(
            properties,
            r#"<w:shd w:val="clear" w:color="auto" w:fill="{fill}"/>"#
        );
    }
    if !look.tabs.is_empty() {
        properties.push_str("<w:tabs>");
        for stop in &look.tabs {
            let kind = match stop.kind {
                TabKind::Left => "left",
                TabKind::Center => "center",
                TabKind::Right => "right",
                TabKind::Decimal => "decimal",
                TabKind::Clear => "clear",
            };
            let leader = match stop.leader {
                TabLeader::None => "",
                TabLeader::Dot => r#" w:leader="dot""#,
                TabLeader::Hyphen => r#" w:leader="hyphen""#,
                TabLeader::Underscore => r#" w:leader="underscore""#,
                TabLeader::MiddleDot => r#" w:leader="middleDot""#,
            };
            let _ = write!(
                properties,
                r#"<w:tab w:val="{kind}"{leader} w:pos="{}"/>"#,
                stop.position_twips
            );
        }
        properties.push_str("</w:tabs>");
    }
    let mut spacing = String::new();
    if let Some(before) = look.space_before_twips {
        let _ = write!(spacing, r#" w:before="{before}""#);
    }
    if let Some(after) = look.space_after_twips {
        let _ = write!(spacing, r#" w:after="{after}""#);
    }
    match look.line_spacing {
        Some(LineSpacing::Auto(line)) => {
            let _ = write!(spacing, r#" w:line="{line}" w:lineRule="auto""#);
        }
        Some(LineSpacing::Exact(line)) => {
            let _ = write!(spacing, r#" w:line="{line}" w:lineRule="exact""#);
        }
        Some(LineSpacing::AtLeast(line)) => {
            let _ = write!(spacing, r#" w:line="{line}" w:lineRule="atLeast""#);
        }
        None => {}
    }
    if !spacing.is_empty() {
        let _ = write!(properties, "<w:spacing{spacing}/>");
    }
    let mut indent = String::new();
    if let Some(left) = look.indent_left_twips {
        let _ = write!(indent, r#" w:left="{left}""#);
    }
    if let Some(right) = look.indent_right_twips {
        let _ = write!(indent, r#" w:right="{right}""#);
    }
    match look.indent_first_line_twips {
        Some(first) if first < 0 => {
            let _ = write!(indent, r#" w:hanging="{}""#, first.unsigned_abs());
        }
        Some(first) => {
            let _ = write!(indent, r#" w:firstLine="{first}""#);
        }
        None => {}
    }
    if !indent.is_empty() {
        let _ = write!(properties, "<w:ind{indent}/>");
    }
}

/// The cell look as `w:tcPr` children, after `w:vMerge`.
fn cell_look_properties(look: &CellLook, xml: &mut String) {
    if look.borders != Borders::default() {
        let _ = write!(
            xml,
            "<w:tcBorders>{}</w:tcBorders>",
            edges_xml(&look.borders, true)
        );
    }
    if let Some(fill) = &look.fill {
        let _ = write!(
            xml,
            r#"<w:shd w:val="clear" w:color="auto" w:fill="{fill}"/>"#
        );
    }
    if let Some(vertical) = look.vertical {
        let value = match vertical {
            VerticalAlignment::Top => "top",
            VerticalAlignment::Center => "center",
            VerticalAlignment::Bottom => "bottom",
        };
        let _ = write!(xml, r#"<w:vAlign w:val="{value}"/>"#);
    }
}

/// Border edges in schema order; `table` names the inside edges as a table does, a paragraph
/// has only the one between paragraphs.
fn edges_xml(borders: &Borders, table: bool) -> String {
    let mut xml = String::new();
    let inside_horizontal = if table { "insideH" } else { "between" };
    let mut edges = vec![
        ("top", &borders.top),
        ("left", &borders.left),
        ("bottom", &borders.bottom),
        ("right", &borders.right),
        (inside_horizontal, &borders.inside_horizontal),
    ];
    if table {
        edges.push(("insideV", &borders.inside_vertical));
    }
    for (name, edge) in edges {
        if let Some(edge) = edge {
            let _ = write!(xml, "<w:{name} {}/>", border_attributes(edge));
        }
    }
    xml
}

fn border_attributes(border: &Border) -> String {
    let style = match border.style {
        BorderStyle::None => return r#"w:val="nil""#.to_owned(),
        BorderStyle::Single => "single",
        BorderStyle::Double => "double",
        BorderStyle::Dotted => "dotted",
        BorderStyle::Dashed => "dashed",
    };
    format!(
        r#"w:val="{style}" w:sz="{}" w:space="0" w:color="{}""#,
        border.width_eighths,
        border.colour.as_deref().unwrap_or("auto")
    )
}

/// The blocks of a note with its reference mark at the start of its first paragraph.
fn with_leading_run(blocks: &str, mark: &str, style: &str) -> String {
    let Some(start) = blocks.find("<w:p>") else {
        return format!(r#"<w:p><w:pPr><w:pStyle w:val="{style}"/></w:pPr>{mark}</w:p>{blocks}"#);
    };
    let after_open = start + "<w:p>".len();
    let insert_at = if blocks[after_open..].starts_with("<w:pPr>") {
        blocks[after_open..]
            .find("</w:pPr>")
            .map_or(after_open, |end| after_open + end + "</w:pPr>".len())
    } else {
        after_open
    };
    format!("{}{mark}{}", &blocks[..insert_at], &blocks[insert_at..])
}

/// A CSS size in points, `"10.5pt"`, as half points.
fn half_points(size: &str) -> Option<u32> {
    let number = size.trim().strip_suffix("pt")?.trim();
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    let whole: u32 = whole.parse().ok()?;
    let half = u32::from(fraction.starts_with(['5', '6', '7', '8', '9']));
    let value = whole.checked_mul(2)?.checked_add(half)?;
    (MIN_HALF_POINTS..=MAX_HALF_POINTS)
        .contains(&value)
        .then_some(value)
}

/// A CSS colour `#RRGGBB` as `RRGGBB`.
fn css_hex(colour: &str) -> Option<String> {
    let hex = colour.trim().strip_prefix('#')?;
    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())).then(|| hex.to_ascii_uppercase())
}

/// What occupies one grid position of a row: a cell of the tree, or the continuation of a cell
/// merged down from a row above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotKind {
    Cell { cell: usize, merge_start: bool },
    Continuation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    kind: SlotKind,
    col_span: u32,
}

struct PlacedTable {
    rows: Vec<Vec<Slot>>,
    columns: usize,
}

/// The slots of every row, with a continuation cell where a merge from above covers the grid, as
/// a docx spells a vertical merge.
fn place_cells(rows: &[TableRow]) -> PlacedTable {
    // Grid column to (rows still covered, column span) of a merge running down.
    let mut running: BTreeMap<usize, (u32, u32)> = BTreeMap::new();
    let mut placed = Vec::with_capacity(rows.len());
    let mut columns = 0;
    for row in rows {
        let mut slots = Vec::new();
        let mut column = 0;
        let mut cells = row.cells.iter().enumerate().peekable();
        loop {
            if let Some(&(remaining, span)) = running.get(&column) {
                slots.push(Slot {
                    kind: SlotKind::Continuation,
                    col_span: span,
                });
                if remaining <= 1 {
                    running.remove(&column);
                } else {
                    running.insert(column, (remaining - 1, span));
                }
                column += usize::try_from(span).unwrap_or(1);
                continue;
            }
            let Some((index, cell)) = cells.next() else {
                // Merges still running at the end of a short row are closed there.
                let tail: Vec<usize> = running.range(column..).map(|(start, _)| *start).collect();
                for start in tail {
                    let (remaining, span) = running[&start];
                    slots.push(Slot {
                        kind: SlotKind::Continuation,
                        col_span: span,
                    });
                    if remaining <= 1 {
                        running.remove(&start);
                    } else {
                        running.insert(start, (remaining - 1, span));
                    }
                    column = start + usize::try_from(span).unwrap_or(1);
                }
                break;
            };
            let span = cell.col_span.max(1);
            let merge_start = cell.row_span > 1;
            if merge_start {
                running.insert(column, (cell.row_span - 1, span));
            }
            slots.push(Slot {
                kind: SlotKind::Cell {
                    cell: index,
                    merge_start,
                },
                col_span: span,
            });
            column += usize::try_from(span).unwrap_or(1);
        }
        columns = columns.max(column);
        placed.push(slots);
    }
    PlacedTable {
        rows: placed,
        columns,
    }
}

/// Runs for text that may hold tabs and line breaks, each with the same properties.
/// The run that refers to a comment, after the end of its range.
fn comment_reference(id: u32, xml: &mut String) {
    let _ = write!(
        xml,
        r#"<w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="{id}"/></w:r>"#
    );
}

/// The `w14:paraId` of a written comment's last paragraph: below `0x80000000`, as Word requires.
fn paragraph_id(comment: u32) -> u32 {
    0x1000_0000 + comment
}

/// Whether a date is the ISO 8601 date and time Word writes, `2026-10-27T10:30:00Z`, so no
/// other text reaches an attribute that takes a date.
fn is_date(date: &str) -> bool {
    date.len() <= 32
        && date.len() >= 10
        && date.bytes().all(|byte| {
            byte.is_ascii_digit() || matches!(byte, b'-' | b':' | b'T' | b'Z' | b'.' | b'+')
        })
}

fn push_text_runs(xml: &mut String, text: &str, run_properties: &str) {
    push_runs(xml, text, run_properties, false);
}

/// The runs of a text, its line breaks and tabs each a run of their own; deleted text is
/// `w:delText`.
fn push_runs(xml: &mut String, text: &str, run_properties: &str, deleted: bool) {
    let element = if deleted { "delText" } else { "t" };
    for (line_index, line) in text.split('\n').enumerate() {
        if line_index > 0 {
            let _ = write!(xml, "<w:r>{run_properties}<w:br/></w:r>");
        }
        for (piece_index, piece) in line.split('\t').enumerate() {
            if piece_index > 0 {
                let _ = write!(xml, "<w:r>{run_properties}<w:tab/></w:r>");
            }
            if !piece.is_empty() {
                let _ = write!(
                    xml,
                    r#"<w:r>{run_properties}<w:{element} xml:space="preserve">{}</w:{element}></w:r>"#,
                    escape(piece)
                );
            }
        }
    }
}

/// Text escaped for XML, with the characters XML 1.0 cannot hold dropped.
pub(crate) fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\t' | '\n' | '\r' => escaped.push(character),
            character if character.is_control() => {}
            '\u{FFFE}' | '\u{FFFF}' => {}
            character => escaped.push(character),
        }
    }
    escaped
}

pub(crate) fn image_extension(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpeg"),
        "image/gif" => Some("gif"),
        "image/bmp" => Some("bmp"),
        "image/tiff" => Some("tiff"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        "image/emf" => Some("emf"),
        "image/wmf" => Some("wmf"),
        _ => None,
    }
}

fn text_width(layout: &PageLayout) -> u32 {
    let [_, right, _, left] = layout.margins_twips;
    layout
        .width_twips
        .saturating_sub(left)
        .saturating_sub(right)
        .max(1_440)
}

fn section_properties(section: &Section, references: &str) -> String {
    let layout = &section.layout;
    let [top, right, bottom, left] = layout.margins_twips;
    let orientation = if layout.width_twips > layout.height_twips {
        r#" w:orient="landscape""#
    } else {
        ""
    };
    let columns = if layout.columns > 1 {
        format!(
            r#"<w:cols w:num="{}" w:space="{}"/>"#,
            layout.columns, layout.column_gap_twips
        )
    } else {
        String::new()
    };
    let title_page = if section.headers_footers.first_header.is_some()
        || section.headers_footers.first_footer.is_some()
    {
        "<w:titlePg/>"
    } else {
        ""
    };
    let kind = if section.continuous {
        r#"<w:type w:val="continuous"/>"#
    } else {
        ""
    };
    format!(
        concat!(
            r#"<w:sectPr>{}{}<w:pgSz w:w="{}" w:h="{}"{}/>"#,
            r#"<w:pgMar w:top="{}" w:right="{}" w:bottom="{}" w:left="{}" w:header="{}" w:footer="{}" w:gutter="0"/>"#,
            "{}{}</w:sectPr>"
        ),
        references,
        kind,
        layout.width_twips,
        layout.height_twips,
        orientation,
        top,
        right,
        bottom,
        left,
        layout.header_distance_twips,
        layout.footer_distance_twips,
        columns,
        title_page
    )
}

fn settings(tree: &DocumentTree, sections: &[Section], embeds_fonts: bool) -> String {
    let even = if sections.iter().any(|section| {
        section.headers_footers.even_header.is_some()
            || section.headers_footers.even_footer.is_some()
    }) {
        "<w:evenAndOddHeaders/>"
    } else {
        ""
    };
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings {}>"#,
            r#"{}<w:defaultTabStop w:val="{}"/>{}"#,
            r#"<w:footnotePr><w:footnote w:id="-1"/><w:footnote w:id="0"/></w:footnotePr>"#,
            r#"<w:endnotePr><w:endnote w:id="-1"/><w:endnote w:id="0"/></w:endnotePr>"#,
            "{}</w:settings>"
        ),
        NAMESPACES,
        // The faces stay with the file, as Word's "Embed fonts in the file" keeps them.
        if embeds_fonts {
            "<w:embedTrueTypeFonts/>"
        } else {
            ""
        },
        tree.layout.default_tab_twips,
        even,
        // Word since 2007 keeps the larger of two spaces; this asks for their sum.
        if tree.layout.spacing_adds_up {
            "<w:compat><w:doNotUseHTMLParagraphAutoSpacing/></w:compat>"
        } else {
            ""
        }
    )
}

/// A notes part with Word's two separators before the notes.
fn notes_part(root: &str, element: &str, notes: &str) -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:{root} {namespaces}>"#,
            r#"<w:{element} w:type="separator" w:id="-1"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:separator/></w:r></w:p></w:{element}>"#,
            r#"<w:{element} w:type="continuationSeparator" w:id="0"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:{element}>"#,
            "{notes}</w:{root}>"
        ),
        root = root,
        element = element,
        namespaces = NAMESPACES,
        notes = notes,
    )
}

/// The content type prefix of the parts of a word processing package.
const WORD_CONTENT_TYPE: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml";

/// A length in CSS pixels as points, to the hundredth.
fn points_of_pixels(pixels: u32) -> String {
    let points = f64::from(pixels) * 0.75;
    format!("{}", (points * 100.0).round() / 100.0)
}

fn content_types(header_parts: &[String], objects: &[(String, String)]) -> String {
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
        r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#,
        r#"<Default Extension="xml" ContentType="application/xml"/>"#,
    ));
    for (extension, content_type) in [
        ("png", "image/png"),
        ("jpeg", "image/jpeg"),
        ("gif", "image/gif"),
        ("bmp", "image/bmp"),
        ("tiff", "image/tiff"),
        ("webp", "image/webp"),
        ("svg", "image/svg+xml"),
        ("emf", "image/x-emf"),
        ("wmf", "image/x-wmf"),
    ] {
        let _ = write!(
            xml,
            r#"<Default Extension="{extension}" ContentType="{content_type}"/>"#
        );
    }
    for (part, kind) in [
        ("document", "document.main"),
        ("styles", "styles"),
        ("numbering", "numbering"),
        ("settings", "settings"),
        ("footnotes", "footnotes"),
        ("endnotes", "endnotes"),
        ("fontTable", "fontTable"),
    ] {
        let _ = write!(
            xml,
            r#"<Override PartName="/word/{part}.xml" ContentType="{WORD_CONTENT_TYPE}.{kind}+xml"/>"#
        );
    }
    xml.push_str(
        r#"<Default Extension="odttf" ContentType="application/vnd.openxmlformats-officedocument.obfuscatedFont"/>"#,
    );
    for part in header_parts {
        let kind = if part.starts_with("header") {
            "header"
        } else if part.starts_with("commentsExtended") {
            "commentsExtended"
        } else if part.starts_with("comments.") {
            "comments"
        } else if part.starts_with("_rels") {
            continue;
        } else {
            "footer"
        };
        let _ = write!(
            xml,
            r#"<Override PartName="/word/{part}" ContentType="{WORD_CONTENT_TYPE}.{kind}+xml"/>"#
        );
    }
    for (part, content_type) in objects {
        let _ = write!(
            xml,
            r#"<Override PartName="/word/{part}" ContentType="{content_type}"/>"#
        );
    }
    xml.push_str("</Types>");
    xml
}

fn package_relationships() -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
            r#"<Relationship Id="rId1" Type="{}/officeDocument" Target="word/document.xml"/>"#,
            "</Relationships>"
        ),
        RELATIONSHIP_TYPE
    )
}

/// A relationships part; the main part's also declares the parts every package has.
fn relationships_part(relationships: &[Relationship], main: bool) -> String {
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    ));
    if main {
        for (index, (kind, target)) in [
            ("styles", "styles.xml"),
            ("numbering", "numbering.xml"),
            ("settings", "settings.xml"),
            ("footnotes", "footnotes.xml"),
            ("endnotes", "endnotes.xml"),
            ("fontTable", "fontTable.xml"),
        ]
        .iter()
        .enumerate()
        {
            let _ = write!(
                xml,
                r#"<Relationship Id="rId{}" Type="{RELATIONSHIP_TYPE}/{kind}" Target="{target}"/>"#,
                index + 1
            );
        }
    }
    for relationship in relationships {
        let mode = if relationship.external {
            r#" TargetMode="External""#
        } else {
            ""
        };
        let kind = if relationship.kind.starts_with("http") {
            relationship.kind.to_owned()
        } else {
            format!("{RELATIONSHIP_TYPE}/{}", relationship.kind)
        };
        let _ = write!(
            xml,
            r#"<Relationship Id="{}" Type="{kind}" Target="{}"{mode}/>"#,
            relationship.id,
            escape(&relationship.target)
        );
    }
    xml.push_str("</Relationships>");
    xml
}

/// The numbering part: abstract list 0 bulleted, 1 numbered, and one instance per list of the
/// tree, so each list counts from one.
fn numbering(lists: &[(u32, bool, u32)]) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering {NAMESPACES}>"#
    );
    for (abstract_id, ordered) in [(0, false), (1, true)] {
        let _ = write!(
            xml,
            r#"<w:abstractNum w:abstractNumId="{abstract_id}"><w:multiLevelType w:val="hybridMultilevel"/>"#
        );
        for level in 0..=MAX_LIST_LEVEL {
            let indent = 720 * (u32::from(level) + 1);
            let (format, text) = if ordered {
                (
                    ["decimal", "lowerLetter", "lowerRoman"][usize::from(level % 3)],
                    format!("%{}.", level + 1),
                )
            } else {
                (
                    "bullet",
                    ["\u{2022}", "o", "\u{25aa}"][usize::from(level % 3)].to_owned(),
                )
            };
            let _ = write!(
                xml,
                concat!(
                    r#"<w:lvl w:ilvl="{level}"><w:start w:val="1"/><w:numFmt w:val="{format}"/>"#,
                    r#"<w:lvlText w:val="{text}"/><w:lvlJc w:val="left"/>"#,
                    r#"<w:pPr><w:ind w:left="{indent}" w:hanging="360"/></w:pPr></w:lvl>"#
                ),
                level = level,
                format = format,
                text = escape(&text),
                indent = indent,
            );
        }
        xml.push_str("</w:abstractNum>");
    }
    // Each list is an abstract definition of its own, so no two lists share their numbers.
    for (num_id, ordered, _) in lists {
        let _ = write!(
            xml,
            concat!(
                r#"<w:num w:numId="{num_id}"><w:abstractNumId w:val="{abstract_id}"/>"#,
                r#"<w:lvlOverride w:ilvl="0"><w:startOverride w:val="1"/></w:lvlOverride></w:num>"#
            ),
            num_id = num_id,
            abstract_id = u8::from(*ordered),
        );
    }
    xml.push_str("</w:numbering>");
    xml
}

/// The styles part: the document's body font and size as its defaults, Calibri 11 when it
/// names none, then [`STYLES`].
fn styles_part(layout: &PageLayout) -> String {
    let family = escape(layout.font_family.as_deref().map_or("Calibri", str::trim));
    let size = layout
        .font_size_half_points
        .filter(|size| (2..=3_276).contains(size))
        .unwrap_or(22);
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
            r#"<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="{family}" w:hAnsi="{family}" w:eastAsia="{family}" w:cs="{family}"/>"#,
            r#"<w:sz w:val="{size}"/><w:szCs w:val="{size}"/><w:lang w:val="it-IT"/></w:rPr></w:rPrDefault>"#,
            r#"<w:pPrDefault><w:pPr><w:spacing w:after="160" w:line="259" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>"#,
            "{styles}"
        ),
        family = family,
        size = size,
        styles = STYLES,
    )
}

/// Word's built-in styles the writer uses, with the look of Word's default template.
const STYLES: &str = concat!(
    r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="240" w:after="0"/><w:outlineLvl w:val="0"/></w:pPr>"#,
    r#"<w:rPr><w:rFonts w:ascii="Calibri Light" w:hAnsi="Calibri Light"/><w:color w:val="2F5496"/><w:sz w:val="32"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="40" w:after="0"/><w:outlineLvl w:val="1"/></w:pPr>"#,
    r#"<w:rPr><w:rFonts w:ascii="Calibri Light" w:hAnsi="Calibri Light"/><w:color w:val="2F5496"/><w:sz w:val="26"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="40" w:after="0"/><w:outlineLvl w:val="2"/></w:pPr>"#,
    r#"<w:rPr><w:rFonts w:ascii="Calibri Light" w:hAnsi="Calibri Light"/><w:color w:val="1F3763"/><w:sz w:val="24"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading4"><w:name w:val="heading 4"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="40" w:after="0"/><w:outlineLvl w:val="3"/></w:pPr>"#,
    r#"<w:rPr><w:i/><w:color w:val="2F5496"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading5"><w:name w:val="heading 5"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="40" w:after="0"/><w:outlineLvl w:val="4"/></w:pPr>"#,
    r#"<w:rPr><w:color w:val="2F5496"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading6"><w:name w:val="heading 6"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="40" w:after="0"/><w:outlineLvl w:val="5"/></w:pPr>"#,
    r#"<w:rPr><w:color w:val="1F3763"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:spacing w:before="200"/><w:ind w:left="864" w:right="864"/></w:pPr>"#,
    r#"<w:rPr><w:i/><w:color w:val="404040"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/><w:contextualSpacing/></w:pPr>"#,
    r#"<w:rPr><w:rFonts w:ascii="Calibri Light" w:hAnsi="Calibri Light"/><w:spacing w:val="-10"/><w:kern w:val="28"/><w:sz w:val="56"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Subtitle"><w:name w:val="Subtitle"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:rPr><w:color w:val="5A5A5A"/><w:spacing w:val="15"/><w:sz w:val="22"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="IntenseQuote"><w:name w:val="Intense Quote"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:pBdr><w:top w:val="single" w:sz="4" w:space="10" w:color="4472C4"/><w:bottom w:val="single" w:sz="4" w:space="10" w:color="4472C4"/></w:pBdr>"#,
    r#"<w:spacing w:before="360" w:after="360"/><w:ind w:left="864" w:right="864"/><w:jc w:val="center"/></w:pPr>"#,
    r#"<w:rPr><w:i/><w:color w:val="4472C4"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Caption"><w:name w:val="caption"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:spacing w:after="200" w:line="240" w:lineRule="auto"/></w:pPr>"#,
    r#"<w:rPr><w:i/><w:color w:val="44546A"/><w:sz w:val="18"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="NoSpacing"><w:name w:val="No Spacing"/><w:qFormat/>"#,
    r#"<w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:ind w:left="720"/><w:contextualSpacing/></w:pPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Code"><w:name w:val="Code"/><w:basedOn w:val="Normal"/>"#,
    r#"<w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr>"#,
    r#"<w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas"/><w:sz w:val="20"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="FootnoteText"><w:name w:val="footnote text"/><w:basedOn w:val="Normal"/>"#,
    r#"<w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="20"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="EndnoteText"><w:name w:val="endnote text"/><w:basedOn w:val="Normal"/>"#,
    r#"<w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="20"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Header"><w:name w:val="header"/><w:basedOn w:val="Normal"/>"#,
    r#"<w:pPr><w:tabs><w:tab w:val="center" w:pos="4819"/><w:tab w:val="right" w:pos="9638"/></w:tabs><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:style>"#,
    r#"<w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/><w:uiPriority w:val="1"/><w:semiHidden/></w:style>"#,
    r#"<w:style w:type="character" w:styleId="Hyperlink"><w:name w:val="Hyperlink"/><w:basedOn w:val="DefaultParagraphFont"/>"#,
    r#"<w:rPr><w:color w:val="0563C1"/><w:u w:val="single"/></w:rPr></w:style>"#,
    r#"<w:style w:type="character" w:styleId="CodeChar"><w:name w:val="Code Char"/><w:basedOn w:val="DefaultParagraphFont"/>"#,
    r#"<w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas"/><w:sz w:val="20"/></w:rPr></w:style>"#,
    r#"<w:style w:type="character" w:styleId="FootnoteReference"><w:name w:val="footnote reference"/><w:basedOn w:val="DefaultParagraphFont"/>"#,
    r#"<w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style>"#,
    r#"<w:style w:type="character" w:styleId="EndnoteReference"><w:name w:val="endnote reference"/><w:basedOn w:val="DefaultParagraphFont"/>"#,
    r#"<w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="CommentText"><w:name w:val="annotation text"/><w:basedOn w:val="Normal"/>"#,
    r#"<w:pPr><w:spacing w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="20"/></w:rPr></w:style>"#,
    r#"<w:style w:type="character" w:styleId="CommentReference"><w:name w:val="annotation reference"/><w:basedOn w:val="DefaultParagraphFont"/>"#,
    r#"<w:rPr><w:sz w:val="16"/></w:rPr></w:style>"#,
    r#"<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:semiHidden/>"#,
    r#"<w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/>"#,
    r#"<w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>"#,
    r#"<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:basedOn w:val="TableNormal"/>"#,
    r#"<w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr>"#,
    r#"<w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/>"#,
    r#"<w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/>"#,
    r#"<w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/>"#,
    r#"<w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/></w:tblBorders></w:tblPr></w:style>"#,
    "</w:styles>"
);

/// The frame of a block set in a text box.
fn block_frame(block: &Block) -> Option<&TextFrame> {
    match block {
        Block::Paragraph { look, .. }
        | Block::Heading { look, .. }
        | Block::ListItem { look, .. } => look.frame.as_ref(),
        Block::Table { look, .. } => look.frame.as_ref(),
        Block::Blockquote { .. } | Block::CodeBlock { .. } | Block::HorizontalRule => None,
    }
}

/// A block of a text box as the box's content, without the frame that put it there.
fn without_frame(block: &Block) -> Block {
    let mut block = block.clone();
    match &mut block {
        Block::Paragraph { look, .. }
        | Block::Heading { look, .. }
        | Block::ListItem { look, .. } => look.frame = None,
        Block::Table { look, .. } => look.frame = None,
        Block::Blockquote { .. } | Block::CodeBlock { .. } | Block::HorizontalRule => {}
    }
    block
}

mod floating;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod parts_tests;
