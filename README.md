# rely-office

Office documents without an office suite, in safe Rust: format detection, text for search, sheets
as tables, a `.docx` as a document tree for a rich-text editor, rendering to PDF and PNG through
Typst, writing `.docx` back from that tree, and an editable spreadsheet with formulas that opens
and saves xlsx. It runs natively and, compiled to WebAssembly, in process from Node and in the
browser.

rely-office is built and maintained by [RelyBytes Consulting](https://relybytes.com) for the Drive
of the [Rely Platform](https://relyplatform.app), where it previews, indexes, edits and exports the
files a team stores, and is published under the Mozilla Public License 2.0 so that it stays open.

## What it does

| Format | Detect | Text for search | Sheets as tables | Document tree | Render | Write | Edit |
| --- | --- | --- | --- | --- | --- | --- | --- |
| docx | yes | body, headers, footers, notes | - | yes | layout | yes | through the tree |
| xlsx, xlsm | yes | every sheet, tab separated | yes, from A1 | - | layout, with its print settings | yes | yes |
| xls, ods | yes | every sheet | yes | - | layout | as xlsx | yes |
| csv, tsv | by name, text only | yes | yes, UTF-8 or Windows-1252, `;` or `,` | - | layout | as xlsx | yes |
| pptx, odt, odp | yes | slides with notes, body with headers and footers, pages with notes | - | - | text | - | - |
| doc, ppt (97-2003) | yes | every story, every text atom outside the masters | - | - | text | - | - |

The format comes from the bytes, never from the file name alone. Every input is bounded by
[`Limits`](crates/rely-office-core/src/limits.rs): the input size, each decompressed archive
member or stream (the zip bomb defence), the characters of text, the rows and columns of a table,
and the images of a document tree; the spreadsheet model caps sheets and cells, formulas their
length and nesting.

## Use it from Rust

```toml
[dependencies]
rely-office-core = "0.3"    # reading, the document tree, writing docx
rely-office-render = "0.3"  # PDF and PNG through Typst
rely-office-sheet = "0.3"   # the editable spreadsheet
```

```rust
use rely_office_core::{Limits, extract_text, read_document_tree, write_docx};
use rely_office_render::{RenderRequest, Renderer};
use rely_office_sheet::{CellRef, Locale, Workbook, write_xlsx};

let bytes = std::fs::read("contratto.docx")?;
let extracted = extract_text(&bytes, Some("contratto.docx"), &Limits::default())?;
let tree = read_document_tree(&bytes, &Limits::default())?;
let copy = write_docx(&tree)?;

let renderer = Renderer::new(font_files); // TrueType or OpenType files, read once
let rendered = renderer.render(&bytes, Some("contratto.docx"), &RenderRequest::default())?;

let mut workbook = Workbook::new();
workbook.set_input(0, CellRef::new(0, 0), "=SOMMA(B1:B9)", Locale::Italian)?;
let xlsx = write_xlsx(&workbook)?;
```

## Use it from Node and the browser

The npm package [`@relybytes/rely-office`](packages/rely-office-node) wraps the WebAssembly
builds: synchronous calls in Node 18 or later, no native addon; the spreadsheet also has a browser
entry, `@relybytes/rely-office/web`, for an editor.

```js
const office = require('@relybytes/rely-office');

office.extractText(bytes, 'contratto.docx');            // { format, text, truncated }
office.readDocumentTree(bytes);                         // { blocks, media, report }
office.render(bytes, 'contratto.docx', { thumbnailWidth: 320 }); // { pdf, thumbnail, ... }
office.writeDocx({ blocks, media });                    // Uint8Array
const sheet = office.openSheet(xlsx, 'conti.xlsx');     // edit, recalculate, save()
```

The package [README](packages/rely-office-node/README.md) has the whole API.

## From a terminal

```sh
cargo run -p rely-office-cli -- text contract.docx
cargo run -p rely-office-cli -- tree contract.docx
cargo run -p rely-office-cli -- pdf contract.docx       # writes contract.docx.pdf
cargo run -p rely-office-cli -- png sales.xlsx          # one PNG per page
cargo run -p rely-office-cli -- rewrite contract.docx   # through the tree and back
cargo run -p rely-office-cli -- rewrite sales.xlsx      # through the editable model and back
```

Renders use the fonts of the folder `RELY_OFFICE_FONTS` names, by default `./fonts`.

## Build and test

```sh
cargo test --workspace
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version "$(node scripts/build-npm.mjs --print-bindgen-version)" --locked
node scripts/build-npm.mjs                        # writes dist/relybytes-rely-office-<version>.tgz
node packages/rely-office-node/test/smoke.cjs     # every Node export
node packages/rely-office-node/test/smoke-web.mjs # the browser entry
SOFFICE=soffice node scripts/corpus.mjs           # pages compared with LibreOffice
```

## Layout

| Path | Purpose |
| --- | --- |
| [`crates/rely-office-core`](crates/rely-office-core) | detection, text, sheet tables and print settings, the document tree and its looks, the docx writer |
| [`crates/rely-office-render`](crates/rely-office-render) | documents to Typst source, typeset to PDF and PNG |
| [`crates/rely-office-sheet`](crates/rely-office-sheet) | the editable spreadsheet: formulas, recalculation, edits, xlsx in and out |
| [`crates/rely-office-cli`](crates/rely-office-cli) | the `rely-office` command |
| [`crates/rely-office-wasm`](crates/rely-office-wasm), [`-render-wasm`](crates/rely-office-render-wasm), [`-sheet-wasm`](crates/rely-office-sheet-wasm) | the WebAssembly fronts |
| [`crates/rely-office-corpus`](crates/rely-office-corpus) | the page comparison with LibreOffice |
| [`packages/rely-office-node`](packages/rely-office-node) | the npm package `@relybytes/rely-office` |
| [`corpus/`](corpus) and [`scripts/corpus.mjs`](scripts/corpus.mjs) | the Italian documents of the comparison |
| [`fonts/`](fonts) | the metric-compatible fonts the package ships, SIL Open Font License 1.1 |
| [`fuzz/`](fuzz) | cargo-fuzz targets |

The design is in [docs/](docs/README.md): [architecture](docs/architecture.md),
[rendering](docs/rendering.md), [editing](docs/editing.md), [the document tree](docs/document-tree.md)
and the [roadmap](docs/roadmap.md); the changes of each release in [CHANGELOG.md](CHANGELOG.md).

## Licence

Mozilla Public License 2.0, see [LICENSE](LICENSE). In short: you may use rely-office in any
software, open or closed; changes to the files of rely-office itself must be published under the
same licence, and the copyright notices of RelyBytes Consulting must be kept. See [NOTICE](NOTICE)
for the attribution and for the licences of the dependencies; the fonts in `fonts/` carry their own
OFL licence texts.

## Contributing

Contributions are welcome under the Developer Certificate of Origin: sign off every commit
(`git commit -s`). There is no contributor licence agreement, so the licence of a contribution is
the licence of the project. See [CONTRIBUTING.md](CONTRIBUTING.md). Security reports go through
the private channel in [SECURITY.md](SECURITY.md), never a public issue.
