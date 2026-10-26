# Architecture

rely-office turns office documents into what a file manager needs from them: what they are, the
text to index, a table to show, a document tree to import into an editor, and later a PDF and a
thumbnail. It never edits a document.

## Why a library of our own

The Drive of the Rely Platform previewed office files through a LibreOffice container (Gotenberg)
or not at all, and imported Word files into its editor with a JavaScript converter. A library in
Rust, called in process from the Drive service, reads untrusted uploads with a memory-safe parser
and gives one implementation of text extraction, sheet tables, the import tree, and later previews
and thumbnails. It is published under MPL-2.0 so improvements to it stay public.

What it deliberately does not do: editing (spreadsheets are edited with IronCalc, documents with an
existing editor), collaboration, and pixel-perfect Word layout. The evaluation behind this scope
compared four options (adopt existing libraries, OnlyOffice, this partial build, a complete suite)
and chose the partial build.

## Crates and packages

| Crate or package | Role | Depends on |
| --- | --- | --- |
| `rely-office-core` | detection, text extraction, sheet tables, document tree | `zip`, `quick-xml`, `calamine`, `cfb`, `encoding_rs`, `thiserror`; `serde` behind the `serde` feature |
| `rely-office-cli` | the `rely-office` command for trying files by hand | `rely-office-core` |
| `rely-office-wasm` | the WebAssembly front for JavaScript, not published as a crate | `rely-office-core` (with `serde`), `wasm-bindgen`, `serde`, `serde_json` |
| `@relybytes/rely-office` (npm, `packages/rely-office-node`) | the Node package: a hand-written wrapper and types over the generated bindings | the module `rely-office-wasm` builds |
| `rely-office-render` (planned) | PDF and PNG thumbnails through Typst | `typst`, `typst-pdf`, `typst-render` |

## Inside the core

| Module | Reads |
| --- | --- |
| `format` | the format, from the zip members or the compound file streams |
| `archive`, `compound` | zip members and compound file streams, each through a size cap |
| `relationships` | the `_rels/*.rels` parts of OOXML: targets resolved like OPC, the main part from `_rels/.rels` |
| `xml_text` | a streaming walk from XML to search text, configured per format by a `TextRules` |
| `xml_tree` | a small element tree with resolved namespaces, depth-capped, for structural reads |
| `word`, `presentation`, `opendocument` | the text of docx, pptx, odt and odp |
| `doc`, `ppt` | the text of the binary formats |
| `sheet` | spreadsheets as tables, through calamine, and delimited text |
| `document` | the docx document tree: styles, numbering, conversion, media sniffing |

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
  twice and notes may come after all the slides: good enough for search, not for display.
- **Tables start at A1.** calamine's ranges start at the first used cell; the table is shifted so a
  row number is the one Excel shows. A formula with no computed value (as libraries such as openpyxl
  write it) widens the table and shows its text, `=A1+B1`, instead of an empty cell.
- **Delimited text is decoded like Excel writes it.** UTF-8 when valid, otherwise Windows-1252; a
  comma file whose first line holds more semicolons than commas is read with semicolons, the
  Italian export.
- **The document tree is neutral.** It names blocks and marks, not HTML or one editor's schema, so
  the caller maps it onto its own; its shape is in [document-tree.md](document-tree.md).
- **No `unsafe`.** The workspace forbids it; parsing goes through crates that do the same or are
  widely fuzzed.
- **Tests carry no binaries.** Documents are built in memory in `test_support.rs` (zip writer,
  `rust_xlsxwriter`, `cfb` for the binary formats), so a reader sees exactly what each test feeds.

## The WebAssembly boundary

`rely-office-wasm` compiles for `wasm32-unknown-unknown` and `wasm-bindgen-cli` generates the Node
bindings (`--target nodejs`, loaded synchronously). The boundary is JSON text, so the package needs
no JavaScript glue crate:

- each export takes the document as a `Uint8Array` (copied into the module's memory) and the
  limits as a JSON string, an object over the defaults whose unknown fields are refused
  (`INVALID_LIMITS`);
- each answers JSON that the wrapper (`packages/rely-office-node/index.js`) parses;
- a failure throws an `Error` whose message is `CODE: message`, and the wrapper rethrows it as a
  `RelyOfficeError` with that `code`;
- the images of a document tree never travel inside the JSON: `readDocumentTreeHandle` returns a
  handle holding the JSON and the image bytes, the wrapper moves each image out as its own
  `Uint8Array` (`takeMediaBytes`) and frees the handle.

The module is built with the `wasm-release` profile (`opt-level = "s"`, fat LTO, one codegen
unit), a profile of its own so native builds and tests keep the defaults. `wasm-bindgen-cli` must
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
