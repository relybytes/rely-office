// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::test_support::{build_opendocument, build_zip, rels_xml};

const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

fn xlsx(workbook_body: &str, comments: Option<&str>) -> Vec<u8> {
    let workbook = format!(
        r#"<workbook xmlns="{MAIN}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Foglio1" sheetId="1" r:id="rId1"/></sheets>{workbook_body}</workbook>"#
    );
    let workbook_rels = rels_xml(&[(
        "rId1",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet",
        "worksheets/sheet1.xml",
        false,
    )]);
    let sheet = format!(r#"<worksheet xmlns="{MAIN}"><sheetData/></worksheet>"#);
    let sheet_rels = rels_xml(&[(
        "rId1",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments",
        "../comments1.xml",
        false,
    )]);
    let mut members: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#),
        ("xl/workbook.xml", workbook.as_bytes()),
        ("xl/_rels/workbook.xml.rels", workbook_rels.as_bytes()),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
    ];
    if let Some(comments) = comments {
        members.push(("xl/worksheets/_rels/sheet1.xml.rels", sheet_rels.as_bytes()));
        members.push(("xl/comments1.xml", comments.as_bytes()));
    }
    build_zip(&members)
}

#[test]
fn reads_the_names_of_an_xlsx_without_excels_own() {
    let bytes = xlsx(
        concat!(
            r#"<definedNames>"#,
            r#"<definedName name="Aliquota">Foglio1!$B$1</definedName>"#,
            r#"<definedName name="Locale" localSheetId="0" hidden="1">0.22&amp;"x"</definedName>"#,
            r#"<definedName name="_xlnm._FilterDatabase" localSheetId="0" hidden="1">Foglio1!$A$1:$B$4</definedName>"#,
            r#"</definedNames>"#
        ),
        None,
    );
    let names = read_workbook_names(&bytes, &Limits::default());
    assert_eq!(
        names,
        vec![
            WorkbookName {
                name: "Aliquota".to_owned(),
                sheet: None,
                formula: "Foglio1!$B$1".to_owned(),
                hidden: false,
            },
            WorkbookName {
                name: "Locale".to_owned(),
                sheet: Some(0),
                formula: "0.22&\"x\"".to_owned(),
                hidden: true,
            },
        ]
    );
}

#[test]
fn a_file_that_is_no_workbook_has_no_names() {
    assert_eq!(
        read_workbook_names(b"not a zip", &Limits::default()),
        Vec::new()
    );
    let bytes = xlsx("", None);
    assert_eq!(read_workbook_names(&bytes, &Limits::default()), Vec::new());
}

#[test]
fn reads_the_comments_of_an_xlsx_sheet_without_the_authors_heading() {
    let comments = format!(
        concat!(
            r#"<comments xmlns="{}"><authors><author>Anna</author><author>Bruno</author></authors><commentList>"#,
            r#"<comment ref="B2" authorId="0"><text><r><rPr><b/></rPr><t>Anna:</t></r><r><t xml:space="preserve">"#,
            "\n",
            r#"Controllare</t></r></text></comment>"#,
            r#"<comment ref="C3" authorId="1"><text><t>Visto</t><rPh><t>みた</t></rPh></text></comment>"#,
            r#"<comment ref="nowhere" authorId="1"><text><t>Lost</t></text></comment>"#,
            r#"<comment ref="D4" authorId="9"><text><t>Anonimo</t></text></comment>"#,
            r#"</commentList></comments>"#
        ),
        MAIN
    );
    let bytes = xlsx("", Some(&comments));
    let layouts = crate::read_xlsx_layouts(&bytes, &Limits::default());
    let found = &layouts["Foglio1"].comments;
    assert_eq!(
        found,
        &vec![
            CellComment {
                row: 1,
                column: 1,
                author: "Anna".to_owned(),
                text: "Controllare".to_owned(),
                ..CellComment::default()
            },
            CellComment {
                row: 2,
                column: 2,
                author: "Bruno".to_owned(),
                text: "Visto".to_owned(),
                ..CellComment::default()
            },
            CellComment {
                row: 3,
                column: 3,
                author: String::new(),
                text: "Anonimo".to_owned(),
                ..CellComment::default()
            },
        ]
    );
}

#[test]
fn a_sheet_without_comments_has_none() {
    let bytes = xlsx("", None);
    let layouts = crate::read_xlsx_layouts(&bytes, &Limits::default());
    assert_eq!(layouts["Foglio1"].comments, Vec::new());
}

const ODS_ANNOTATION: &str = r#"<office:spreadsheet><table:named-expressions><table:named-range table:name="Vendite" table:base-cell-address="$Foglio1.$A$1" table:cell-range-address="$Foglio1.$A$1:.$A$3"/><table:named-expression table:name="Doppio" table:base-cell-address="$Foglio1.$A$1" table:expression="of:=[.$A$1]*2"/></table:named-expressions><table:table table:name="Foglio1"><table:table-row><table:table-cell office:value-type="float" office:value="1"><office:annotation xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>Carla</dc:creator><dc:date>2026-10-29T10:00:00</dc:date><text:p>Prima<text:line-break/>riga</text:p><text:p>Seconda<text:s text:c="2"/>fine</text:p></office:annotation><text:p>1</text:p></table:table-cell><table:table-cell table:number-columns-repeated="2"/><table:table-cell><text:p>x</text:p></table:table-cell></table:table-row></table:table><table:table table:name="Foglio2"><table:named-expressions><table:named-expression table:name="Locale" table:expression="of:=5"/><table:named-range table:name="Vuoto"/></table:named-expressions></table:table></office:spreadsheet>"#;

#[test]
fn reads_the_names_of_an_ods_in_excels_notation() {
    let bytes = build_opendocument(
        "application/vnd.oasis.opendocument.spreadsheet",
        ODS_ANNOTATION,
        "",
    );
    let names = read_workbook_names(&bytes, &Limits::default());
    let found: Vec<(&str, Option<usize>, &str)> = names
        .iter()
        .map(|name| (name.name.as_str(), name.sheet, name.formula.as_str()))
        .collect();
    assert_eq!(
        found,
        vec![
            ("Vendite", None, "Foglio1!$A$1:$A$3"),
            ("Doppio", None, "$A$1*2"),
            ("Locale", Some(1), "5"),
        ]
    );
}

#[test]
fn reads_the_comment_of_an_ods_cell() {
    let bytes = build_opendocument(
        "application/vnd.oasis.opendocument.spreadsheet",
        ODS_ANNOTATION,
        "",
    );
    let layouts = crate::read_ods_sheet_layouts(&bytes, &Limits::default());
    assert_eq!(
        layouts["Foglio1"].comments,
        vec![CellComment {
            row: 0,
            column: 0,
            author: "Carla".to_owned(),
            text: "Prima\nriga\nSeconda  fine".to_owned(),
            date: Some("2026-10-29T10:00:00".to_owned()),
            ..CellComment::default()
        }]
    );
}

#[test]
fn a_long_comment_is_cut() {
    assert_eq!(
        truncate(&"a".repeat(MAX_COMMENT_CHARS + 10)).len(),
        MAX_COMMENT_CHARS
    );
}

#[test]
fn malformed_xml_is_an_error() {
    assert!(xlsx_names("<workbook><definedName name=\"x\">").is_ok());
    assert!(xlsx_names("<workbook></wrong>").is_err());
    assert!(xlsx_comments("<comments>").is_err());
}

#[test]
fn reads_a_thread_with_its_replies_and_drops_the_note_beside_it() {
    let workbook = format!(
        r#"<workbook xmlns="{MAIN}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Foglio1" sheetId="1" r:id="rId1"/></sheets></workbook>"#
    );
    let workbook_rels = rels_xml(&[
        (
            "rId1",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet",
            "worksheets/sheet1.xml",
            false,
        ),
        (
            "rId2",
            "http://schemas.microsoft.com/office/2017/10/relationships/person",
            "persons/person.xml",
            false,
        ),
    ]);
    let sheet = format!(r#"<worksheet xmlns="{MAIN}"><sheetData/></worksheet>"#);
    let sheet_rels = rels_xml(&[
        (
            "rId1",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments",
            "../comments1.xml",
            false,
        ),
        (
            "rId2",
            "http://schemas.microsoft.com/office/2017/10/relationships/threadedComment",
            "../threadedComments/threadedComment1.xml",
            false,
        ),
    ]);
    let persons = r#"<personList xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments"><person displayName="Anna" id="{P1}" userId="Anna" providerId="None"/><person displayName="Bruno" id="{P2}" userId="Bruno" providerId="None"/></personList>"#;
    let threads = r#"<ThreadedComments xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments"><threadedComment ref="B2" dT="2026-10-30T09:00:00.00" personId="{P1}" id="{T1}" done="1"><text>Prezzo giusto?</text></threadedComment><threadedComment ref="B2" dT="2026-10-30T09:05:00.00" personId="{P2}" id="{T2}" parentId="{T1}"><text>Si</text></threadedComment><threadedComment ref="B2" personId="{P9}" id="{T3}" parentId="{T9}"><text>Orfana</text></threadedComment></ThreadedComments>"#;
    let comments = format!(
        r#"<comments xmlns="{MAIN}"><authors><author>tc={{T1}}</author><author>Carla</author></authors><commentList><comment ref="B2" authorId="0"><text><t>[Threaded comment]</t></text></comment><comment ref="C3" authorId="1"><text><t>Nota</t></text></comment></commentList></comments>"#
    );
    let bytes = build_zip(&[
        (
            "[Content_Types].xml",
            br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#,
        ),
        ("xl/workbook.xml", workbook.as_bytes()),
        ("xl/_rels/workbook.xml.rels", workbook_rels.as_bytes()),
        ("xl/persons/person.xml", persons.as_bytes()),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
        ("xl/worksheets/_rels/sheet1.xml.rels", sheet_rels.as_bytes()),
        (
            "xl/threadedComments/threadedComment1.xml",
            threads.as_bytes(),
        ),
        ("xl/comments1.xml", comments.as_bytes()),
    ]);
    let layouts = crate::read_xlsx_layouts(&bytes, &Limits::default());
    assert_eq!(
        layouts["Foglio1"].comments,
        vec![
            CellComment {
                row: 1,
                column: 1,
                author: "Anna".to_owned(),
                text: "Prezzo giusto?".to_owned(),
                date: Some("2026-10-30T09:00:00.00".to_owned()),
                threaded: true,
                resolved: true,
                replies: vec![CommentReply {
                    author: "Bruno".to_owned(),
                    text: "Si".to_owned(),
                    date: Some("2026-10-30T09:05:00.00".to_owned()),
                }],
            },
            CellComment {
                row: 2,
                column: 2,
                author: "Carla".to_owned(),
                text: "Nota".to_owned(),
                ..CellComment::default()
            },
        ]
    );
}
