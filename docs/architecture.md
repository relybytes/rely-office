# Architecture

rely-office turns office documents into what a file manager needs from them: what they are, the
text to index, a table to show, a document tree to import into an editor, a PDF and a thumbnail;
it writes a docx back from that tree, and it is the model behind a spreadsheet editor.

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

What it deliberately does not do: collaboration, pixel-perfect Word layout (Typst lays pages out
its own way; [rendering.md](rendering.md) measures how close), a rich-text editor of its own, and
spreadsheet features past values, formulas, number formats and column widths.

## Crates and packages

| Crate or package | Role | Depends on |
| --- | --- | --- |
| `rely-office-core` | detection, text extraction, sheet tables and print settings, the document tree with its looks, the docx writer | `zip`, `quick-xml`, `calamine`, `cfb`, `encoding_rs`, `thiserror`; `serde` behind the `serde` feature |
| `rely-office-render` | documents written as Typst source and typeset to PDF and PNG, see [rendering.md](rendering.md) | `rely-office-core`, `typst`, `typst-layout`, `typst-pdf`, `typst-render`, `comemo` |
| `rely-office-sheet` | the editable spreadsheet: formulas, recalculation, edits, xlsx in and out, see [editing.md](editing.md) | `rely-office-core`, `calamine`, `quick-xml`, `zip`, `thiserror` |
| `rely-office-cli` | the `rely-office` command for trying files by hand | the three crates above |
| `rely-office-wasm`, `rely-office-render-wasm`, `rely-office-sheet-wasm` | the WebAssembly fronts for JavaScript, not published as crates | `wasm-bindgen`, `serde`, `serde_json` |
| `rely-office-corpus` | the page comparison with LibreOffice, not published | `rely-office-render`, `tiny-skia` |
| `@relybytes/rely-office` (npm, `packages/rely-office-node`) | the Node package and its browser entry: hand-written wrappers and types over the generated bindings, and the fonts | the three modules |

## Inside the core

| Module | Reads |
| --- | --- |
| `format` | the format, from the zip members or the compound file streams |
| `archive`, `compound` | zip members and compound file streams, each through a size cap |
| `relationships` | the `_rels/*.rels` parts of OOXML: targets resolved like OPC, the main part from `_rels/.rels` |
| `xml_text` | a streaming walk from XML to search text, configured per format by a `TextRules` |
| `xml_tree` | a small element tree with resolved namespaces, depth-capped, for structural reads |
| `word`, `presentation`, `opendocument` | the text of docx, pptx, odt and odp; `opendocument` also reads an odt into the document tree and an odp into slides, through its style resolver |
| `doc`, `ppt` | the text of the binary formats, a doc as the document tree and a ppt as slides |
| `office_art` | the drawing records doc and ppt share: shapes, their properties and the pictures of the blip store |
| `geometry` | shape outlines as paths: the `DrawingML` presets and custom geometries, `OpenDocument` enhanced geometries, polygons and SVG paths |
| `sheet` | spreadsheets as tables, through calamine, and delimited text |
| `sheet_layout`, `sheet_styles`, `number_format` | the print settings, cell styles and merges of xlsx sheets, read as a stream that skips the cells, and Excel's number format codes |
| `sheet_drawings` | the pictures and charts a sheet's drawing anchors to its cells, and the drawing parts an editor writes back unchanged |
| `document` | the docx document tree: styles, table styles, numbering, notes, headers and footers, sections, floating anchors, conversion, looks, media sniffing, and the docx writer |
| `slides`, `theme` | a pptx as slides over its layouts and masters, the pptx writer, and the colours and fonts of a `DrawingML` theme |
| `chart` | `DrawingML` and `OpenDocument` charts, read from their cached values or cells and drawn as SVG |

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
  tabs. Word 6 and 95 files, an encrypted file, or a piece table pointing outside its streams are
  refused with an error, never read past the end.
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
  and its size.
- **The renderer writes Typst source, never Typst code from input.** Document text enters the
  source only as string literals, so a crafted document cannot become markup.
- **Formulas are trees.** A formula is parsed once into an expression tree and printed back for
  storage; that is what lets row and column edits, sheet renames and pastes move references the
  way a spreadsheet does, and what makes the Italian syntax a reader concern only.
- **No `unsafe`.** The workspace forbids it; parsing goes through crates that do the same or are
  widely fuzzed.
- **Tests carry no binaries.** Documents are built in memory in `test_support.rs` (zip writer,
  `rust_xlsxwriter`, `cfb` for the binary formats), so a reader sees exactly what each test feeds.

## The WebAssembly boundary

Three modules compile for `wasm32-unknown-unknown`, apart so a process loads only what it uses:
`rely_office_wasm` (reading and docx writing, 1.2 MB), `rely_office_render_wasm` (Typst, 27 MB,
loaded on the first render) and `rely_office_sheet_wasm` (the spreadsheet, 1.2 MB, loaded on the
first sheet). `wasm-bindgen-cli` generates the Node bindings (`--target nodejs`, loaded
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
  PNGs out of, a docx writer takes its images one by one (`addMedia`), and `readDocumentTreeHandle` returns a
  handle holding the JSON and the image bytes, the wrapper moves each image out as its own
  `Uint8Array` (`takeMediaBytes`) and frees the handle;
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
