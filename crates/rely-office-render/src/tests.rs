// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use std::fmt::Write as _;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::LazyLock;

use zip::write::SimpleFileOptions;

use super::*;

const DOCX_CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
const DOCX_NAMESPACES: &str = concat!(
    r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture""#,
);

static RENDERER: LazyLock<Renderer> = LazyLock::new(|| {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fonts");
    let fonts = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ttf"))
        .map(|path| std::fs::read(path).unwrap());
    Renderer::new(fonts)
});

fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in members {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn docx(body: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document {DOCX_NAMESPACES}><w:body>{body}</w:body></w:document>"#
    );
    let mut members: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", DOCX_CONTENT_TYPES.as_bytes()),
        ("word/document.xml", document.as_bytes()),
    ];
    members.extend_from_slice(extra);
    zip(&members)
}

fn paragraphs(count: usize) -> String {
    let mut body = String::new();
    for index in 0..count {
        let _ = write!(
            body,
            "<w:p><w:r><w:t>Paragrafo {index}: il contratto si rinnova tacitamente di anno in anno salvo disdetta.</w:t></w:r></w:p>"
        );
    }
    body
}

fn image_docx(image: &[u8]) -> Vec<u8> {
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/a.png"/></Relationships>"#;
    let body = concat!(
        r#"<w:p><w:r><w:drawing><wp:inline><wp:extent cx="952500" cy="476250"/><wp:docPr id="1" name="logo"/>"#,
        r#"<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
        r#"<pic:pic><pic:blipFill><a:blip r:embed="rId1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic>"#,
        r#"</wp:inline></w:drawing></w:r></w:p>"#
    );
    docx(
        body,
        &[
            ("word/_rels/document.xml.rels", rels.as_bytes()),
            ("word/media/a.png", image),
        ],
    )
}

fn thumbnail_request(width: u32) -> RenderRequest {
    RenderRequest {
        pdf: true,
        thumbnail_width: Some(width),
        ..RenderRequest::default()
    }
}

fn png_size(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
    (width, height)
}

#[test]
fn holds_the_bundled_fonts() {
    assert_eq!(RENDERER.font_count(), 19);
    assert_eq!(bundled_font_families().len(), 5);
}

#[test]
fn renders_a_docx_to_a_pdf_and_a_thumbnail_of_its_page_size() {
    let body = format!(
        "{}<w:sectPr><w:pgSz w:w=\"16838\" w:h=\"11906\"/></w:sectPr>",
        paragraphs(3)
    );
    let rendered = RENDERER
        .render(
            &docx(&body, &[]),
            Some("contratto.docx"),
            &thumbnail_request(300),
        )
        .unwrap();
    assert_eq!(rendered.format, DocumentFormat::Docx);
    assert_eq!(rendered.fidelity, Fidelity::Layout);
    assert_eq!(rendered.page_count, 1);
    assert!(rendered.pdf.unwrap().starts_with(b"%PDF-"));
    let (width, height) = png_size(&rendered.thumbnail.unwrap());
    assert_eq!(width, 300);
    assert_eq!(height, 212, "landscape A4 at 300 pixels wide");
    assert_eq!(rendered.page_images.len(), 0);
    assert!(!rendered.truncated);
}

#[test]
fn flows_a_long_docx_over_several_pages_and_images_each() {
    let request = RenderRequest {
        pdf: false,
        page_images: Some(PageImages {
            pixels_per_point: 0.5,
            max_pages: 2,
        }),
        ..RenderRequest::default()
    };
    let rendered = RENDERER
        .render(&docx(&paragraphs(200), &[]), None, &request)
        .unwrap();
    assert!(rendered.page_count >= 3, "{} pages", rendered.page_count);
    assert!(rendered.pdf.is_none());
    assert!(rendered.thumbnail.is_none());
    assert_eq!(rendered.page_images.len(), 2);
    assert_eq!(png_size(&rendered.page_images[0]), (298, 421));
}

#[test]
fn renders_the_structures_of_a_document() {
    let body = concat!(
        r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Contratto</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t>Oggetto</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t xml:space="preserve">Testo </w:t></w:r><w:r><w:rPr><w:i/><w:u w:val="single"/></w:rPr><w:t>sottolineato</w:t></w:r><w:r><w:br/></w:r><w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:t>1</w:t></w:r></w:p>"#,
        r#"<w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>Voce</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Importo</w:t></w:r></w:p></w:tc></w:tr>"#,
        r#"<w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>Totale</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="single"/></w:pBdr></w:pPr></w:p>"#,
        r#"<w:p/>"#,
    );
    let rendered = RENDERER
        .render(&docx(body, &[]), None, &RenderRequest::default())
        .unwrap();
    assert_eq!(rendered.page_count, 1);
}

#[test]
fn draws_a_frame_for_an_image_typst_cannot_decode() {
    let blank = RENDERER
        .render_source(
            &TypstSource {
                format: DocumentFormat::Unknown,
                fidelity: Fidelity::Layout,
                source: "#set page(width: 20pt, height: 10pt)".to_owned(),
                files: Vec::new(),
                truncated: false,
            },
            &RenderRequest {
                pdf: false,
                page_images: Some(PageImages {
                    pixels_per_point: 1.0,
                    max_pages: 1,
                }),
                ..RenderRequest::default()
            },
        )
        .unwrap()
        .page_images
        .remove(0);
    assert_eq!(png_size(&blank), (20, 10));

    let good = image_docx(&blank);
    assert_eq!(
        typst_source(&good, None, &render_limits())
            .unwrap()
            .files
            .len(),
        1
    );
    assert_eq!(
        RENDERER
            .render(&good, None, &RenderRequest::default())
            .unwrap()
            .page_count,
        1
    );

    let mut damaged = b"\x89PNG\r\n\x1a\n".to_vec();
    damaged.extend_from_slice(&[0; 64]);
    let rendered = RENDERER
        .render(&image_docx(&damaged), None, &RenderRequest::default())
        .unwrap();
    assert_eq!(rendered.page_count, 1);
}

#[test]
fn renders_the_sheets_of_a_workbook() {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sales = workbook.add_worksheet().set_name("Vendite").unwrap();
    sales.write(0, 0, "Mese").unwrap();
    sales.write(0, 1, "Ricavi").unwrap();
    for row in 1..80 {
        sales.write(row, 0, format!("Riga {row}")).unwrap();
        sales.write(row, 1, f64::from(row) * 10.5).unwrap();
    }
    workbook
        .add_worksheet()
        .set_name("Clienti")
        .unwrap()
        .write(0, 0, "Acme")
        .unwrap();
    let bytes = workbook.save_to_buffer().unwrap();

    let rendered = RENDERER
        .render(&bytes, Some("vendite.xlsx"), &thumbnail_request(200))
        .unwrap();
    assert_eq!(rendered.format, DocumentFormat::Xlsx);
    assert_eq!(rendered.fidelity, Fidelity::Layout);
    assert!(rendered.page_count >= 3, "{} pages", rendered.page_count);
    assert_eq!(png_size(&rendered.thumbnail.unwrap()).0, 200);

    let csv = RENDERER
        .render(b"a,b\n1,2\n", Some("dati.csv"), &RenderRequest::default())
        .unwrap();
    assert_eq!((csv.format, csv.page_count), (DocumentFormat::Csv, 1));

    let markdown = RENDERER
        .render(
            b"# Titolo\n\n- uno\n- due\n",
            Some("nota.md"),
            &RenderRequest::default(),
        )
        .unwrap();
    assert_eq!(
        (markdown.format, markdown.page_count),
        (DocumentFormat::Markdown, 1)
    );
    let page = RENDERER
        .render(
            b"<!doctype html><h1>Titolo</h1><p>Testo</p>",
            None,
            &RenderRequest::default(),
        )
        .unwrap();
    assert_eq!((page.format, page.page_count), (DocumentFormat::Html, 1));
}

#[test]
fn says_a_sheet_was_cut_to_the_limits() {
    let request = RenderRequest {
        limits: Limits {
            max_rows: 2,
            ..render_limits()
        },
        ..RenderRequest::default()
    };
    let rendered = RENDERER
        .render(b"a\n1\n2\n3\n", Some("dati.csv"), &request)
        .unwrap();
    assert!(rendered.truncated);
}

#[test]
fn lays_out_an_odt_from_its_document_tree() {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("mimetype", stored).unwrap();
    writer
        .write_all(b"application/vnd.oasis.opendocument.text")
        .unwrap();
    writer
        .start_file("content.xml", SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content "#,
            r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
            r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.2">"#,
            r#"<office:body><office:text><text:p>Verbale #1 [bozza]</text:p></office:text></office:body>"#,
            r#"</office:document-content>"#
        ).as_bytes())
        .unwrap();
    let bytes = writer.finish().unwrap().into_inner();

    let source = typst_source(&bytes, Some("verbale.odt"), &render_limits()).unwrap();
    assert_eq!(source.fidelity, Fidelity::Layout);
    assert!(
        source.source.contains(r#""Verbale #1 [bozza]""#),
        "{}",
        source.source
    );
    let rendered = RENDERER
        .render(&bytes, Some("verbale.odt"), &RenderRequest::default())
        .unwrap();
    assert_eq!(
        (rendered.format, rendered.page_count),
        (DocumentFormat::Odt, 1)
    );
}

#[test]
fn refuses_what_it_cannot_read() {
    assert!(matches!(
        RENDERER.render(b"not an office file", None, &RenderRequest::default()),
        Err(RenderError::UnsupportedFormat(DocumentFormat::Unknown))
    ));
    assert!(matches!(
        RENDERER.render(
            &docx("<w:p", &[]),
            Some("a.docx"),
            &RenderRequest::default()
        ),
        Err(RenderError::Office(_))
    ));
}

#[test]
fn reports_a_source_typst_refuses() {
    let error = RENDERER
        .render_source(
            &TypstSource {
                format: DocumentFormat::Unknown,
                fidelity: Fidelity::Layout,
                source: "#undefined-function()".to_owned(),
                files: Vec::new(),
                truncated: false,
            },
            &RenderRequest::default(),
        )
        .unwrap_err();
    assert!(matches!(error, RenderError::Typeset(message) if message.contains("unknown variable")));
}

#[test]
fn caps_the_thumbnail_width() {
    let rendered = RENDERER
        .render(
            &docx(&paragraphs(1), &[]),
            None,
            &thumbnail_request(u32::MAX),
        )
        .unwrap();
    assert_eq!(
        png_size(&rendered.thumbnail.unwrap()).0,
        MAX_THUMBNAIL_WIDTH
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one document exercises every case together"
)]
fn typesets_headers_footers_notes_tabs_labels_and_cell_looks() {
    use rely_office_core::{
        Block, Border, BorderStyle, Borders, CellLook, DocumentTree, HeadersFooters, Inline, Note,
        NoteKind, PageField, ParagraphLook, RunLook, TabKind, TabLeader, TabStop, TableCell,
        TableLook, TableRow, write_docx,
    };
    let text = |value: &str| Inline::Text {
        text: value.to_owned(),
        marks: Vec::new(),
        look: RunLook::default(),
    };
    let paragraph = |content: Vec<Inline>, look: ParagraphLook| Block::Paragraph {
        content,
        align: None,
        look,
    };
    let page = Inline::Text {
        text: "1".to_owned(),
        marks: Vec::new(),
        look: RunLook {
            field: Some(PageField::Page),
            ..RunLook::default()
        },
    };
    let border = Border {
        style: BorderStyle::Single,
        width_eighths: 8,
        colour: Some("2F5496".to_owned()),
    };
    let tabbed = ParagraphLook {
        tabs: vec![TabStop {
            position_twips: 9_000,
            kind: TabKind::Right,
            leader: TabLeader::Dot,
        }],
        shading: Some("DEEAF6".to_owned()),
        borders: Borders {
            top: Some(border.clone()),
            ..Borders::default()
        },
        ..ParagraphLook::default()
    };
    let cell = |value: &str| TableCell {
        col_span: 1,
        row_span: 1,
        blocks: vec![paragraph(vec![text(value)], ParagraphLook::default())],
        look: CellLook {
            fill: Some("D9E2F3".to_owned()),
            borders: Borders {
                bottom: Some(border.clone()),
                ..Borders::default()
            },
            vertical: Some(rely_office_core::VerticalAlignment::Center),
        },
    };
    let tree = DocumentTree {
        blocks: vec![
            paragraph(
                vec![text("Premesse\t1"), Inline::NoteReference { note: 0 }],
                tabbed,
            ),
            Block::ListItem {
                ordered: true,
                level: 0,
                list_id: 1,
                content: vec![text("voce"), Inline::NoteReference { note: 1 }],
                look: ParagraphLook::default(),
            },
            Block::Table {
                rows: vec![TableRow {
                    header: false,
                    cells: vec![cell("a"), cell("b")],
                }],
                look: TableLook {
                    edges: Borders {
                        inside_vertical: Some(border.clone()),
                        ..Borders::default()
                    },
                    ..TableLook::default()
                },
            },
        ],
        notes: vec![
            Note {
                kind: NoteKind::Footnote,
                blocks: vec![paragraph(vec![text("a piede")], ParagraphLook::default())],
            },
            Note {
                kind: NoteKind::Endnote,
                blocks: vec![paragraph(vec![text("alla fine")], ParagraphLook::default())],
            },
        ],
        headers_footers: HeadersFooters {
            header: vec![paragraph(vec![text("Testa")], ParagraphLook::default())],
            footer: vec![paragraph(
                vec![text("Pagina "), page],
                ParagraphLook::default(),
            )],
            first_header: Some(vec![paragraph(
                vec![text("Copertina")],
                ParagraphLook::default(),
            )]),
            even_footer: Some(Vec::new()),
            ..HeadersFooters::default()
        },
        ..DocumentTree::default()
    };
    let bytes = write_docx(&tree).unwrap();
    let rendered = RENDERER
        .render(&bytes, Some("completo.docx"), &thumbnail_request(200))
        .unwrap();
    assert_eq!(rendered.fidelity, Fidelity::Layout);
    assert_eq!(rendered.page_count, 1);
    assert!(rendered.pdf.unwrap().starts_with(b"%PDF-"));
}

#[test]
fn draws_the_slides_of_a_presentation() {
    const NS: &str = concat!(
        r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
        r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
        r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#
    );
    let presentation = format!(
        r#"<p:presentation {NS}><p:sldIdLst><p:sldId id="256" r:id="rId1"/><p:sldId id="257" r:id="rId2"/></p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/></p:presentation>"#
    );
    let relationships = concat!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>"#,
        r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide2.xml"/>"#,
        r#"</Relationships>"#
    );
    let slide = format!(
        concat!(
            r#"<p:sld {}><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="2" name="Box"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>"#,
            r#"<p:spPr><a:xfrm rot="1800000"><a:off x="914400" y="914400"/><a:ext cx="3657600" cy="1828800"/></a:xfrm>"#,
            r#"<a:prstGeom prst="ellipse"/><a:solidFill><a:srgbClr val="ED7D31"/></a:solidFill></p:spPr>"#,
            r#"<p:txBody><a:bodyPr anchor="ctr"/><a:p><a:pPr algn="ctr"><a:buChar char="&#x2022;"/></a:pPr>"#,
            r#"<a:r><a:rPr sz="2400" b="1"/><a:t>Novit&#xE0;</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#
        ),
        NS
    );
    let content_types = concat!(
        r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
        r#"<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/></Types>"#
    );
    let package_relationships = concat!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>"#,
        r#"</Relationships>"#
    );
    let bytes = zip(&[
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", package_relationships.as_bytes()),
        ("ppt/presentation.xml", presentation.as_bytes()),
        ("ppt/_rels/presentation.xml.rels", relationships.as_bytes()),
        ("ppt/slides/slide1.xml", slide.as_bytes()),
        ("ppt/slides/slide2.xml", slide.as_bytes()),
    ]);
    let rendered = RENDERER
        .render(&bytes, Some("offerta.pptx"), &thumbnail_request(320))
        .unwrap();
    assert_eq!(rendered.format, DocumentFormat::Pptx);
    assert_eq!(rendered.fidelity, Fidelity::Layout);
    assert_eq!(rendered.page_count, 2);
    assert_eq!(
        png_size(&rendered.thumbnail.unwrap()),
        (320, 180),
        "a 16:9 slide"
    );
}

#[test]
fn draws_the_pages_of_an_odp() {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("mimetype", stored).unwrap();
    writer
        .write_all(b"application/vnd.oasis.opendocument.presentation")
        .unwrap();
    writer
        .start_file("content.xml", SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(concat!(
            r#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
            r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
            r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0"><office:body><office:presentation>"#,
            r#"<draw:page><draw:frame svg:x="1cm" svg:y="1cm" svg:width="10cm" svg:height="2cm"><draw:text-box><text:p>Pagina uno</text:p></draw:text-box></draw:frame></draw:page>"#,
            r#"<draw:page/></office:presentation></office:body></office:document-content>"#
        ).as_bytes())
        .unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    let rendered = RENDERER
        .render(&bytes, Some("riunione.odp"), &thumbnail_request(280))
        .unwrap();
    assert_eq!(rendered.format, DocumentFormat::Odp);
    assert_eq!(rendered.fidelity, Fidelity::Layout);
    assert_eq!(rendered.page_count, 2);
    assert_eq!(
        png_size(&rendered.thumbnail.unwrap()),
        (280, 210),
        "a 4:3 page without a master"
    );
}

#[test]
fn draws_metafile_pictures_as_svg_and_leaves_the_rest() {
    let mut wmf: Vec<u8> = [1_u16, 9, 0x0300, 0, 0, 0, 0, 0, 0]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    for (function, parameters) in [(0x041B_u16, vec![5_i16, 5, 0, 0]), (0, Vec::new())] {
        let size = u32::try_from(3 + parameters.len()).unwrap();
        wmf.extend_from_slice(&size.to_le_bytes());
        wmf.extend_from_slice(&function.to_le_bytes());
        wmf.extend(parameters.iter().flat_map(|word| word.to_le_bytes()));
    }
    let mut media = vec![
        Media {
            content_type: "image/wmf",
            bytes: wmf,
        },
        Media {
            content_type: "image/emf",
            bytes: vec![1, 2, 3],
        },
        Media {
            content_type: "image/png",
            bytes: vec![1],
        },
    ];
    draw_metafiles(&mut media);
    assert_eq!(media[0].content_type, "image/svg+xml");
    assert!(media[0].bytes.starts_with(b"<svg"));
    assert_eq!(
        media[1].content_type, "image/emf",
        "a metafile that draws nothing stays"
    );
    assert_eq!(media[2].bytes, vec![1]);
    assert_eq!(metafile_svg("image/png", b"x"), None);
}

#[test]
fn typesets_a_paragraph_between_two_pictures_with_tabs() {
    use rely_office_core::{
        Anchor, AnchorAlign, AnchorBase, AnchorPosition, Block, DocumentTree, Inline, Media,
        ParagraphLook, RunLook, Wrap, write_docx,
    };
    let side = |align| Anchor {
        horizontal: AnchorPosition {
            base: AnchorBase::Column,
            offset_emu: 0,
            align: Some(align),
        },
        vertical: AnchorPosition {
            base: AnchorBase::Paragraph,
            offset_emu: 0,
            align: None,
        },
        width_emu: 1_200_000,
        height_emu: 900_000,
        wrap: Wrap::Around,
        distance_emu: [0; 4],
    };
    let image = |align| Inline::Image {
        media: 0,
        alt: None,
        width: Some(126),
        height: Some(94),
        anchor: Some(Box::new(side(align))),
    };
    let words = "Voce\tvalore e un testo che gira attorno alle due figure. ".repeat(20);
    let tree = DocumentTree {
        blocks: vec![Block::Paragraph {
            content: vec![
                image(AnchorAlign::Start),
                image(AnchorAlign::End),
                Inline::Text {
                    text: words,
                    marks: Vec::new(),
                    look: RunLook::default(),
                },
            ],
            align: None,
            look: ParagraphLook::default(),
        }],
        media: vec![Media {
            content_type: "image/svg+xml",
            bytes: br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>"#.to_vec(),
        }],
        ..DocumentTree::default()
    };
    let bytes = write_docx(&tree).unwrap();
    let source = RENDERER
        .typst_source(&bytes, Some("figure.docx"), &render_limits())
        .unwrap()
        .source;
    assert!(source.contains("#rely-flow(((box(image("), "{source}");
    let rendered = RENDERER
        .render(&bytes, Some("figure.docx"), &thumbnail_request(200))
        .unwrap();
    assert_eq!(rendered.page_count, 1);
}
