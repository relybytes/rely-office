# Changelog

All notable changes to rely-office. The project follows [Semantic Versioning](https://semver.org);
below 1.0, a minor version may change the API.

## 0.8.0

### Added

- Markdown and HTML read into the document tree (`read_document_tree_named`, and
  `readDocumentTree(bytes, limits, options, fileName)` from JavaScript): CommonMark with tables,
  task lists, footnotes and math through `pulldown-cmark`, and HTML through `html5gum`, which
  never runs what a page holds. `write_markdown` and `write_html` write the tree back; the
  renderer draws both formats like a document.
- `write_odt` writes the document tree as an odt: styles, lists, tables, images, notes, comments
  with replies, tracked changes, page fields, headers, footers and sections. `write_odp` writes
  slides as an odp, with their shapes, fills, tables, notes, transitions and animations. From
  JavaScript, `writeOdt`, `writeMarkdown`, `writeHtml`, `writeOdp`; in the frontend package,
  `saveDocumentAs(document, 'odt' | 'markdown' | 'html' | 'docx')` and the slide editor's
  `saveOdp`.
- `write_csv` saves a sheet as csv (UTF-8 with its byte order mark, CRLF, the cells as shown),
  with `default_separator(locale)` and `Workbook::delimiter` for the separator of the file read;
  `saveCsv` and `delimiter` from JavaScript.
- Names in formulas: `LET`, `LAMBDA` called in place or by `MAP`, `REDUCE`, `SCAN`, `BYROW`,
  `BYCOL` and `MAKEARRAY`, `ISOMITTED`, and the names a workbook defines (`set_defined_name`,
  `remove_defined_name`, `defined_names`), for the workbook or one sheet, which follow row,
  column and sheet edits and are read and written in xlsx and ods; a named `LAMBDA` is called by
  its name.
- Cell comments: `set_comment`, `Sheet::comments`, read from and written to xlsx (with Excel's
  note boxes) and ods, moved with their cells.
- The rest of Excel's function library, 489 functions in all, each with its Italian name where it
  has one: the distributions and their tests, complex numbers and Bessel functions, the database
  functions, bonds and discounted securities, regression and matrices, `OFFSET`, `INDIRECT`,
  `TRIMRANGE`, `XMATCH`, `AGGREGATE`, `CELL`, random numbers and regular expressions. See
  `docs/editing.md` for the list and for the functions left out on purpose.
- Charts made and changed in a sheet: `charts`, `chart_for_range`, `add_chart`, `set_chart` and
  `remove_chart`, over the drawing part the sheet carries, so the rest of the drawing is kept as
  the file had it; a chart read back from xlsx or ods stays editable. `ChartSpec` is read from
  and written as a `DrawingML` chart part and an `OpenDocument` chart object.
- The grid: panels for comments, defined names and charts, a commented cell marked and its
  comment shown on hover, charts chosen, dragged, resized and removed with Delete.
- Floating images in the document editor: the anchor crosses the JSON, the image stands on its
  side of the text, and the toolbar sets its wrapping and side; resizing it resizes its anchor.

### Changed

- A floating image's `anchor` is part of the tree's JSON (`ImageAnchor` in the model); an editor
  that ignores it keeps it as it came.
- An ods keeps its pictures and charts on save, and a chart of an xlsx goes to an ods as a chart.
- The odt writer and the HTML writer place a floating image as its anchor says.
- `YEARFRAC` on basis 1 follows Excel within a year, and on basis 0 applies the February rule.
- A reference a function only describes (`ROW`, `COLUMNS`, `OFFSET`'s first argument) is no
  longer a dependency, so `=OFFSET(A1,1,0)` in `A1` is not circular.
- A function the grid does not know keeps the name it was typed with, as a defined `LAMBDA`
  needs; an unknown name computes `#NAME?` as before.
- New error codes: `INVALID_NAME` and `INVALID_CHART`.

### Fixed

- `styles.css` sets the size and weight of headings, the markers and indent of lists and the
  rule of a horizontal line in the document editor and in the panel's small editors. A host with
  a CSS reset, Tailwind's preflight among them, showed headings at the size of the text and lists
  without bullets.

## 0.7.0

### Added

- `@relybytes/rely-office-react`, the frontend package: documents, presentations and spreadsheets
  read and written in the browser (`initDocuments`, `readDocumentTree`, `writeDocx`,
  `convertToDocx`, `readPresentation`, `writePptx`, `convertToPptx`, `initSheets`, `createSheet`,
  `openSheet`), from the reading and spreadsheet modules bound for the web.
- `DocumentEditor`, a React editor on TipTap over the document tree. `documentTreeToEditor` and
  `editorToDocumentTree` map every block, mark and inline both ways, alignment, superscript and
  subscript, font, size and colour, highlights, comments, tracked changes (accepted or rejected
  from the toolbar), footnote references, equations, inline images and section breaks included;
  what the editor does not show as text rides along and is written back. `openDocument` and
  `saveDocument` open a docx, doc or odt and save a docx; `createRelyDocumentExtensions` serves a
  host that draws its own editor.
- `SheetGrid`, a spreadsheet grid over `SheetDocument`, drawn by window: a formula bar, styles,
  number formats, merges, conditional formats with data bars and icons, validation that refuses a
  value or offers its list, rows, columns and sheets, and the clipboard, which pastes a copy of
  the workbook with its references moved.
- `SlideEditor`, a slide editor over the slide model: thumbnails, shapes drawn with their
  geometry, fill, gradient, picture and outline, moved, resized and written into with the
  formatting of each run kept, text styling, pictures, tables, slides added, duplicated,
  reordered and hidden, speaker notes; layout shapes show locked.
- English and Italian labels for the three editors; the look is set by CSS custom properties.
- The document editor's side panel: comments with replies and resolved state, footnotes and
  endnotes, headers and footers (a section's own included), each in a small editor of the same
  schema (`BlocksEditor`); a comment on the selection and a footnote at the caret from the toolbar.
  On save only what the text still refers to is kept, the notes numbered in order
  (`compactNotes`, `compactComments`).
- Formulas in the locale: `parse_formula_in` and `print_formula_in` read and write the Italian
  names, `;` and the decimal comma; `Input::text_in` shows a cell as its locale types it.
- Undo and redo: `EditHistory` in `rely-office-sheet`, `undo`, `redo`, `canUndo` and `canRedo` on
  every `SheetDocument`, and in the grid and the slide editor with Ctrl+Z and Ctrl+Y.
- The document tree carries what a paragraph sets itself, `format`: a named style of Word
  (title, subtitle, intense quote, caption, no spacing), spacing, line spacing, indents and a page
  break; and a table its `columnWidthsTwips`. Both are read from docx and written back.
- The document editor: font, size and line spacing in the toolbar, the named styles of Word in
  the style list, page breaks, a page view with the document's page size, margins, headers and
  footers drawn on every page and pages numbered, find and replace (Ctrl+F) with case and
  replace all, tracked changes recorded while typing by the author and accepted or rejected one at
  a time, images resized by their corner with their proportions kept, and table columns resized
  with the mouse.
- 143 more spreadsheet functions, 298 in all, each with its Italian name: statistics of samples
  and pairs, distributions, depreciation and loans, engineering conversions and bits, text and
  information (see `docs/editing.md`). `FUNCTION_NAMES` and `function_names(locale)` list them.
- Sorting by several keys, the autofilter, frozen panes and the fill handle's series in the
  spreadsheet engine (`sort_range`, `set_auto_filter`, `set_filter_column`, `filter_choices`,
  `set_frozen`, `fill_range`); the filter and the frozen panes are read from and written to xlsx.
- The grid: rows and columns resized by dragging their header edge and fitted by a double click,
  the fill handle and Ctrl+D or Ctrl+R, frozen panes, sort and filter buttons with a menu of the
  values to show, a borders menu, panels that create and remove conditional formats and
  validations, the pictures and charts of the file drawn over the cells, and function names
  suggested while a formula is typed.
- The slide editor: a slide show (F5) with the transitions and the animations played click by
  click, a panel that sets the transition and adds, orders and times the animations, several
  shapes selected with Shift or a dragged box and moved together, rotation by a handle or a
  number, copy, cut, paste (shapes, text and images from elsewhere) and duplicate, twenty preset
  shapes drawn as outlines that stretch with their frame, tables inserted and edited cell by cell
  with rows, columns and colours, and guides that a moved shape snaps to.

- Dynamic arrays: a formula whose result is an array spills it into the cells beside its own,
  with `#SPILL!` where they are taken and `#CALC!` for an empty `FILTER`. `FILTER`, `SORT`,
  `SORTBY`, `UNIQUE`, `SEQUENCE`, `TRANSPOSE`, `TAKE`, `DROP`, `CHOOSEROWS`, `CHOOSECOLS`,
  `VSTACK`, `HSTACK`, `TOCOL`, `TOROW`, `WRAPROWS`, `WRAPCOLS`, `EXPAND`, `MMULT`, `MUNIT`,
  `FREQUENCY` and `TEXTSPLIT`, with their Italian names; operators over ranges computed value by
  value; array constants. Written to xlsx as Excel's dynamic arrays and to ods as matrix
  formulas, and read back from both (`Workbook::spills`, `Workbook::spill_anchor`).
- Charts drawn again from the values the cells hold now (`Workbook::pictures`,
  `redraw_carried_drawing`), so the grid's charts follow an edit.
- The frozen panes and the filter of an ods, read and written.
- The page view splits a paragraph that does not fit between its lines and a table between its
  rows, and fills in the page number and page count of headers and footers; a page field is the
  `field` of a text run in JSON.
- Tracked formatting changes: the `formatChange` mark with the marks a run had before, read from
  and written to `w:rPrChange`, recorded by the editor while changes are tracked and accepted or
  rejected like the others.

### Changed

- The model is declared once, in `packages/rely-office-node/model.d.ts`, and the frontend package
  compiles the same file, so the two packages declare the same shapes.
- `@relybytes/rely-office` is the backend package only: its browser entry
  `@relybytes/rely-office/web` moves into `@relybytes/rely-office-react`.
- `SheetDocument.input` and the `input` of `cells` give a cell in the document's locale
  (`=SOMMA(A1;0,5)` in Italian) instead of the file's English syntax; `setInput` reads both.
- In the Italian locale a `,` between two digits of a typed formula is a decimal comma even with
  no `;` in it: `=SOMMA(1,5)` is now one argument, 1.5, as Italian Excel reads it.

### Fixed

- A sum of nothing showed as `-0`: Rust's float sum starts from negative zero, and zero is now
  shown as `0` in every number format.

## 0.6.0

### Added

- Every DrawingML preset shape: the 97 that were drawn as their rectangle (curved and circular
  arrows, ribbons, waves, braces, scrolls, action buttons) come from LibreOffice's preset table
  (MPL-2.0), generated by `scripts/enhanced-presets.py`; a face a preset shades darker or lighter
  is drawn in its shade (`FillShade`).
- Gradient and picture fills (`Paint`): linear and radial gradients, stretched and tiled pictures,
  on shapes and slide backgrounds, read from pptx, odp and ppt, drawn, and written back by
  `write_pptx`.
- The pictures and charts of a sheet are drawn over the cells they are anchored in
  (`SheetDrawing::offset_emu`).
- ods rendering with its own layout: column widths, row heights, cell styles and their data
  styles as format codes, merges, page setup, header and footer, pictures and charts. A number
  wider than its column shows as `#` signs.
- EMF and WMF pictures drawn as SVG in documents, slides and sheets
  (`convert_metafile_to_svg`, the CLI's `svg` command); `write_docx` and `write_pptx` keep them.
- The drawn shapes and groups of a docx, an odt and a doc become SVG pictures where they float,
  with their fill, line and text.
- A doc's nested tables are read as tables inside their cell.
- Radar, bubble and stock charts, the last with high-low lines, up-down bars and close ticks; the
  chart of an ods reads the sheet ranges it names from its own copy of the cells.
- Presentations edited as slides: the slide model serializes to JSON, npm `readPresentation` and
  `writePptx`; each slide carries its speaker notes, `hidden`, its `Transition` and its
  `Animation`s, read from pptx, odp and ppt and written back as notes slides, `p:transition` and
  `p:timing`; shapes keep their `id` and are marked `inherited` when drawn from a layout or master.
- Comments of docx, odt and doc in `DocumentTree::comments`, with replies and done states, marked
  in the text by `Mark::Comment` and `Inline::CommentAnchor`; tracked changes kept as
  `Mark::Insertion` and `Mark::Deletion` with `TreeOptions { tracked_changes: Keep }`. Both are
  written back by `write_docx`.
- Equations: `Inline::Math` holds LaTeX read from Office Math and MathML, set as Typst math and
  written as Office Math; DejaVu Math TeX Gyre ships with the npm package.
- SmartArt drawn from the drawing Office keeps of it, in docx and pptx.
- The text of a shape stays editable: a shape with text is a text box whose `TextFrame::shape`
  names its preset, drawn as its outline and written back as `prstGeom`.
- Conditional formats and data validations of a sheet (`sheet_rules`): read from xlsx and ods,
  drawn, edited (`set_conditional_formats`, `conditional_looks`, `set_validations`,
  `validation_at`, `is_valid`, `invalid_cells`, `validation_items`) and written back; pivot tables
  carried through an xlsx; `write_ods`, npm `saveOds` and the CLI's `ods` command.
- Surface charts and the Office 2016 kinds of `cx:chartSpace`: waterfall, funnel, treemap,
  sunburst, histogram, Pareto and box and whisker.
- A ppt's and a doc's shapes take the adjust values they set, for about sixty shape types whose
  handles map onto their DrawingML preset; the binary trapezoid is drawn the right way up.
- Text flows around a picture along its outline (`Wrap::Contour`, `WrapOutline`), a tight wrap
  written back as `wp:wrapPolygon`; a paragraph flows around several pictures at once, with its
  tabs.
- Word 6 and Word 95 documents read into the document tree, with their comments, endnotes,
  pictures and drawing objects.
- Embedded objects in `DocumentTree::objects` (`EmbeddedObject`): the package or OLE compound file
  of a docx, the `draw:object-ole` of an odt and the object pool storage of a doc, each with the
  picture that shows it, written back by `write_docx` under `word/embeddings`; npm
  `readDocumentTree` returns them and `writeDocx` takes them.

### Changed

- `SlideShape::fill` and `Slide::background` are an `Option<Paint>` instead of a colour.
- `ImportReport::shapes` counts only the shapes that draw nothing; a shape that draws is a picture
  in `media`.
- `DocumentTree` gains `comments` and `objects`, `Inline` the `Math` and `CommentAnchor` variants,
  `Mark` the `Comment`, `Insertion` and `Deletion` variants, `TextFrame` its `shape`, `Wrap` the
  `Contour` variant, `PageLayout` `spacing_adds_up`, and `Slide` and `SlideShape` their show and
  identity fields: code that builds them field by field adds the new ones.
- Between two paragraphs the renderer keeps the larger space, as Word since 2007 and LibreOffice
  do, and adds them only for a document that asks for Word 2003's rule; a line's box is the office
  suite's single line. The corpus comparison passes 11 of 11 pages.
- A table row is taller by the double border under it.
- A docx `w:object` is its replacement picture instead of being left out.
- `rely-office-render` depends on `rely-office-sheet`, which computes the looks of conditional
  formats.
- Compound files built by the tests are version 3, as Office writes them.

## 0.5.0

### Added

- doc: Word 97-2003 files read into the document tree from their own structures (piece table,
  formatting pages, styles, fonts, lists, sections, pictures and drawings, notes, headers and
  footers, fields), laid out like a docx, imported into an editor and written back as a docx.
- ppt: PowerPoint 97-2003 files read into slides from their persist directories, masters, colour
  schemes, shapes with their preset or free-form geometry, groups, text styles, pictures and
  tables, and drawn like a pptx.
- `write_pptx`: the slide model back to a pptx, so a ppt or an odp converts to the new format.
- docx and odt sections of their own page: each section's size, orientation, margins, columns,
  headers and footers, read into `sections`, laid out and written back.
- Floating images and text boxes where they float: `wp:anchor`, VML positioning and odt frames
  read into an `anchor`, drawn in front, behind, beside or between the lines, and written back
  as `wp:anchor` pictures and `wps` text boxes.
- Shape geometry: the DrawingML presets (arrows, stars, callouts, flowchart symbols and about
  120 more) and custom geometries from their guide formulas, OpenDocument enhanced geometries,
  polygons and SVG paths, drawn as curves (`ShapeGeometry::Path`).
- npm: `convertToDocx` and `convertToPptx`; the tree's `sections`; `readDocumentTree` and
  `render` accept doc, and `render` lays out ppt.
- CLI: `rewrite` converts a doc to docx and a ppt, odp or pptx to pptx.

### Changed

- `Rendered::fidelity` is `layout` for doc and ppt; `text` only when such a file cannot be read.
- `ShapeGeometry` is no longer `Copy`, since a path geometry holds its commands.

## 0.4.0

### Added

- docx: headers and footers with their first-page and even-page variants and page numbers,
  footnotes and endnotes, text boxes, tab stops with their leaders, list labels in the
  numbering's own formats, table styles with their conditional regions, per-cell borders, fills
  and vertical alignment, paragraph borders and shading, highlights and the run fonts, sizes and
  colours as `textStyle` marks. All of it is read into the tree (`notes`, `headersFooters`,
  `page`, the `noteReference` inline, the `textStyle` and `highlight` marks), drawn, and written
  back by `write_docx`.
- odt: read into the same document tree, laid out like a docx and imported into an editor the
  same way; the tree is written back as a docx.
- pptx and odp: drawn as slides, one page each at the slide's size, over their layouts and
  masters, with backgrounds, text frames, bullets and numbering, custom shapes, lines, groups,
  rotations, tables and pictures (`read_presentation`).
- Charts: the charts of docx, xlsx, pptx, odt and odp are drawn as SVG from the values they
  store, or from the workbook's cells when they store none: columns, bars, lines, areas, pies,
  doughnuts and scatter charts with titles, legends, axis titles, gridlines, data labels and a
  secondary axis.
- xlsx rendering: number formats in Italian and English notation, fonts, fills, borders,
  alignment, wrapping, merged cells, row heights, hidden rows, and the sheet's pictures and charts
  after its cells (`SheetLayout::drawings`).
- `rely-office-sheet`: cell styles (`set_style` with a `StylePatch`, `set_borders`,
  `clear_styles`), `merge_cells` and `unmerge_cells`, `set_row_height`, number format codes, and
  102 more functions, 155 in all. Saving keeps the styles, merges and row heights, and writes the
  pictures, charts and theme of the file back unchanged (`read_xlsx_drawing_parts`,
  `read_xlsx_theme_part`).
- npm: `setStyle`, `setBorders`, `clearStyles`, `stylesJson`, `mergesJson`, `mergeCells`,
  `unmergeCells`, `setRowHeight` and the `INVALID_STYLE` code; the tree's `notes`,
  `headersFooters` and `page`; `readDocumentTree` and `render` accept odt, and `render` lays out
  pptx and odp.

### Changed

- `read_document_tree` accepts an odt besides a docx; `Rendered::fidelity` is `layout` for docx,
  odt, xlsx, xls, ods, csv, tsv, pptx and odp, `text` only for doc and ppt.
- A chart of a docx is an image of the tree instead of a count in the report.
- `read_workbook` reads numbers in English notation and `read_workbook_in` in the locale it is
  given; the renderer reads sheets in Italian.

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
