// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! `rely-office-corpus`: render every document of a folder and compare each page with the page
//! `LibreOffice` printed for it, as `scripts/corpus.mjs` prepares them.

mod compare;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rely_office_render::{PageImages, RenderRequest, Renderer};
use serde_json::json;

use compare::{GreyPage, page_distance};

const USAGE: &str = "usage: rely-office-corpus --documents <dir> --references <dir> --fonts <dir>
                          [--threshold <distance>] [--min-pass-rate <share>] [--report <file>]
                          [--pages <dir>]

  --documents      the documents to render
  --references     LibreOffice's pages as <document stem>-<page number>.png, at 72 dpi
  --fonts          the font files the renderer uses
  --threshold      the largest distance of a passing page, default 0.03
  --min-pass-rate  fail when fewer pages pass, from 0 to 1, default 0
  --report         write the distances of every page as JSON
  --pages          write the pages rendered, as <document stem>-<page number>.png";
const DEFAULT_THRESHOLD: f64 = 0.03;
const MAX_PAGES: usize = 200;

struct Options {
    documents: PathBuf,
    references: PathBuf,
    fonts: PathBuf,
    threshold: f64,
    min_pass_rate: f64,
    report: Option<PathBuf>,
    pages: Option<PathBuf>,
}

fn main() -> ExitCode {
    let options = match parse_options(std::env::args().skip(1).collect()) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("rely-office-corpus: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&options) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(message) => {
            eprintln!("rely-office-corpus: {message}");
            ExitCode::from(2)
        }
    }
}

fn parse_options(arguments: Vec<String>) -> Result<Options, String> {
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    let mut arguments = arguments.into_iter();
    while let Some(name) = arguments.next() {
        let Some(name) = name.strip_prefix("--") else {
            return Err(format!("unexpected argument {name}"));
        };
        let value = arguments
            .next()
            .ok_or_else(|| format!("--{name} needs a value"))?;
        values.insert(name.to_owned(), value);
    }
    let mut take = |name: &str| values.remove(name);
    let required = |value: Option<String>, name: &str| {
        value
            .map(PathBuf::from)
            .ok_or_else(|| format!("--{name} is required"))
    };
    let number = |value: Option<String>, name: &str, default: f64| {
        value.map_or(Ok(default), |value| {
            value
                .parse::<f64>()
                .ok()
                .filter(|number| (0.0..=1.0).contains(number))
                .ok_or_else(|| format!("--{name} must be a number from 0 to 1"))
        })
    };
    let options = Options {
        documents: required(take("documents"), "documents")?,
        references: required(take("references"), "references")?,
        fonts: required(take("fonts"), "fonts")?,
        threshold: number(take("threshold"), "threshold", DEFAULT_THRESHOLD)?,
        min_pass_rate: number(take("min-pass-rate"), "min-pass-rate", 0.0)?,
        report: take("report").map(PathBuf::from),
        pages: take("pages").map(PathBuf::from),
    };
    if let Some(unknown) = values.keys().next() {
        return Err(format!("unknown option --{unknown}"));
    }
    Ok(options)
}

fn files_in(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    Ok(files)
}

/// `LibreOffice`'s pages of a document, in page order: `<stem>-<n>.png`, with or without zeros
/// before `n` as the rasterizer writes them.
fn reference_pages(references: &[PathBuf], stem: &str) -> Vec<PathBuf> {
    let mut pages: Vec<(u32, PathBuf)> = references
        .iter()
        .filter_map(|path| {
            let name = path.file_stem()?.to_str()?;
            let number = name
                .strip_prefix(stem)?
                .strip_prefix('-')?
                .parse::<u32>()
                .ok()?;
            (path.extension()? == "png").then(|| (number, path.clone()))
        })
        .collect();
    pages.sort();
    pages.into_iter().map(|(_, path)| path).collect()
}

fn run(options: &Options) -> Result<bool, String> {
    let fonts = files_in(&options.fonts)?
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "ttf" || extension == "otf")
        })
        .map(|path| std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display())))
        .collect::<Result<Vec<_>, _>>()?;
    let renderer = Renderer::new(fonts);
    let references = files_in(&options.references)?;
    let request = RenderRequest {
        pdf: false,
        page_images: Some(PageImages {
            pixels_per_point: 1.0,
            max_pages: MAX_PAGES,
        }),
        ..RenderRequest::default()
    };

    let mut table = String::new();
    let mut documents = Vec::new();
    let (mut passed_pages, mut total_pages) = (0usize, 0usize);
    for document in files_in(&options.documents)? {
        let compared = compare_document(&renderer, &request, &references, &document, options)?;
        table.push_str(&compared.line);
        documents.push(compared.report);
        passed_pages += compared.passed;
        total_pages += compared.pages;
    }

    #[allow(clippy::cast_precision_loss)]
    let pass_rate = if total_pages == 0 {
        0.0
    } else {
        passed_pages as f64 / total_pages as f64
    };
    print!("{table}");
    println!(
        "\n{passed_pages} of {total_pages} pages within {} ({:.1}%), minimum {:.1}%",
        options.threshold,
        pass_rate * 100.0,
        options.min_pass_rate * 100.0
    );
    if let Some(report) = &options.report {
        let report_json = json!({
            "threshold": options.threshold,
            "passedPages": passed_pages,
            "totalPages": total_pages,
            "passRate": pass_rate,
            "documents": documents,
        });
        std::fs::write(report, format!("{report_json:#}\n"))
            .map_err(|error| format!("{}: {error}", report.display()))?;
    }
    Ok(total_pages > 0 && pass_rate >= options.min_pass_rate)
}

/// One document compared: its line of the table, its part of the report, and its pages.
struct ComparedDocument {
    line: String,
    report: serde_json::Value,
    passed: usize,
    pages: usize,
}

fn compare_document(
    renderer: &Renderer,
    request: &RenderRequest,
    references: &[PathBuf],
    document: &Path,
    options: &Options,
) -> Result<ComparedDocument, String> {
    let threshold = options.threshold;
    let name = document
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    let stem = document
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_owned();
    let bytes =
        std::fs::read(document).map_err(|error| format!("{}: {error}", document.display()))?;
    let theirs = reference_pages(references, &stem);
    let ours = match renderer.render(&bytes, Some(&name), request) {
        Ok(output) => output.page_images,
        Err(error) => {
            return Ok(ComparedDocument {
                line: format!(
                    "{name:<40} render failed: {error}
"
                ),
                report: json!({ "document": name, "error": error.to_string() }),
                passed: 0,
                pages: theirs.len().max(1),
            });
        }
    };
    if let Some(pages) = &options.pages {
        std::fs::create_dir_all(pages).map_err(|error| format!("{}: {error}", pages.display()))?;
        for (index, page) in ours.iter().enumerate() {
            let path = pages.join(format!("{stem}-{}.png", index + 1));
            std::fs::write(&path, page).map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    let mut distances = Vec::new();
    for (our_page, their_page) in ours.iter().zip(&theirs) {
        let their_page = std::fs::read(their_page)
            .map_err(|error| format!("{}: {error}", their_page.display()))?;
        distances.push(page_distance(
            &GreyPage::from_png(our_page)?,
            &GreyPage::from_png(&their_page)?,
        ));
    }
    let passed = distances
        .iter()
        .filter(|distance| distance.is_some_and(|distance| distance <= threshold))
        .count();
    let worst = distances
        .iter()
        .map(|distance| distance.unwrap_or(1.0))
        .fold(0.0_f64, f64::max);
    Ok(ComparedDocument {
        line: format!(
            "{name:<40} pages {:>3} / {:>3}  passed {passed:>3}  worst {worst:.3}
",
            ours.len(),
            theirs.len()
        ),
        report: json!({
            "document": name,
            "pages": ours.len(),
            "referencePages": theirs.len(),
            "passed": passed,
            "distances": distances,
        }),
        passed,
        pages: ours.len().max(theirs.len()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn reads_the_options() {
        let options = parse_options(arguments(
            "--documents d --references r --fonts f --threshold 0.1 --min-pass-rate 0.5 --report x.json",
        ))
        .unwrap();
        assert_eq!(options.documents, PathBuf::from("d"));
        assert!((options.threshold - 0.1).abs() < f64::EPSILON);
        assert!((options.min_pass_rate - 0.5).abs() < f64::EPSILON);
        assert_eq!(options.report, Some(PathBuf::from("x.json")));

        let defaults = parse_options(arguments("--documents d --references r --fonts f")).unwrap();
        assert!((defaults.threshold - DEFAULT_THRESHOLD).abs() < f64::EPSILON);
        assert!(defaults.report.is_none());
    }

    #[test]
    fn refuses_options_it_cannot_use() {
        for wrong in [
            "--documents d --references r",
            "--documents d --references r --fonts f --threshold 2",
            "--documents d --references r --fonts f --colour red",
            "--documents d --references r --fonts",
            "documents",
        ] {
            assert!(parse_options(arguments(wrong)).is_err(), "{wrong}");
        }
    }

    #[test]
    fn orders_reference_pages_by_number_with_or_without_zeros() {
        let files: Vec<PathBuf> = [
            "lettera-10.png",
            "lettera-02.png",
            "lettera-1.png",
            "lettera-bis-1.png",
            "lettera-3.txt",
            "altro-1.png",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        assert_eq!(
            reference_pages(&files, "lettera"),
            vec![
                PathBuf::from("lettera-1.png"),
                PathBuf::from("lettera-02.png"),
                PathBuf::from("lettera-10.png")
            ]
        );
    }
}
