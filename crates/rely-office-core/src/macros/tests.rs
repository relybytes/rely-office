// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::test_support::{
    build_compound_file, build_docx, build_pptx, build_zip, ppt_record, zip_member,
};
use crate::{read_document_tree, read_presentation, write_docx, write_pptx};

const PROJECT: &[u8] = b"\xCC\x61VBA project bytes";
const VBA_DATA: &str = r#"<?xml version="1.0"?><wne:vbaSuppData xmlns:wne="http://schemas.microsoft.com/office/word/2006/wordml"/>"#;

/// A docm: the document of `build_docx` with a VBA project and the data Word keeps beside it.
fn build_docm() -> Vec<u8> {
    let plain = build_docx(&["Ciao"]);
    let project = MacroProject {
        project_part: "word/vbaProject.bin".to_owned(),
        parts: vec![
            CarriedPart {
                name: "word/vbaProject.bin".to_owned(),
                content_type: "application/vnd.ms-office.vbaProject".to_owned(),
                bytes: PROJECT.to_vec(),
            },
            CarriedPart {
                name: "word/_rels/vbaProject.bin.rels".to_owned(),
                content_type: RELATIONSHIPS_TYPE.to_owned(),
                bytes: br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.microsoft.com/office/2006/relationships/wordVbaData" Target="vbaData.xml"/></Relationships>"#.to_vec(),
            },
            CarriedPart {
                name: "word/vbaData.xml".to_owned(),
                content_type: "application/vnd.ms-word.vbaData+xml".to_owned(),
                bytes: VBA_DATA.as_bytes().to_vec(),
            },
        ],
    };
    insert_macro_project(&plain, &project).unwrap()
}

#[test]
fn a_docm_is_read_with_its_project_and_written_back_as_a_docm() {
    let docm = build_docm();
    assert_eq!(detect_format(&docm, None), DocumentFormat::Docx);
    assert!(holds_macros(&docm, &Limits::default()));
    let tree = read_document_tree(&docm, &Limits::default()).unwrap();
    assert!(tree.report.macros);
    let project = tree.macro_project.clone().unwrap();
    assert_eq!(project.project_part, "word/vbaProject.bin");
    assert_eq!(project.parts.len(), 3);

    let written = write_docx(&tree).unwrap();
    let types = zip_member(&written, "[Content_Types].xml");
    assert!(types.contains("application/vnd.ms-word.document.macroEnabled.main+xml"));
    assert!(!types.contains("wordprocessingml.document.main+xml"));
    assert!(types.contains(r#"PartName="/word/vbaProject.bin""#));
    assert!(types.contains(r#"PartName="/word/vbaData.xml""#));
    let relationships = zip_member(&written, "word/_rels/document.xml.rels");
    assert!(relationships.contains(r#"Target="vbaProject.bin""#));
    assert_eq!(zip_member(&written, "word/vbaData.xml"), VBA_DATA);
    let again = read_document_tree(&written, &Limits::default()).unwrap();
    assert_eq!(again.macro_project, Some(project));
}

#[test]
fn a_document_without_macros_stays_a_docx() {
    let tree = read_document_tree(&build_docx(&["Ciao"]), &Limits::default()).unwrap();
    assert!(!tree.report.macros);
    assert!(tree.macro_project.is_none());
    let written = write_docx(&tree).unwrap();
    assert!(!zip_member(&written, "[Content_Types].xml").contains("macroEnabled"));
    assert!(!holds_macros(&written, &Limits::default()));
}

#[test]
fn a_project_survives_its_opaque_value() {
    let project = read_macro_project(&build_docm(), &Limits::default()).unwrap();
    let bytes = project.to_bytes().unwrap();
    assert_eq!(
        MacroProject::from_bytes(&bytes, &Limits::default()).unwrap(),
        project
    );
    assert!(MacroProject::from_bytes(b"not a zip", &Limits::default()).is_err());
    let no_project = build_zip(&[("[Content_Types].xml", b"<Types/>")]);
    assert!(MacroProject::from_bytes(&no_project, &Limits::default()).is_err());
}

#[test]
fn a_pptm_keeps_its_project_through_the_slides() {
    let plain = build_pptx(&[&["Titolo"]], &[]);
    let project = MacroProject {
        project_part: "ppt/vbaProject.bin".to_owned(),
        parts: vec![CarriedPart {
            name: "ppt/vbaProject.bin".to_owned(),
            content_type: "application/vnd.ms-office.vbaProject".to_owned(),
            bytes: PROJECT.to_vec(),
        }],
    };
    let pptm = insert_macro_project(&plain, &project).unwrap();
    let presentation = read_presentation(&pptm, &Limits::default()).unwrap();
    assert!(presentation.holds_macros);
    assert_eq!(presentation.macro_project, Some(project.clone()));
    let written = write_pptx(&presentation).unwrap();
    assert!(
        zip_member(&written, "[Content_Types].xml")
            .contains("application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml")
    );
    assert_eq!(
        read_macro_project(&written, &Limits::default()),
        Some(project)
    );
}

#[test]
fn a_project_in_another_folder_is_named_from_the_root() {
    let plain = build_docx(&["Ciao"]);
    let project = MacroProject {
        project_part: "macros/vbaProject.bin".to_owned(),
        parts: vec![CarriedPart {
            name: "macros/vbaProject.bin".to_owned(),
            content_type: "application/vnd.ms-office.vbaProject".to_owned(),
            bytes: PROJECT.to_vec(),
        }],
    };
    let written = insert_macro_project(&plain, &project).unwrap();
    assert!(
        zip_member(&written, "word/_rels/document.xml.rels")
            .contains(r#"Target="/macros/vbaProject.bin""#)
    );
    assert_eq!(
        read_macro_project(&written, &Limits::default()),
        Some(project)
    );
}

#[test]
fn the_binary_formats_tell_their_macros() {
    let limits = Limits::default();
    let doc = build_compound_file(&[("/WordDocument", &[0; 16]), ("/Macros/VBA/dir", b"x")]);
    assert!(holds_macros(&doc, &limits));
    let xls = build_compound_file(&[("/Workbook", &[0; 16]), ("/_VBA_PROJECT_CUR/VBA/dir", b"x")]);
    assert!(holds_macros(&xls, &limits));
    assert!(!holds_macros(
        &build_compound_file(&[("/Workbook", &[0; 16])]),
        &limits
    ));
    let with_vba = ppt_record(0xF, 0, PPT_DOCUMENT, &ppt_record(0xF, 0, PPT_VBA_INFO, &[]));
    let ppt = build_compound_file(&[("/PowerPoint Document", &with_vba)]);
    assert!(holds_macros(&ppt, &limits));
    let without = ppt_record(0xF, 0, PPT_DOCUMENT, &[]);
    assert!(!holds_macros(
        &build_compound_file(&[("/PowerPoint Document", &without)]),
        &limits
    ));
    assert!(!holds_macros(b"plain text", &limits));
}

#[test]
fn a_package_without_a_main_part_is_refused() {
    let package = build_zip(&[("[Content_Types].xml", b"<Types/>")]);
    assert!(insert_macro_project(&package, &MacroProject::default()).is_err());
}
