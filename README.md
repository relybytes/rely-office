# rely-office

Reads office documents without an office suite: format detection, text for search, sheets as
tables, and a `.docx` as a document tree for import into a rich-text editor, in safe Rust. It runs
natively and, compiled to WebAssembly, in process from Node. Rendering to PDF and thumbnails comes
next (see the [roadmap](docs/roadmap.md)).

rely-office is built and maintained by [RelyBytes Consulting](https://relybytes.com) for the Drive
of the [Rely Platform](https://relyplatform.app), where it previews and indexes the files a team
stores, and is published under the Mozilla Public License 2.0 so that it stays open.

## What it does today

| Format | Detect | Text for search | Sheets as tables | Document tree |
| --- | --- | --- | --- | --- |
| docx | yes | body, headers, footers, notes | - | yes |
| xlsx, xlsm | yes | every sheet, tab separated | yes, from A1 | - |
| xls | yes | every sheet | yes | - |
| ods | yes | every sheet | yes | - |
| csv, tsv | by name, text only | yes | yes, UTF-8 or Windows-1252, `;` or `,` | - |
| pptx | yes | every slide in order, each followed by its notes | - | - |
| odt | yes | body, then the headers and footers of the page styles | - | - |
| odp | yes | every page in order, with its notes | - | - |
| doc (Word 97-2003) | yes | every story through the piece table | - | - |
| ppt (PowerPoint 97-2003) | yes | every text atom outside the masters | - | - |

The format comes from the bytes, never from the file name alone: a zip is read for its
`[Content_Types].xml` or `mimetype`, a compound file for the names of its streams. Every input is
bounded by [`Limits`](crates/rely-office-core/src/limits.rs): the input size, each decompressed
archive member or stream (the zip bomb defence), the characters of text, the rows and columns of a
table, and the size, number and total bytes of the images of a document tree.

## Use it from Rust

```toml
[dependencies]
rely-office-core = "0.2"
```

```rust
use rely_office_core::{Limits, extract_text, read_document_tree, read_workbook};

let bytes = std::fs::read("report.docx")?;
let limits = Limits::default();
let extracted = extract_text(&bytes, Some("report.docx"), &limits)?;
println!("{} ({:?}, cut: {})", extracted.text, extracted.format, extracted.truncated);
let tree = read_document_tree(&bytes, &limits)?;
let workbook = read_workbook(&std::fs::read("sales.xlsx")?, Some("sales.xlsx"), &limits)?;
```

The `serde` feature serializes the document tree and its import report.

## Use it from Node

The npm package [`@relybytes/rely-office`](packages/rely-office-node) wraps the WebAssembly build:
synchronous calls, no native addon, Node 18 or later.

```js
const office = require('@relybytes/rely-office');

office.detectFormat(bytes, 'contratto.docx');           // 'docx'
office.extractText(bytes, 'contratto.docx');            // { format, text, truncated }
office.readSheets(bytes, 'vendite.xlsx');               // { format, sheets: [...] }
office.readDocumentTree(bytes, { maxImages: 50 });      // { blocks, media, report }
```

Failures throw a `RelyOfficeError` with a stable `code` (`TOO_LARGE`, `ENCRYPTED`, ...). The
package [README](packages/rely-office-node/README.md) has the whole API and the limits, and
[docs/document-tree.md](docs/document-tree.md) the shape of the tree.

Build the package locally (needs the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` at the
version of the `wasm-bindgen` crate in `Cargo.lock`):

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version "$(node scripts/build-npm.mjs --print-bindgen-version)" --locked
node scripts/build-npm.mjs                       # writes dist/relybytes-rely-office-<version>.tgz
node packages/rely-office-node/test/smoke.cjs    # reads a docx built in memory through every export
```

## From a terminal

```sh
cargo run -p rely-office-cli -- detect report.xlsx
cargo run -p rely-office-cli -- text contract.docx
cargo run -p rely-office-cli -- sheets export.csv
cargo run -p rely-office-cli -- tree contract.docx
```

## Layout

| Path | Purpose |
| --- | --- |
| [`crates/rely-office-core`](crates/rely-office-core) | detection, text extraction, sheet tables, document tree |
| [`crates/rely-office-cli`](crates/rely-office-cli) | the `rely-office` command |
| [`crates/rely-office-wasm`](crates/rely-office-wasm) | the WebAssembly front, through wasm-bindgen |
| [`packages/rely-office-node`](packages/rely-office-node) | the npm package `@relybytes/rely-office` |
| [`scripts/build-npm.mjs`](scripts/build-npm.mjs) | builds the WebAssembly module and packs the npm package |

The design and its decisions are in [docs/architecture.md](docs/architecture.md); the changes of
each release in [CHANGELOG.md](CHANGELOG.md).

## Licence

Mozilla Public License 2.0, see [LICENSE](LICENSE). In short: you may use rely-office in any
software, open or closed; changes to the files of rely-office itself must be published under the
same licence, and the copyright notices of RelyBytes Consulting must be kept. See [NOTICE](NOTICE)
for the attribution and for the licences of the dependencies.

## Contributing

Contributions are welcome under the Developer Certificate of Origin: sign off every commit
(`git commit -s`). There is no contributor licence agreement, so the licence of a contribution is
the licence of the project. See [CONTRIBUTING.md](CONTRIBUTING.md). Security reports go through
the private channel in [SECURITY.md](SECURITY.md), never a public issue.
