# Changelog

All notable changes to rely-office. The project follows [Semantic Versioning](https://semver.org);
below 1.0, a minor version may change the API.

## 0.3.0

### Added

- `rely-office-render`: documents to PDF and PNG through Typst 0.15, in process. A docx is laid
  out from its document tree with its page, spacing, line height computed from the fonts' metrics,
  indents, page breaks, fonts, sizes, colours, list indents, table grids, cell margins and borders;
  xlsx sheets print with their orientation, margins, column widths, header, footer and grid lines;
  xls, ods, csv and tsv with Excel's defaults; pptx, odt, odp, doc and ppt as their text. See
  `docs/rendering.md`.
- `write_docx`: a document tree back to a docx that reads back to the same tree.
- `rely-office-sheet`: an editable spreadsheet with formulas (53 functions, Italian names and
  separators accepted), iterative recalculation with cycle detection, number formats, row, column
  and sheet edits that move references, paste with relative references, and xlsx, xls, ods, csv
  and tsv in, xlsx out with cached values. See `docs/editing.md`.
- The document tree carries `align` on paragraphs and headings, and, in Rust only, the looks of
  paragraphs, runs and tables and the page layout; it also deserializes from JSON.
- `read_xlsx_layouts` and `Sheet::layout`: the print settings of xlsx sheets.
- npm: `render`, `writeDocx`, `createSheet`, `openSheet`, and the browser entry
  `@relybytes/rely-office/web` for the spreadsheet; the package ships the OFL fonts.
- CLI: `typst`, `pdf`, `png` and `rewrite`.
- The corpus of Italian documents and the page comparison with LibreOffice in CI
  (`scripts/corpus.mjs`, `rely-office-corpus`), and six cargo-fuzz targets with a weekly run.

### Changed

- The minimum Rust version is 1.92, which Typst requires.
- The npm package is published to GitHub Packages.
- Error cells of a spreadsheet show their code (`#DIV/0!`) instead of the reader's debug name.
- The WebAssembly modules are built with their symbols stripped.

## 0.2.0

### Added

- Text extraction for pptx (slides in presentation order, each followed by its speaker notes),
  odt (body, then the headers and footers of the master pages), odp (pages in order with their
  notes), doc (Word 97-2003, through the piece table) and ppt (PowerPoint 97-2003, through the text
  atoms of the document stream, masters left out).
- `read_document_tree`: a docx as a neutral tree of headings, paragraphs, list items, tables,
  quotes, code blocks and rules, with marked text runs, links limited to http, https and mailto,
  hard breaks and images, plus the embedded images with their sniffed type and an import report of
  what the tree left out. See `docs/document-tree.md`.
- The `serde` feature of `rely-office-core`, serializing the document tree and its report.
- `Limits::max_image_bytes`, `max_images` and `max_total_image_bytes` for the images of a tree.
- `OfficeError::code()`, a stable code per error, and the variants `UnreadableDocument` and
  `Encrypted` (a password-protected doc or ppt).
- `DocumentFormat::as_str()`, the lowercase name of a format.
- `rely-office-wasm` and the npm package `@relybytes/rely-office`: `detectFormat`, `extractText`,
  `readSheets` and `readDocumentTree` from Node through WebAssembly, errors as `RelyOfficeError`
  with the stable code, built and packed by `scripts/build-npm.mjs`.
- The `tree` command of the CLI.
- CI builds the npm package and runs its smoke test; `release.yml` publishes it on a `v*` tag.

### Changed

- `extract_text` returns an `ExtractedText` (`format`, `text`, `truncated`) instead of a `String`.
- Empty paragraphs no longer add blank lines to the search text, and the fallback branch of
  `mc:AlternateContent` is no longer read twice into a docx's text.

## 0.1.0

- Format detection from the bytes, text extraction for docx and spreadsheets, sheet tables from
  A1 for xlsx, xls, ods, csv and tsv, and the `rely-office` command.
