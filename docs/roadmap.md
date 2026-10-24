# Roadmap

Phase 1 is a server-side viewer for the Drive of the Rely Platform. Estimates are in person-months
(PM) and are the team's own, with no external source.

| Step | Content | PM | Status |
| --- | --- | --- | --- |
| 1.1 | Detection, text extraction for docx and spreadsheets, sheet tables, CLI | 0.5 | done (0.1.0) |
| 1.2 | Text of pptx, odt, odp; text of doc and ppt through the compound file streams | 1-1.5 | next |
| 1.3 | Rendering spike: docx and xlsx to PDF through Typst, office2pdf (Apache-2.0) evaluated as a base | 1 | planned |
| 1.4 | `rely-office-render`: PDF and PNG thumbnails, metric-compatible fonts (Carlito, Caladea, OFL) | 2-3 | planned |
| 1.5 | `rely-office-node`: napi addon for linux-x64 musl, used by the Drive service | 1 | planned |
| 1.6 | Corpus of Italian documents and a pixel comparison against LibreOffice in CI; `cargo-fuzz` targets | 1-2 | planned |

**Gate after 1.6:** the Drive drops Gotenberg for previews only when at least 95% of the corpus
pages pass the pixel threshold. Below it, Gotenberg stays for previews and rely-office keeps text
extraction and thumbnails.

Out of phase 1: editing (IronCalc for spreadsheets, an existing editor for Word documents),
collaboration, and WebAssembly for the browser, which phase 1 does not need.

## Known risks

- Typst lays out pages its own way: a Word document may break pages differently.
- Calibri and Cambria cannot be redistributed; metric-compatible fonts keep line breaks close.
- office2pdf has a single maintainer: contribute upstream before forking.
- The binary formats (doc, xls, ppt) are the long tail; xls is read today through calamine.
