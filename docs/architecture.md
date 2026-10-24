# Architecture

rely-office turns office documents into what a file manager needs from them: what they are, the
text to index, a table to show, and later a PDF and a thumbnail. It never edits a document.

## Why a library of our own

The Drive of the Rely Platform previewed office files through a LibreOffice container (Gotenberg)
or not at all. A library in Rust, called in process from the Drive service, removes the container,
reads untrusted uploads with a memory-safe parser, and gives one implementation of text extraction,
previews and thumbnails. It is published under MPL-2.0 so improvements to it stay public.

What it deliberately does not do: editing (spreadsheets are edited with IronCalc, documents with an
existing editor), collaboration, and pixel-perfect Word layout. The evaluation behind this scope
compared four options (adopt existing libraries, OnlyOffice, this partial build, a complete suite)
and chose the partial build.

## Crates

| Crate | Role | Depends on |
| --- | --- | --- |
| `rely-office-core` | detection, text extraction, sheet tables | `zip`, `quick-xml`, `calamine`, `cfb`, `encoding_rs`, `thiserror` |
| `rely-office-cli` | the `rely-office` command for trying files by hand | `rely-office-core` |
| `rely-office-render` (planned) | PDF and PNG thumbnails through Typst | `typst`, `typst-pdf`, `typst-render` |
| `rely-office-node` (planned) | Node addon for the Drive service | `napi`, built for linux-x64 musl |

## Decisions

- **The bytes decide the format.** A name can say `.docx` over an xlsx; only CSV and TSV, which
  carry no signature, are told apart by their extension, and only when the bytes look like text.
- **Every read is bounded.** `Limits` caps the input, each decompressed archive member (a member is
  read through `take(cap + 1)`, so a zip bomb stops at the cap), the characters of text and the rows
  and columns of a table. Nothing panics on malformed input; errors are `OfficeError`.
- **Tables start at A1.** calamine's ranges start at the first used cell; the table is shifted so a
  row number is the one Excel shows. A formula with no computed value (as libraries such as openpyxl
  write it) widens the table and shows its text, `=A1+B1`, instead of an empty cell.
- **Delimited text is decoded like Excel writes it.** UTF-8 when valid, otherwise Windows-1252; a
  comma file whose first line holds more semicolons than commas is read with semicolons, the
  Italian export.
- **No `unsafe`.** The workspace forbids it; parsing goes through crates that do the same or are
  widely fuzzed.
- **Tests carry no binaries.** Documents are built in memory in `test_support.rs` (zip writer,
  `rust_xlsxwriter`), so a reader sees exactly what each test feeds.

## Licence and attribution

MPL-2.0: file-level copyleft. Software of any licence may use the library; a change to one of its
files is published under MPL-2.0. Every source file carries the MPL header and the RelyBytes
copyright line; `NOTICE` holds the attribution. Contributions come under the Developer Certificate
of Origin, without a CLA, so the licence cannot be changed behind contributors' backs.
