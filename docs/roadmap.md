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
deciding anything. On 2026-10-28, 11 of 11 corpus pages pass the threshold (100%), the worst at
0.023: the drift per heading came from summing the spaces between paragraphs where Word since 2007
and LibreOffice keep the larger, and the last page from the room a double border takes in a table
row. [rendering.md](rendering.md) has the method and the figures.

## Phase 2: editing

| Step | Content | Status |
| --- | --- | --- |
| 2.1 | `write_docx`: the document tree back to a docx Word and LibreOffice open, read back to the same tree | done (0.3.0) |
| 2.2 | `rely-office-sheet`: cells, formulas (53 functions, Italian names accepted), recalculation with cycle detection, number formats | done (0.3.0) |
| 2.3 | Row, column and sheet edits that move references; paste with relative references | done (0.3.0) |
| 2.4 | xlsx, xls, ods, csv and tsv in, xlsx out with cached values | done (0.3.0) |
| 2.5 | The spreadsheet for JavaScript, in Node and in the browser | done (0.3.0) |

Until phase 7 the editors were each application's own; since 0.7 they are the library's too, in
the frontend package (phase 7).

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

## Phase 6: the preview and the editing complete (0.6.0)

| Step | Content | Status |
| --- | --- | --- |
| 6.1 | Presentations edited as slides in JSON, with their speaker notes, hidden slides, transitions and animations, read from pptx, odp and ppt and written back | done (0.6.0) |
| 6.2 | Comments and tracked changes of docx, odt and doc kept in the tree and written back | done (0.6.0) |
| 6.3 | Equations read as LaTeX from Office Math and MathML, drawn as Typst math, written as Office Math | done (0.6.0) |
| 6.4 | SmartArt drawn from the drawing Office keeps of it, in documents and slides | done (0.6.0) |
| 6.5 | The text of a shape kept editable in a text box of its shape | done (0.6.0) |
| 6.6 | Conditional formats and data validation of a sheet: read, drawn, edited and written; pivot tables carried through an xlsx; a workbook saved as ods | done (0.6.0) |
| 6.7 | Surface charts and the Office 2016 chart kinds (waterfall, funnel, treemap, sunburst, histogram, Pareto, box and whisker) | done (0.6.0) |
| 6.8 | A ppt's shapes with the adjust values they set | done (0.6.0) |
| 6.9 | Text flowing around a picture along its outline | done (0.6.0) |
| 6.10 | Paragraph spacing and line heights as Word and LibreOffice set them | done (0.6.0) |
| 6.11 | Word 6 and Word 95 documents read into the tree | done (0.6.0) |
| 6.12 | Embedded objects kept with their picture and written back into a docx | done (0.6.0) |

An xls is saved as xlsx after an edit, by the user's decision of 2026-10-27: nothing writes the
binary formats.

## Phase 7: editor components (0.7.0)

The mapping between the document tree and a rich-text editor was written by each application that
edits a document, the Rely Platform's Documento among them, and had nothing specific to any of
them. Phase 7 moved it into the library, with React components, in a frontend package of its own:
`@relybytes/rely-office` stays the backend package, `@relybytes/rely-office-react` is the
frontend one, and both declare the same model from `packages/rely-office-node/model.d.ts`.

| Step | Content | Status |
| --- | --- | --- |
| 7.1 | The document tree to and from a ProseMirror document: every block, mark, list, table and inline of the tree has a node or a mark, and what the editor does not show (notes, comments, page, headers, sections, objects) rides along and is written back | done (0.7.0) |
| 7.2 | `DocumentEditor` on TipTap: toolbar, tables, inline images, comments, tracked changes accepted or rejected, equations, links; opening and saving docx, doc and odt through the browser build | done (0.7.0) |
| 7.3 | `SheetGrid` over `SheetDocument`: drawn by window, formulas, styles, formats, merges, conditional formats and validation, rows, columns and sheets, the clipboard with references moved | done (0.7.0) |
| 7.4 | `SlideEditor` over the slide model: shapes moved, resized and written into with their run formatting kept, pictures, tables, slides and speaker notes, saving pptx | done (0.7.0) |
| 7.5 | Formulas shown in the locale's names and separators, `=SOMMA(A1;0,5)`, and read back the same | done (0.7.0) |
| 7.6 | Comments, notes, headers and footers edited in a panel beside the document | done (0.7.0) |
| 7.7 | Undo and redo in the spreadsheet, from the engine's history, and in the slide editor | done (0.7.0) |
| 7.8 | The document editor a Word user expects first: font, size and line spacing, Word's named styles, the page view with headers and footers, find and replace, tracked changes recorded while typing, images and table columns resized by the mouse | done (0.7.0) |
| 7.9 | The grid a spreadsheet user expects first: resizing by drag, the fill handle and its series, frozen panes, sort and filter, borders, the conditional format and validation editors, charts over the grid, formula suggestions, and 298 functions | done (0.7.0) |
| 7.10 | The slide editor a presenter expects first: the show with transitions and animations, their editor, rotation, several shapes at once, the clipboard, preset shapes, tables edited cell by cell, alignment guides | done (0.7.0) |
| 7.11 | Dynamic arrays and the functions that return them, charts drawn from the current cells, the frozen panes and the filter of an ods, the page view splitting a block over pages and filling the page fields, tracked formatting changes | done (0.7.0) |

The host application keeps what is its own: where a file is stored, who may open it, locking and
autosave, and where an inserted image goes, all through the components' props.

## Phase 8: every format edited and saved (0.8.0)

| Step | Content | Status |
| --- | --- | --- |
| 8.1 | Markdown and HTML read into the document tree and written from it; a sheet saved as csv | done (0.8.0) |
| 8.2 | A document tree written as an odt, and slides as an odp, so an odt and an odp are saved as themselves | done (0.8.0) |
| 8.3 | Defined names, `LET`, `LAMBDA` and the functions that call one, read and written in xlsx and ods | done (0.8.0) |
| 8.4 | Cell comments read, edited and written in xlsx and ods | done (0.8.0) |
| 8.5 | The rest of Excel's function library, 489 functions, with the computed references of `OFFSET` and `INDIRECT` | done (0.8.0) |
| 8.6 | Charts of a sheet made from a selection, changed, moved and removed; the pictures and charts of an ods kept on save | done (0.8.0) |
| 8.7 | Floating images edited in the document editor and written to docx, odt and HTML | done (0.8.0) |

## Phase 9: the rest of a spreadsheet user's file (unreleased)

| Step | Content | Status |
| --- | --- | --- |
| 9.1 | The threaded comments of Excel 365: replies, dates and resolved state, read, edited and written in xlsx, one annotation in ods | done |
| 9.2 | Macros kept and never run: the VBA project of a docm, an xlsm and a pptm written back, the macros of a doc, an xls and a ppt reported when a conversion leaves them out | done |
| 9.3 | Pivot tables made, changed, refreshed and removed, written to and read from xlsx and ods, with `GETPIVOTDATA`, `GROUPBY` and `PIVOTBY` | done |
| 9.4 | Editing together: documents, presentations and text as a shared Yjs document, a sheet as the operations a room orders, presence, and undo of one's own edits | done |
| 9.5 | Plain text files edited with their encoding, byte order mark and line ending kept | done |
| 9.6 | The functions left that stay inside the workbook, 502 in all, and the stored value of those that reach outside | done |
| 9.7 | The adjust values of the old arrows, ribbons and action buttons of ppt and doc | done |

## Next

- A real Word 6 or 95 sample in the tests, beside the synthetic ones.
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
  are read from their own records, which a malformed file can only cut short. Word 6 and 95 files
  are read through a translation of their structures checked against LibreOffice on synthetic
  files only, since no real sample was at hand.
- calamine holds a sheet as a dense range from its first to its last used cell, so a workbook with
  two cells far apart costs memory in proportion to the rectangle between them, inside the input
  and member caps.
