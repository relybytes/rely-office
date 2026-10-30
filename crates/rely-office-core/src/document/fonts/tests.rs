// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::opendocument::write_odt;
use crate::test_support::{build_docx_from_body, build_zip, rels_xml, zip_member};
use crate::{PageLayout, read_document_tree, write_docx, write_html};

const REGULAR: &[u8] = include_bytes!("../../../../../fonts/Carlito-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../../../../../fonts/Carlito-Bold.ttf");
const GUID: &str = "{01234567-89AB-CDEF-0123-456789ABCDEF}";

fn text_in(family: &str, value: &str) -> Block {
    Block::Paragraph {
        content: vec![Inline::Text {
            text: value.to_owned(),
            marks: vec![Mark::TextStyle {
                font_family: Some(family.to_owned()),
                font_size: None,
                color: None,
            }],
            look: crate::RunLook::default(),
        }],
        align: None,
        look: crate::ParagraphLook::default(),
    }
}

fn tree_with_embedded_calibri() -> DocumentTree {
    DocumentTree {
        blocks: vec![text_in("Garamond", "Titolo"), text_in("Calibri", "Corpo")],
        layout: PageLayout {
            font_family: Some("Calibri".to_owned()),
            font_size_half_points: Some(24),
            ..PageLayout::default()
        },
        fonts: DocumentFonts {
            families: vec![DocumentFont {
                name: "Calibri".to_owned(),
                generic: FontGeneric::Swiss,
                pitch: FontPitch::Variable,
                panose: Some("020F0502020204030204".to_owned()),
                charset: Some("00".to_owned()),
                faces: vec![
                    FontFace {
                        style: FaceStyle::Regular,
                        file: 0,
                    },
                    FontFace {
                        style: FaceStyle::Bold,
                        file: 1,
                    },
                ],
                ..DocumentFont::default()
            }],
            files: vec![REGULAR.to_vec(), BOLD.to_vec()],
        },
        ..DocumentTree::default()
    }
}

#[test]
fn reads_a_font_key_from_its_last_pair_to_its_first() {
    let key = font_key(GUID).unwrap();
    assert_eq!(key[0], 0xEF);
    assert_eq!(key[1], 0xCD);
    assert_eq!(key[15], 0x01);
    assert_eq!(font_key("{0123}"), None);
    assert_eq!(font_key("{01234567-89AB-CDEF-0123-456789ABCDEZ}"), None);
}

#[test]
fn obfuscation_is_its_own_inverse_and_touches_32_bytes() {
    let key = font_key(GUID).unwrap();
    let mut bytes = REGULAR.to_vec();
    assert!(xor_font(&mut bytes, &key));
    assert_ne!(&bytes[..32], &REGULAR[..32]);
    assert_eq!(&bytes[32..], &REGULAR[32..]);
    assert!(!is_font_file(&bytes) || bytes[..4] != REGULAR[..4]);
    assert!(xor_font(&mut bytes, &key));
    assert_eq!(bytes, REGULAR);
    assert!(!xor_font(&mut [0u8; 31], &key));
}

#[test]
fn makes_the_same_key_for_the_same_file() {
    let first = key_guid_for(REGULAR);
    assert_eq!(first, key_guid_for(REGULAR));
    assert_ne!(first, key_guid_for(BOLD));
    assert!(font_key(&first).is_some());
    assert_eq!(first.len(), 38);
}

#[test]
fn tells_font_files_and_their_faces() {
    assert!(is_font_file(REGULAR));
    assert!(is_font_file(b"OTTO...."));
    assert!(!is_font_file(b"PK\x03\x04"));
    assert_eq!(face_style_of(REGULAR), Some(FaceStyle::Regular));
    assert_eq!(face_style_of(BOLD), Some(FaceStyle::Bold));
    assert_eq!(face_style_of(b"\0\x01\0\0"), None);
    assert!(FaceStyle::BoldItalic.bold() && FaceStyle::BoldItalic.italic());
    assert!(!FaceStyle::Regular.bold() && !FaceStyle::Bold.italic());
}

#[test]
fn substitutes_by_metrics_then_by_kind() {
    assert_eq!(font_substitute("Calibri", FontGeneric::Auto), "Carlito");
    assert_eq!(font_substitute(" Aptos ", FontGeneric::Auto), "Carlito");
    assert_eq!(
        font_substitute("Cambria Math", FontGeneric::Auto),
        "Caladea"
    );
    assert_eq!(
        font_substitute("Segoe UI", FontGeneric::Auto),
        "Liberation Sans"
    );
    assert_eq!(
        font_substitute("Book Antiqua", FontGeneric::Auto),
        "Liberation Serif"
    );
    assert_eq!(
        font_substitute("Consolas", FontGeneric::Auto),
        "Liberation Mono"
    );
    assert_eq!(
        font_substitute("Unknown Serif", FontGeneric::Auto),
        "Liberation Serif"
    );
    assert_eq!(
        font_substitute("Fira Code", FontGeneric::Auto),
        "Liberation Mono"
    );
    assert_eq!(
        font_substitute("Mystery", FontGeneric::Roman),
        "Liberation Serif"
    );
    assert_eq!(
        font_substitute("Mystery", FontGeneric::Swiss),
        "Liberation Sans"
    );
    assert_eq!(
        font_substitute("Mystery", FontGeneric::Modern),
        "Liberation Mono"
    );
    assert_eq!(font_substitute("Mystery", FontGeneric::Auto), "Carlito");
    assert_eq!(font_generic("Georgia"), FontGeneric::Roman);
    assert_eq!(font_generic("Source Code Pro"), FontGeneric::Modern);
    assert_eq!(font_generic("Noto Serif"), FontGeneric::Roman);
    assert_eq!(font_generic("Mystery"), FontGeneric::Auto);
}

#[test]
fn writes_a_css_stack_ending_with_the_generic_family() {
    assert_eq!(
        css_font_stack("Calibri", FontGeneric::Auto),
        "'Calibri', 'Carlito', sans-serif"
    );
    assert_eq!(
        css_font_stack("Mystery", FontGeneric::Roman),
        "'Mystery', 'Liberation Serif', serif"
    );
    assert_eq!(
        css_font_stack("Carlito", FontGeneric::Auto),
        "'Carlito', sans-serif"
    );
    assert_eq!(
        css_font_stack("'\"", FontGeneric::Modern),
        "'Liberation Mono', monospace"
    );
    assert_eq!(
        css_font_stack("A';}b", FontGeneric::Script),
        "'Ab', 'Carlito', cursive"
    );
    assert_eq!(FontGeneric::Decorative.css(), "fantasy");
}

#[test]
fn lists_the_families_a_tree_uses_once_each() {
    let mut tree = tree_with_embedded_calibri();
    tree.notes.push(crate::Note {
        kind: crate::NoteKind::Footnote,
        blocks: vec![text_in("Consolas", "nota")],
    });
    tree.headers_footers.header = vec![Block::Blockquote {
        blocks: vec![text_in("calibri", "testata")],
    }];
    assert_eq!(used_families(&tree), ["Calibri", "Garamond", "Consolas"]);
}

#[test]
fn writes_and_reads_a_docx_font_table_with_embedded_faces() {
    let tree = tree_with_embedded_calibri();
    let docx = write_docx(&tree).unwrap();
    let table = zip_member(&docx, "word/fontTable.xml");
    assert!(table.contains(r#"<w:font w:name="Garamond"><w:family w:val="roman"/>"#));
    assert!(table.contains(r#"<w:panose1 w:val="020F0502020204030204"/>"#));
    assert!(table.contains("w:embedRegular"));
    assert!(table.contains("w:embedBold"));
    assert!(zip_member(&docx, "word/settings.xml").contains("<w:embedTrueTypeFonts/>"));
    let styles = zip_member(&docx, "word/styles.xml");
    assert!(styles.contains(r#"<w:rFonts w:ascii="Calibri""#));
    assert!(styles.contains(r#"<w:sz w:val="24"/>"#));
    assert!(zip_member(&docx, "[Content_Types].xml").contains("obfuscatedFont"));
    assert!(zip_member(&docx, "word/_rels/document.xml.rels").contains("fontTable.xml"));

    let read = read_document_tree(&docx, &Limits::default()).unwrap();
    let calibri = read.fonts.family("calibri").unwrap();
    assert_eq!(calibri.faces.len(), 2);
    assert_eq!(read.fonts.files[calibri.faces[0].file], REGULAR);
    assert_eq!(read.fonts.files[calibri.faces[1].file], BOLD);
    assert_eq!(calibri.panose.as_deref(), Some("020F0502020204030204"));
    assert_eq!(
        read.fonts.family("Garamond").unwrap().generic,
        FontGeneric::Roman
    );
    assert_eq!(read.fonts.family("Consolas"), None);
    assert_eq!(read.report.fonts_left_out, 0);
}

#[test]
fn writes_no_embedding_when_the_tree_carries_no_face() {
    let tree = DocumentTree {
        blocks: vec![text_in("Consolas", "x")],
        ..DocumentTree::default()
    };
    let docx = write_docx(&tree).unwrap();
    let table = zip_member(&docx, "word/fontTable.xml");
    assert!(table.contains(
        r#"<w:font w:name="Consolas"><w:family w:val="modern"/><w:pitch w:val="fixed"/>"#
    ));
    assert!(!zip_member(&docx, "word/settings.xml").contains("embedTrueTypeFonts"));
    assert!(zip_member(&docx, "word/styles.xml").contains(r#"w:ascii="Calibri""#));
}

fn font_table_docx(font_table: &str, fonts: &[(&str, &[u8])], theme: Option<&str>) -> Vec<u8> {
    let mut relationships = vec![("rId1", "fontTable", "fontTable.xml", false)];
    if theme.is_some() {
        relationships.push(("rId2", "theme", "theme/theme1.xml", false));
    }
    let document_rels = rels_xml(&relationships);
    let font_rels = rels_xml(
        &fonts
            .iter()
            .enumerate()
            .map(|(index, (path, _))| {
                (
                    ["rId1", "rId2", "rId3"][index],
                    "font",
                    path.trim_start_matches("word/"),
                    false,
                )
            })
            .collect::<Vec<_>>(),
    );
    let mut extra: Vec<(&str, &[u8])> = vec![
        ("word/_rels/document.xml.rels", document_rels.as_bytes()),
        ("word/fontTable.xml", font_table.as_bytes()),
        ("word/_rels/fontTable.xml.rels", font_rels.as_bytes()),
    ];
    extra.extend_from_slice(fonts);
    if let Some(theme) = theme {
        extra.push(("word/theme/theme1.xml", theme.as_bytes()));
    }
    build_docx_from_body(
        r#"<w:p><w:r><w:rPr><w:rFonts w:asciiTheme="minorHAnsi"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
        &extra,
    )
}

#[test]
fn reports_an_embedded_face_it_cannot_read_and_keeps_the_family() {
    let key = font_key(GUID).unwrap();
    let mut obfuscated = REGULAR.to_vec();
    xor_font(&mut obfuscated, &key);
    let table = format!(
        concat!(
            r#"<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
            r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
            r#"<w:font w:name="Aptos"><w:altName w:val="Aptos Body"/><w:family w:val="swiss"/><w:pitch w:val="variable"/>"#,
            r#"<w:embedRegular r:id="rId1" w:fontKey="{guid}"/><w:embedBold r:id="rId2" w:fontKey="{guid}"/>"#,
            r#"<w:embedItalic r:id="rId9"/></w:font><w:font/></w:fonts>"#
        ),
        guid = GUID
    );
    let docx = font_table_docx(
        &table,
        &[
            ("word/fonts/font1.odttf", &obfuscated),
            ("word/fonts/font2.odttf", b"short"),
        ],
        None,
    );
    let read = read_document_tree(&docx, &Limits::default()).unwrap();
    let aptos = read.fonts.family("Aptos").unwrap();
    assert_eq!(aptos.alt_name.as_deref(), Some("Aptos Body"));
    assert_eq!(aptos.generic, FontGeneric::Swiss);
    assert_eq!(aptos.pitch, FontPitch::Variable);
    assert_eq!(aptos.faces.len(), 1);
    assert_eq!(read.fonts.files[0], REGULAR);
    assert_eq!(read.report.fonts_left_out, 2);
    assert_eq!(read.fonts.families.len(), 1);
}

#[test]
fn resolves_theme_fonts_from_the_document_theme() {
    let theme = concat!(
        r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements>"#,
        r#"<a:fontScheme name="F"><a:majorFont><a:latin typeface="Aptos Display"/></a:majorFont>"#,
        r#"<a:minorFont><a:latin typeface="Aptos"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#
    );
    let table =
        r#"<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#;
    let read = read_document_tree(
        &font_table_docx(table, &[], Some(theme)),
        &Limits::default(),
    )
    .unwrap();
    let Block::Paragraph { content, .. } = &read.blocks[0] else {
        panic!("a paragraph");
    };
    let Inline::Text { marks, .. } = &content[0] else {
        panic!("a run");
    };
    assert!(marks.iter().any(|mark| matches!(
        mark,
        Mark::TextStyle { font_family: Some(family), .. } if family == "Aptos"
    )));
    let without =
        read_document_tree(&font_table_docx(table, &[], None), &Limits::default()).unwrap();
    let Block::Paragraph { content, .. } = &without.blocks[0] else {
        panic!("a paragraph");
    };
    let Inline::Text { marks, .. } = &content[0] else {
        panic!("a run");
    };
    assert!(marks.iter().any(|mark| matches!(
        mark,
        Mark::TextStyle { font_family: Some(family), .. } if family == "Calibri"
    )));
}

#[test]
fn keeps_no_more_font_files_than_the_limits() {
    let mut fonts = DocumentFonts::default();
    let mut report = ImportReport::default();
    for _ in 0..MAX_FONT_FILES {
        assert!(fonts.keep_file(b"OTTO".to_vec(), &mut report).is_some());
    }
    assert_eq!(fonts.keep_file(b"OTTO".to_vec(), &mut report), None);
    assert_eq!(report.fonts_left_out, 1);
    let mut other = ImportReport::default();
    assert_eq!(
        DocumentFonts::default().keep_file(b"nope".to_vec(), &mut other),
        None
    );
    assert_eq!(other.fonts_left_out, 1);
    assert!(fonts.has_files());
}

#[test]
fn writes_and_reads_odf_font_faces_with_embedded_files() {
    let tree = tree_with_embedded_calibri();
    let odt = write_odt(&tree).unwrap();
    let styles = zip_member(&odt, "styles.xml");
    assert!(styles.contains(r#"<style:font-face style:name="Garamond" svg:font-family="'Garamond'" style:font-family-generic="roman" style:font-pitch="variable"/>"#));
    assert!(styles.contains(r#"xlink:href="Fonts/font1-regular.ttf""#));
    assert!(zip_member(&odt, "META-INF/manifest.xml").contains("Fonts/font2-bold.ttf"));
    assert!(zip_member(&odt, "settings.xml").contains("EmbedFonts"));

    let read = read_document_tree(&odt, &Limits::default()).unwrap();
    let calibri = read.fonts.family("Calibri").unwrap();
    assert_eq!(calibri.generic, FontGeneric::Swiss);
    assert_eq!(calibri.faces.len(), 2);
    assert_eq!(calibri.faces[1].style, FaceStyle::Bold);
    assert_eq!(read.fonts.files[calibri.faces[1].file], BOLD);
    assert_eq!(
        read.fonts.family("Garamond").unwrap().pitch,
        FontPitch::Variable
    );
}

#[test]
fn reports_an_odf_font_file_that_is_missing() {
    let content = concat!(
        r#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
        r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
        r#"xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">"#,
        r#"<office:font-face-decls><style:font-face style:name="F1" svg:font-family="'Gone'">"#,
        r#"<svg:font-face-src><svg:font-face-uri xlink:href="Fonts/gone.ttf"/><svg:font-face-uri xlink:href="https://example.com/a.ttf"/></svg:font-face-src>"#,
        r#"</style:font-face><style:font-face style:name="Fallback"/></office:font-face-decls>"#,
        r#"<office:body><office:text><text:p>x</text:p></office:text></office:body></office:document-content>"#
    );
    let odt = build_zip(&[
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        ("content.xml", content.as_bytes()),
    ]);
    let read = read_document_tree(&odt, &Limits::default()).unwrap();
    assert_eq!(read.report.fonts_left_out, 1);
    assert_eq!(read.fonts.family("Gone").unwrap().faces, Vec::new());
    assert!(read.fonts.family("Fallback").is_some());
}

#[test]
fn writes_small_embedded_faces_into_an_html_page() {
    let mut tree = tree_with_embedded_calibri();
    tree.fonts.files = vec![b"   regular".to_vec(), b"   bold".to_vec()];
    let html = write_html(&tree, "t");
    assert_eq!(html.matches("@font-face").count(), 2);
    assert!(html.contains("font-family:'Calibri', 'Carlito', sans-serif;font-size:12pt;"));
    assert!(html.contains("font-family:'Garamond', 'Liberation Serif', serif;"));
    let mut large = tree;
    large.fonts.files[0] = vec![0; crate::document::html::MAX_HTML_FONT_FACE_BYTES + 1];
    assert_eq!(write_html(&large, "t").matches("@font-face").count(), 1);
}

#[test]
fn the_react_table_of_substitutes_matches_this_one() {
    let source = include_str!("../../../../../packages/rely-office-react/src/document/fonts.ts");
    for (name, substitute, generic) in SUBSTITUTES {
        let line = format!("['{name}', '{substitute}', '{}']", generic.name());
        assert!(source.contains(&line), "fonts.ts lacks {line}");
    }
    let rows = source
        .lines()
        .filter(|line| line.trim_start().starts_with("['"))
        .count();
    assert_eq!(rows, SUBSTITUTES.len());
}
