// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! `rely-office <command> <file>`: the library from a terminal, for trying documents by hand.

use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use rely_office_core::{
    Limits, convert_metafile_to_svg, detect_format, extract_text, read_document_tree,
    read_presentation, read_workbook, write_docx, write_pptx,
};
use rely_office_render::{PageImages, RenderRequest, Renderer, render_limits};
use rely_office_sheet::{Locale, read_spreadsheet, write_ods, write_xlsx};

const USAGE: &str =
    "usage: rely-office <detect|text|sheets|tree|rewrite|ods|svg|typst|pdf|png> <file>

  detect   print the format read from the bytes
  text     print the text a search index would store
  sheets   print every sheet as tab-separated rows
  tree     print the document tree of a docx, its images and its import report
  rewrite  write a document back from its document tree as <file>.rewritten.docx, a
           presentation from its slides as <file>.rewritten.pptx, a spreadsheet from its
           editable model as <file>.rewritten.xlsx
  ods      write a spreadsheet from its editable model as <file>.rewritten.ods
  svg      write an EMF or WMF picture as <file>.svg
  typst    print the Typst source a render typesets
  pdf      write <file>.pdf next to the file
  png      write <file>-<page>.png next to the file, at 144 dpi

The renders use the fonts of the folder RELY_OFFICE_FONTS names, by default ./fonts.";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [command, path] = arguments.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    match run(command, Path::new(path)) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("rely-office: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: &str, path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let file_name = path.file_name().and_then(|name| name.to_str());
    let limits = Limits::default();

    match command {
        "detect" => Ok(format!("{:?}\n", detect_format(&bytes, file_name))),
        "text" => extract_text(&bytes, file_name, &limits)
            .map(|extracted| extracted.text)
            .map_err(|error| error.to_string()),
        "sheets" => {
            let workbook =
                read_workbook(&bytes, file_name, &limits).map_err(|error| error.to_string())?;
            let mut output = String::new();
            for sheet in workbook.sheets {
                let _ = writeln!(
                    output,
                    "# {} ({} x {})",
                    sheet.name, sheet.total_rows, sheet.total_columns
                );
                for row in sheet.rows {
                    output.push_str(&row.join("\t"));
                    output.push('\n');
                }
            }
            Ok(output)
        }
        "tree" => {
            let tree = read_document_tree(&bytes, &limits).map_err(|error| error.to_string())?;
            let mut output = format!("{:#?}\n", tree.blocks);
            for (index, media) in tree.media.iter().enumerate() {
                let _ = writeln!(
                    output,
                    "media {index}: {} ({} bytes)",
                    media.content_type,
                    media.bytes.len()
                );
            }
            let _ = writeln!(output, "{:#?}", tree.report);
            Ok(output)
        }
        "typst" | "pdf" | "png" => render(command, path, &bytes, file_name),
        "rewrite"
            if matches!(
                detect_format(&bytes, file_name).as_str(),
                "xlsx" | "xls" | "ods" | "csv" | "tsv"
            ) =>
        {
            rewrite_workbook(path, &bytes, file_name, &limits, false)
        }
        "rewrite"
            if matches!(
                detect_format(&bytes, file_name).as_str(),
                "pptx" | "odp" | "ppt"
            ) =>
        {
            let presentation =
                read_presentation(&bytes, &limits).map_err(|error| error.to_string())?;
            let written = write_pptx(&presentation).map_err(|error| error.to_string())?;
            let target = path.with_file_name(format!(
                "{}.rewritten.pptx",
                file_name.unwrap_or("presentation")
            ));
            std::fs::write(&target, written)
                .map_err(|error| format!("{}: {error}", target.display()))?;
            Ok(format!("{}\n", target.display()))
        }
        "ods" => rewrite_workbook(path, &bytes, file_name, &limits, true),
        "svg" => {
            let svg = convert_metafile_to_svg(&bytes)
                .ok_or_else(|| "not an EMF or WMF picture, or one that draws nothing".to_owned())?;
            let target = path.with_file_name(format!("{}.svg", file_name.unwrap_or("picture")));
            std::fs::write(&target, svg)
                .map_err(|error| format!("{}: {error}", target.display()))?;
            Ok(format!(
                "{}
",
                target.display()
            ))
        }
        "rewrite" => {
            let tree = read_document_tree(&bytes, &limits).map_err(|error| error.to_string())?;
            let written = write_docx(&tree).map_err(|error| error.to_string())?;
            let target = path.with_file_name(format!(
                "{}.rewritten.docx",
                file_name.unwrap_or("document")
            ));
            std::fs::write(&target, written)
                .map_err(|error| format!("{}: {error}", target.display()))?;
            Ok(format!("{}\n", target.display()))
        }
        other => Err(format!("unknown command {other}\n{USAGE}")),
    }
}

/// Write a spreadsheet back from its editable model, as ods or as xlsx, next to the file.
fn rewrite_workbook(
    path: &Path,
    bytes: &[u8],
    file_name: Option<&str>,
    limits: &Limits,
    as_ods: bool,
) -> Result<String, String> {
    let workbook = read_spreadsheet(bytes, file_name, limits, Locale::Italian)
        .map_err(|error| error.to_string())?;
    let (written, extension) = if as_ods {
        (write_ods(&workbook), "ods")
    } else {
        (write_xlsx(&workbook), "xlsx")
    };
    let written = written.map_err(|error| error.to_string())?;
    let target = path.with_file_name(format!(
        "{}.rewritten.{extension}",
        file_name.unwrap_or("workbook")
    ));
    std::fs::write(&target, written).map_err(|error| format!("{}: {error}", target.display()))?;
    Ok(format!("{}\n", target.display()))
}

/// The renderer with every font file of the fonts folder.
fn renderer() -> Result<Renderer, String> {
    let folder = std::env::var_os("RELY_OFFICE_FONTS").unwrap_or_else(|| "fonts".into());
    let folder = Path::new(&folder);
    let entries =
        std::fs::read_dir(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let mut fonts = Vec::new();
    for path in entries.flatten().map(|entry| entry.path()) {
        if path
            .extension()
            .is_some_and(|extension| extension == "ttf" || extension == "otf" || extension == "ttc")
        {
            fonts.push(
                std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?,
            );
        }
    }
    Ok(Renderer::new(fonts))
}

fn render(
    command: &str,
    path: &Path,
    bytes: &[u8],
    file_name: Option<&str>,
) -> Result<String, String> {
    let typesetter = renderer()?;
    if command == "typst" {
        return typesetter
            .typst_source(bytes, file_name, &render_limits())
            .map(|source| source.source)
            .map_err(|error| error.to_string());
    }
    let request = RenderRequest {
        pdf: command == "pdf",
        page_images: (command == "png").then_some(PageImages {
            pixels_per_point: 2.0,
            max_pages: usize::MAX,
        }),
        ..RenderRequest::default()
    };
    let rendered = typesetter
        .render(bytes, file_name, &request)
        .map_err(|error| error.to_string())?;
    let name = file_name.unwrap_or("document");
    let mut written = Vec::new();
    if let Some(pdf) = rendered.pdf {
        written.push((path.with_file_name(format!("{name}.pdf")), pdf));
    }
    for (index, page) in rendered.page_images.into_iter().enumerate() {
        written.push((
            path.with_file_name(format!("{name}-{}.png", index + 1)),
            page,
        ));
    }
    let mut output = String::new();
    for (target, content) in written {
        std::fs::write(&target, content)
            .map_err(|error| format!("{}: {error}", target.display()))?;
        let _ = writeln!(output, "{}", target.display());
    }
    let _ = writeln!(
        output,
        "{} pages, {:?}{}",
        rendered.page_count,
        rendered.fidelity,
        if rendered.truncated {
            ", cut to the limits"
        } else {
            ""
        }
    );
    Ok(output)
}
