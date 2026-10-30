# Architecture

rely-office turns office documents into what a file manager needs from them: what they are, the
text to index, a table to show, a document tree to import into an editor, a PDF and a thumbnail;
it writes a docx, an odt, Markdown or HTML back from that tree and a pptx or an odp from slides,
and it is the model behind a spreadsheet editor.

## Why a library of our own

The Drive of the Rely Platform previewed office files through a LibreOffice container (Gotenberg)
or not at all, and imported Word files into its editor with a JavaScript converter. A library in
Rust, called in process from the Drive service, reads untrusted uploads with a memory-safe parser
and gives one implementation of text extraction, sheet tables, the import tree, and later previews
and thumbnails. It is published under MPL-2.0 so improvements to it stay public.

The user's decision of 2026-10-25 is that the Drive depends on rely-office alone for office
files: no LibreOffice container for previews, no JavaScript docx library for export, no third-party
spreadsheet engine. Rendering goes through Typst, a typesetter in Rust linked into the library;
editing a Word document goes through the document tree and the platform's own rich-text editor;
editing a spreadsheet goes through `rely-office-sheet` and the platform's own grid.

Editing together is in the frontend package, over Yjs and a room the host runs
([collaboration.md](collaboration.md)); the Rust crates know nothing of it beyond the regions and
checkpoints a shared workbook undoes and rolls back with.

What it deliberately does not do: pixel-perfect Word layout (Typst lays pages out
its own way; [rendering.md](rendering.md) measures how close), macros, editing pivot tables, and
the functions that call outside the workbook.

## Crates and packages

| Crate or package | Role | Depends on |
| --- | --- | --- |
| `rely-office-core` | detection, text extraction, sheet tables, print settings and rules, defined names, comments and chart specs, the document tree with its looks, Markdown and HTML, the slide model, the docx, odt, Markdown, HTML, pptx and odp writers | `zip`, `quick-xml`, `calamine`, `cfb`, `encoding_rs`, `pulldown-cmark`, `html5gum`, `thiserror`; `serde` behind the `serde` feature |
| `rely-office-render` | documents written as Typst source and typeset to PDF and PNG, see [rendering.md](rendering.md) | `rely-office-core`, `rely-office-sheet` (for the looks of conditional formats), `typst`, `typst-layout`, `typst-pdf`, `typst-render`, `comemo` |
| `rely-office-sheet` | the editable spreadsheet: formulas, names and functions, recalculation, edits, comments, charts, conditional formats and validation, xlsx, ods and csv out, see [editing.md](editing.md) | `rely-office-core`, `calamine`, `quick-xml`, `zip`, `regex`, `thiserror` |
| `rely-office-cli` | the `rely-office` command for trying files by hand | the three crates above |
| `rely-office-wasm`, `rely-office-render-wasm`, `rely-office-sheet-wasm` | the WebAssembly fronts for JavaScript, not published as crates | `wasm-bindgen`, `serde`, `serde_json` |
| `rely-office-corpus` | the page comparison with LibreOffice, not published | `rely-office-render`, `tiny-skia` |
| `@relybytes/rely-office` (npm, `packages/rely-office-node`) | the backend package: hand-written wrappers and types over the bindings generated for Node, the fonts, and `model.d.ts`, the model both packages declare | the three modules |
| `@relybytes/rely-office-react` (npm, `packages/rely-office-react`) | the frontend package: the reading and spreadsheet modules bound for the web, the same calls in TypeScript, and the React editors for documents (on TipTap), spreadsheets and slides | `rely-office-wasm`, `rely-office-sheet-wasm`; React and TipTap as peers |

## Inside the core

| Module | Reads |
| --- | --- |
| `format` | the format, from the zip members or the compound file streams |
| `archive`, `compound` | zip members and compound file streams, each through a size cap |
| `relationships` | the `_rels/*.rels` parts of OOXML: targets resolved like OPC, the main part from `_rels/.rels` |
| `xml_text` | a streaming walk from XML to search text, configured per format by a `TextRules` |
| `xml_tree` | a small element tree with resolved namespaces, depth-capped, for structural reads |
| `word`, `presentation`, `opendocument` | the text of docx, pptx, odt and odp; `opendocument` also reads an odt into the document tree, an odp into slides and an ods into a sheet layout, through its style resolver, and writes the tree as an odt (`text_write`) and slides as an odp (`presentation_write`) |
| `doc`, `ppt` | the text of the binary formats, a doc as the document tree and a ppt as slides; `doc/word6` rewrites the structures of a Word 6 or 95 file as Word 97 stores them |
| `office_art` | the drawing records doc and ppt share: shapes, their properties and the pictures of the blip store |
| `geometry` | shape outlines as paths: the `DrawingML` presets (the ones `presets.rs` writes in guide language, the rest generated into `enhanced_presets.rs`) and custom geometries, `OpenDocument` enhanced geometries, polygons and SVG paths; `picture` draws the shapes of a document as an SVG picture |
| `metafile` | EMF and WMF pictures played on a device context and written as SVG |
| `sheet` | spreadsheets as tables, through calamine, and delimited text |
| `sheet_layout`, `sheet_styles`, `number_format` | the print settings, cell styles and merges of xlsx sheets, read as a stream that skips the cells, and Excel's number format codes |
| `sheet_drawings` | the pictures and charts a sheet's drawing anchors to its cells, and the drawing and pivot table parts an editor writes back unchanged |
| `sheet_rules` | the conditional formats and data validations of a sheet, the model the renderer and the editor share, read from xlsx and ods |
| `sheet_names` | the names a workbook defines and the comments of its cells, read from xlsx and ods |
| `chart_spec` | a chart as an editor describes it, read from and written as a `DrawingML` chart part and an `OpenDocument` chart object |
| `document` | the docx document tree: styles, table styles, numbering, notes, comments, tracked changes, headers and footers, sections, floating anchors and wrap outlines, shapes and SmartArt (`convert/shapes`), embedded objects (`objects`), conversion, looks, media sniffing, the docx writer, and Markdown and HTML read and written (`markdown`, `html`) |
| `math` | one formula tree for equations: LaTeX read and written, Office Math and MathML read, Office Math and Typst math written |
| `slides`, `theme` | a pptx as slides over its layouts and masters, their notes, transitions and animations (`show`), the pptx writer, and the colours and fonts of a `DrawingML` theme |
| `chart` | `DrawingML` and `OpenDocument` charts, read from their cached values or cells and drawn as SVG, and the Office 2016 kinds of `cx:chartSpace` (`extended`) |

## Decisions

- **The bytes decide the format.** A name can say `.docx` over an xlsx; only CSV and TSV, which
  carry no signature, are told apart by their extension, and only when the bytes look like text. A
  compound file is a doc when it holds `WordDocument`, an xls with `Workbook` or `Book`, a ppt with
  `PowerPoint Document`.
- **Every read is bounded.** `Limits` caps the input, each decompressed archive member or stream
  (read through `take(cap + 1)`, so a zip bomb stops at the cap, and a compound file stream is also
  refused when its declared length is past the cap), the characters of text, the rows and columns
  of a table, and the images of a document tree by size, count and total. Element trees refuse
  nesting deeper than 256 levels. Nothing panics on malformed input; errors are `OfficeError`, each
  with a stable `code()`.
- **Search text is reading order, not layout.** A pptx is read slide by slide in the order of
  `p:sldIdLst`, each slide followed by its notes; slide number and date fields are left out. An odt
  adds the headers and footers of its master pages after the body; an odp keeps each page's notes
  after its frames. The fallback branch of `mc:AlternateContent` repeats its choice and is skipped.
  Empty paragraphs collapse, since a blank line matches nothing.
- **Word 97-2003 through the piece table.** The File Information Block names the table stream
  (`0Table` or `1Table`) and the `Clx` in it; each piece is either compressed (Windows-1252, at half
  its file offset) or UTF-16LE. Fields keep their result and drop their code, cell marks become
  tabs. An encrypted file or a piece table pointing outside its streams is refused with an error,
  never read past the end.
- **Word 6 and 95 through Word 97's shapes.** A file whose `nFib` is below 192 has its File
  Information Block, 8-bit text, formatting pages, style sheet, font table and sections rewritten
  as Word 97 stores them, and its one-byte property modifiers turned into Word 97's two-byte ones,
  so one converter reads both. A modifier this reader does not know ends its list, since its
  length is unknown. The comments are rewritten the same way; the pictures and the drawing layer,
  which have no Word 97 shape, are read on their own (`doc/word6/drawings`).
- **PowerPoint 97-2003 through its records.** The `PowerPoint Document` stream is walked linearly:
  stepping into a container is moving past its header, so no recursion is needed. Text comes from
  `TextCharsAtom` (UTF-16LE) and `TextBytesAtom` (the low bytes of UTF-16); masters and handouts
  are skipped. The walk follows stream order, so after an incremental save a slide may appear
  twice and notes may come after all the slides: good enough for search. Display reads the same
  stream through its persist directories instead, so each slide is the one the last save kept.
- **Old formats are converted, not written.** A doc and a ppt are read into the structures a docx
  and a pptx give and saved as docx and pptx; nothing writes the binary formats, so rendering,
  editing and saving know only the new ones.
- **Tables start at A1.** calamine's ranges start at the first used cell; the table is shifted so a
  row number is the one Excel shows. A formula with no computed value (as libraries such as openpyxl
  write it) widens the table and shows its text, `=A1+B1`, instead of an empty cell.
- **Delimited text is decoded like Excel writes it.** UTF-8 when valid, otherwise Windows-1252; a
  comma file whose first line holds more semicolons than commas is read with semicolons, the
  Italian export.
- **The document tree is neutral.** It names blocks and marks, not HTML or one editor's schema, so
  the caller maps it onto its own; its shape is in [document-tree.md](document-tree.md).
- **Looks stay out of the editor's JSON.** Spacing, fonts and table geometry are read for the
  renderer into `look` fields that `serde` skips, so the tree an editor receives keeps its shape
  and its size. A floating image's anchor is the exception: an editor shows it, so it crosses.
- **The renderer writes Typst source, never Typst code from input.** Document text enters the
  source only as string literals, so a crafted document cannot become markup.
- **Formulas are trees.** A formula is parsed once into an expression tree and printed back for
  storage; that is what lets row and column edits, sheet renames and pastes move references the
  way a spreadsheet does, and what makes the Italian syntax a reader concern only.
- **A sheet's drawing is edited in place.** Charts are made and changed in the drawing part the
  sheet carries, anchor by anchor and part by part, so what no editor describes is written back as
  the file had it; an ods's pictures and charts are built into the same kind of drawing when it is
  read.
- **No `unsafe`.** The workspace forbids it; parsing goes through crates that do the same or are
  widely fuzzed.
- **Tests carry no binaries.** Documents are built in memory in `test_support.rs` (zip writer,
  `rust_xlsxwriter`, `cfb` for the binary formats), so a reader sees exactly what each test feeds.

## The WebAssembly boundary

Three modules compile for `wasm32-unknown-unknown`, apart so a process loads only what it uses:
`rely_office_wasm` (reading and writing documents and slides, 3.6 MB), `rely_office_render_wasm`
(Typst, 31 MB, loaded on the first render) and `rely_office_sheet_wasm` (the spreadsheet, 3.7 MB,
loaded on the first sheet). `wasm-bindgen-cli` generates the Node bindings (`--target nodejs`, loaded
synchronously) for all three, and browser bindings (`--target web`) for the spreadsheet, the one
module an editor runs in the page. The boundary is JSON text, so the package needs no JavaScript
glue crate:

- each export takes the document as a `Uint8Array` (copied into the module's memory) and the
  limits as a JSON string, an object over the defaults whose unknown fields are refused
  (`INVALID_LIMITS`);
- each answers JSON that the wrapper (`packages/rely-office-node/index.js`) parses;
- a failure throws an `Error` whose message is `CODE: message`, and the wrapper rethrows it as a
  `RelyOfficeError` with that `code`;
- bytes never travel inside the JSON: a render answers a handle the wrapper moves the PDF and the
  PNGs out of, a docx writer takes its images and embedded objects one by one (`addMedia`,
  `addObject`), and `readDocumentTreeHandle` returns a handle holding the JSON, the image bytes and
  the object bytes, which the wrapper moves out each as its own `Uint8Array` (`takeMediaBytes`,
  `takeObjectBytes`) before it frees the handle;
- a spreadsheet is a handle that lives in the module's memory until `free`, asked for windows of
  cells and sent edits as calls.

The modules are built with the `wasm-release` profile (`opt-level = "s"`, fat LTO, one codegen
unit, symbols stripped), a profile of its own so native builds and tests keep the defaults. `wasm-bindgen-cli` must
match the `wasm-bindgen` crate exactly, which is why the crate is pinned with `=` and
`scripts/build-npm.mjs` checks the installed CLI against `Cargo.lock` before building. A panic in
the module would abort the instance, so the same rule as everywhere holds: malformed input is an
error, never a panic.

The npm package is published to GitHub Packages by `.github/workflows/release.yml` on a `v*` tag
that matches its version, with the workflow's own `GITHUB_TOKEN`, so no secret is needed. GitHub
Packages has no npm provenance and asks for a token even to install; the public npm registry is
tracked in the issues.

## Licence and attribution

MPL-2.0: file-level copyleft. Software of any licence may use the library; a change to one of its
files is published under MPL-2.0. Every source file carries the MPL header and the RelyBytes
copyright line; `NOTICE` holds the attribution and is shipped in the npm package with the licence.
Contributions come under the Developer Certificate of Origin, without a CLA, so the licence cannot
be changed behind contributors' backs.
