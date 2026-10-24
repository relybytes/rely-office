# Roadmap

Phase 1 is a server-side reader and viewer for the Drive of the Rely Platform. Estimates are in
person-months (PM) and are the team's own, with no external source.

| Step | Content | PM | Status |
| --- | --- | --- | --- |
| 1.1 | Detection, text extraction for docx and spreadsheets, sheet tables, CLI | 0.5 | done (0.1.0) |
| 1.2 | Text of pptx, odt, odp; text of doc and ppt through the compound file streams | 1-1.5 | done (0.2.0) |
| 1.2b | The docx document tree for import into a rich-text editor, with media and an import report | 0.5 | done (0.2.0) |
| 1.3 | Rendering spike: docx and xlsx to PDF through Typst, office2pdf (Apache-2.0) evaluated as a base | 1 | planned |
| 1.4 | `rely-office-render`: PDF and PNG thumbnails, metric-compatible fonts (Carlito, Caladea, OFL) | 2-3 | planned |
| 1.5 | `@relybytes/rely-office`: the WebAssembly package for Node, used in process by the Drive service | 1 | done (0.2.0) |
| 1.6 | Corpus of Italian documents and a pixel comparison against LibreOffice in CI; `cargo-fuzz` targets | 1-2 | planned |

Step 1.5 was planned as a napi addon built for linux-x64 musl. WebAssembly replaced it: one
artifact for every platform the Drive runs on, no native build in its image, and the same memory
safety as the Rust core. Rendering (1.3, 1.4) will reach Node through the same package.

**Gate after 1.6:** the Drive drops Gotenberg for previews only when at least 95% of the corpus
pages pass the pixel threshold. Below it, Gotenberg stays for previews and rely-office keeps text
extraction, the import tree and thumbnails.

Out of phase 1: editing (IronCalc for spreadsheets, an existing editor for Word documents),
collaboration, a browser build of the package (the WebAssembly module runs there, the bindings are
generated for Node only), and a document tree for odt.

## Known risks

- Typst lays out pages its own way: a Word document may break pages differently.
- Calibri and Cambria cannot be redistributed; metric-compatible fonts keep line breaks close.
- office2pdf has a single maintainer: contribute upstream before forking.
- The binary formats (doc, xls, ppt) are the long tail: xls is read through calamine; doc and ppt
  give text for search only, ppt in stream order, and Word 6 and 95 files are refused.
- calamine holds a sheet as a dense range from its first to its last used cell, so a workbook with
  two cells far apart costs memory in proportion to the rectangle between them, inside the input
  and member caps.
