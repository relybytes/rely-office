# rely-office

Reads office documents without an office suite: format detection, text for search, and sheets as
tables, in safe Rust. Rendering to PDF and thumbnails comes next (see the [roadmap](docs/roadmap.md)).

rely-office is built and maintained by [RelyBytes Consulting](https://relybytes.com) for the Drive
of the [Rely Platform](https://relyplatform.app), where it previews and indexes the files a team
stores, and is published under the Mozilla Public License 2.0 so that it stays open.

## What it does today

| Format | Detect | Text for search | Sheets as tables |
| --- | --- | --- | --- |
| docx | yes | body, headers, footers, notes | - |
| xlsx, xlsm | yes | every sheet, tab separated | yes, from A1 |
| xls | yes | every sheet | yes |
| ods | yes | every sheet | yes |
| csv, tsv | by name, text only | yes | yes, UTF-8 or Windows-1252, `;` or `,` |
| pptx, odt, odp, doc, ppt | yes | not yet | - |

The format comes from the bytes, never from the file name alone: a zip is read for its
`[Content_Types].xml` or `mimetype`, a compound file for its streams. Every input is bounded by
[`Limits`](crates/rely-office-core/src/limits.rs): the input size, each decompressed archive member
(the zip bomb defence), the characters of text and the rows and columns of a table.

## Use it

```toml
[dependencies]
rely-office-core = "0.1"
```

```rust
use rely_office_core::{Limits, extract_text, read_workbook};

let bytes = std::fs::read("report.xlsx")?;
let text = extract_text(&bytes, Some("report.xlsx"), &Limits::default())?;
let workbook = read_workbook(&bytes, Some("report.xlsx"), &Limits::default())?;
```

From a terminal:

```sh
cargo run -p rely-office-cli -- detect report.xlsx
cargo run -p rely-office-cli -- text contract.docx
cargo run -p rely-office-cli -- sheets export.csv
```

## Layout

| Crate | Purpose |
| --- | --- |
| [`rely-office-core`](crates/rely-office-core) | detection, text extraction, sheet tables |
| [`rely-office-cli`](crates/rely-office-cli) | the `rely-office` command |

The design and its decisions are in [docs/architecture.md](docs/architecture.md).

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
