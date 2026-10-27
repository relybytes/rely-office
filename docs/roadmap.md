# Roadmap

Phase 1 is a server-side reader and viewer for the Drive of the Rely Platform; phase 2 the editing
the Drive needs from the same library. Estimates are in person-months (PM) and are the team's own,
with no external source.

## Phase 1: reading and rendering

| Step | Content | PM | Status |
| --- | --- | --- | --- |
| 1.1 | Detection, text extraction for docx and spreadsheets, sheet tables, CLI | 0.5 | done (0.1.0) |
| 1.2 | Text of pptx, odt, odp; text of doc and ppt through the compound file streams | 1-1.5 | done (0.2.0) |
| 1.2b | The docx document tree for import into a rich-text editor, with media and an import report | 0.5 | done (0.2.0) |
| 1.3 | Rendering through Typst: docx and spreadsheets written as Typst source, typeset in process | 1 | done (0.3.0) |
| 1.4 | `rely-office-render`: PDF and PNG, the looks of a docx and the print settings of an xlsx, metric-compatible fonts (Carlito, Caladea, Liberation, OFL) | 2-3 | done (0.3.0) |
| 1.5 | `@relybytes/rely-office`: the WebAssembly package for Node, used in process by the Drive service | 1 | done (0.2.0) |
| 1.6 | Corpus of Italian documents and a page comparison against LibreOffice in CI; `cargo-fuzz` targets | 1-2 | done (0.3.0) |

Step 1.3 was planned on office2pdf as a base. It was not used: the document tree already held the
structure, so the renderer writes Typst source from it and from the new looks, with no second
reader to maintain. Step 1.5 was planned as a napi addon and became WebAssembly: one artifact for
every platform, no native build in the Drive's image.

**The 95% gate.** The gate was written to decide when the Drive could drop Gotenberg for previews.
Gotenberg was dropped by decision on 2026-10-25, so the gate now measures quality rather than
deciding anything. On 2026-10-27, 6 of 11 corpus pages pass the threshold (54.5%): every sheet
page, and every docx with the page count LibreOffice prints; the docx pages that fail drift by a
few pixels per heading, where LibreOffice and Word space paragraphs differently and the renderer
follows Word. [rendering.md](rendering.md) has the method and the figures.

## Phase 2: editing

| Step | Content | Status |
| --- | --- | --- |
| 2.1 | `write_docx`: the document tree back to a docx Word and LibreOffice open, read back to the same tree | done (0.3.0) |
| 2.2 | `rely-office-sheet`: cells, formulas (53 functions, Italian names accepted), recalculation with cycle detection, number formats | done (0.3.0) |
| 2.3 | Row, column and sheet edits that move references; paste with relative references | done (0.3.0) |
| 2.4 | xlsx, xls, ods, csv and tsv in, xlsx out with cached values | done (0.3.0) |
| 2.5 | The spreadsheet for JavaScript, in Node and in the browser | done (0.3.0) |

The editors themselves are the platform's: the Documento's rich-text editor maps the document
tree, and the Foglio's grid drives a `SheetDocument`.

## Phase 3: fidelity (0.4.0)

| Step | Content | Status |
| --- | --- | --- |
| 3.1 | docx headers, footers with their first and even variants, page numbers, footnotes and endnotes, text boxes, tab stops with leaders, the numbering's own label formats, table styles, per-cell borders, fills and vertical alignment, paragraph borders and shading, highlights; read, drawn and written back | done (0.4.0) |
| 3.2 | xlsx cells with their number formats, fonts, fills, borders, alignment, wrapping, merges and row heights, drawn and edited (`set_style`, `set_borders`, `merge_cells`, `set_row_height`); 155 functions | done (0.4.0) |
| 3.3 | pptx and odp drawn as slides over their layouts and masters | done (0.4.0) |
| 3.4 | Charts of docx, xlsx, pptx, odt and odp drawn as SVG; the pictures and charts of an edited xlsx written back | done (0.4.0) |
| 3.5 | odt read into the document tree: laid out, and imported into the editor like a docx | done (0.4.0) |

## Phase 4: layout and the binary formats (0.5.0)

| Step | Content | Status |
| --- | --- | --- |
| 4.1 | Sections of their own page in docx and odt: read, laid out and written back | done (0.5.0) |
| 4.2 | Floating images and text boxes where they float, in docx (DrawingML and VML) and odt | done (0.5.0) |
| 4.3 | Shape geometry: the DrawingML presets and custom geometries, OpenDocument enhanced geometries, polygons and paths | done (0.5.0) |
| 4.4 | doc read into the document tree: laid out, imported into the editor, converted to docx | done (0.5.0) |
| 4.5 | ppt read into slides: drawn, and converted to pptx with `write_pptx` | done (0.5.0) |

A doc and a ppt are converted rather than edited in place: the library reads them into the same
structures a docx and a pptx give, so rendering, editing and saving only know the new formats, and
a converted file is saved as docx or pptx.

## Phase 5: drawing what the files draw (0.6.0)

| Step | Content | Status |
| --- | --- | --- |
| 5.1 | Every DrawingML preset, the faces shaded as the preset says | done (0.6.0) |
| 5.2 | Gradient and picture fills of shapes and slide backgrounds, read from pptx, odp and ppt and written back | done (0.6.0) |
| 5.3 | The pictures and charts of a sheet over the cells they are anchored in; the ods layout | done (0.6.0) |
| 5.4 | EMF and WMF pictures drawn as SVG | done (0.6.0) |
| 5.5 | The drawn shapes of docx, odt and doc as pictures where they float; nested doc tables | done (0.6.0) |
| 5.6 | Radar, bubble and stock charts | done (0.6.0) |

## Next

- The adjust values of a ppt's preset shapes, which OfficeArt gives in each shape's own units.
- Surface charts and the `chartex` charts (waterfall, treemap, sunburst).
- A larger corpus with documents made by Word and pptx and odp pages, and the corpus figure as a
  CI gate once it is stable.
- Publishing to the public npm registry (issue #1).

## Known risks

- Typst lays out pages its own way: a Word document may break pages differently.
- Calibri and Cambria cannot be redistributed; metric-compatible fonts keep line breaks close.
- The renderer module is 27 MB of WebAssembly, most of it Typst; it loads only on the first
  render, and a process that only reads never loads it.
- Typst pulls in crates that RustSEC lists as unmaintained (ttf-parser, rustybuzz, yaml and
  bincode crates); `deny.toml` records each with its reason, to revisit on every Typst upgrade.
- The binary formats (doc, xls, ppt) are the long tail: xls is read through calamine; doc and ppt
  are read from their own records, which a malformed file can only cut short, and Word 6 and 95
  files are refused.
- calamine holds a sheet as a dense range from its first to its last used cell, so a workbook with
  two cells far apart costs memory in proportion to the rectangle between them, inside the input
  and member caps.
