# @relybytes/rely-office

Reads office documents in Node without an office suite, through
[rely-office](https://github.com/relybytes/rely-office) compiled to WebAssembly: the format of a
file, its text for a search index, the sheets of a spreadsheet as tables, and a `.docx` as a
neutral document tree for import into a rich-text editor.

Everything runs in process and synchronously; there is no native addon to build and no external
program to call. Every read is bounded by limits, so an uploaded file cannot exhaust memory.

## Install

The package is published to GitHub Packages, which asks for a token even to install a public
package. Point the `@relybytes` scope at it and give it a token with `read:packages`:

```ini
# .npmrc
@relybytes:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=${NPM_GITHUB_TOKEN}
```

```sh
npm install @relybytes/rely-office
```

Node 18 or later. Publishing to the public npm registry is tracked in the issues.

## Use

```js
const { readFileSync } = require('node:fs');
const office = require('@relybytes/rely-office');

const bytes = readFileSync('contratto.docx');

office.detectFormat(bytes, 'contratto.docx');
// 'docx'

office.extractText(bytes, 'contratto.docx', { maxTextChars: 200000 });
// { format: 'docx', text: 'Contratto di fornitura\n...', truncated: false }

office.readSheets(readFileSync('vendite.xlsx'), 'vendite.xlsx');
// { format: 'xlsx', sheets: [{ name: 'Vendite', rows: [['Mese', 'Ricavi'], ...], totalRows: 3, totalColumns: 2 }] }

const tree = office.readDocumentTree(bytes);
// { blocks: [...], media: [{ index: 0, contentType: 'image/png', bytes: Uint8Array }], report: {...} }
```

A document is a `Uint8Array` (a Node `Buffer` is one), an `ArrayBuffer` or any typed array view.

| Function | Reads | Returns |
| --- | --- | --- |
| `detectFormat(bytes, fileName?)` | anything | `'docx' \| 'xlsx' \| 'pptx' \| 'odt' \| 'ods' \| 'odp' \| 'doc' \| 'xls' \| 'ppt' \| 'csv' \| 'tsv' \| 'unknown'` |
| `extractText(bytes, fileName?, limits?)` | every format above | `{ format, text, truncated }` |
| `readSheets(bytes, fileName?, limits?)` | xlsx, xls, ods, csv, tsv | `{ format, sheets: [{ name, rows, totalRows, totalColumns }] }` |
| `readDocumentTree(bytes, limits?)` | docx | `{ blocks, media, report }` |
| `version` | | the library version |

The bytes decide the format; the file name only tells CSV from TSV, which carry no signature.

### Limits

Every field is optional and keeps its default when left out; an unknown field is refused.

| Field | Default | Bounds |
| --- | --- | --- |
| `maxInputBytes` | 50 MiB | the input |
| `maxMemberBytes` | 128 MiB | each decompressed archive member or compound file stream |
| `maxTextChars` | 1,000,000 | the text returned (and the text of a document tree) |
| `maxRows`, `maxColumns` | 1,000, 60 | each sheet of `readSheets` |
| `maxImageBytes` | 10 MiB | each image of a document tree |
| `maxImages` | 200 | the images of a document tree |
| `maxTotalImageBytes` | 100 MiB | all the images of a document tree together |

### Errors

Every function but `detectFormat` throws a `RelyOfficeError` when a document cannot be read. Its
`code` is stable: `UNSUPPORTED_FORMAT`, `DAMAGED_CONTAINER`, `TOO_LARGE`, `MALFORMED_XML`,
`UNREADABLE_WORKBOOK`, `UNREADABLE_DOCUMENT`, `ENCRYPTED` (a password-protected doc or ppt),
`INVALID_LIMITS`.

```js
try {
  office.extractText(bytes, name);
} catch (error) {
  if (error instanceof office.RelyOfficeError && error.code === 'ENCRYPTED') {
    // ask for the unprotected file
  }
}
```

## The document tree

`readDocumentTree` returns blocks, not HTML, so the caller maps them onto its editor's schema.

- **Blocks**: `heading` (`level` 1 to 6), `paragraph`, `listItem` (`ordered`, `level` from 0,
  `listId`: consecutive items with the same id form one list), `table` (rows with `header`, cells
  with `colSpan`, `rowSpan` and their own blocks), `blockquote` (consecutive quote paragraphs),
  `codeBlock` (consecutive code paragraphs as lines of `text`), `horizontalRule`.
- **Inline content**: `text` with optional `marks` (`bold`, `italic`, `underline`, `strike`,
  `code`, `superscript`, `subscript`, `link` with `href`), `hardBreak`, and `image` with the
  `media` index, `alt`, `width` and `height` in CSS pixels.
- **Links** keep only `http:`, `https:` and `mailto:` targets; any other target is dropped (its
  text stays) and counted in the report.
- **Media** holds each embedded image once, with the type sniffed from its bytes. An SVG is
  returned as it is: sanitize it before serving it to a browser.
- **Report** counts what the tree leaves out or changes: `textBoxes`, `footnotes`, `endnotes`,
  `comments`, `headersAndFooters`, `fieldsKeptAsText`, `charts`, `smartArt`, `equations`,
  `shapes`, `embeddedObjects`, `unsupportedLinks`, `oversizedImages`, `imagesOverLimit`,
  `missingImages`, `trackedInsertions` (taken), `trackedDeletions` (skipped), `hiddenRuns`, and
  `truncated` when the text limit cut the document.

```json
{
  "blocks": [
    { "type": "heading", "level": 1, "content": [{ "type": "text", "text": "Contratto" }] },
    {
      "type": "paragraph",
      "content": [
        { "type": "text", "text": "Firmato da " },
        { "type": "text", "text": "Mario Rossi", "marks": [{ "type": "bold" }] },
        { "type": "hardBreak" },
        { "type": "text", "text": "il sito", "marks": [{ "type": "link", "href": "https://relybytes.com" }] },
        { "type": "image", "media": 0, "alt": "Firma", "width": 120, "height": 40 }
      ]
    },
    { "type": "listItem", "ordered": true, "level": 0, "listId": 3, "content": [{ "type": "text", "text": "Primo" }] }
  ],
  "media": [{ "index": 0, "contentType": "image/png", "bytes": "Uint8Array(5120)" }],
  "report": { "truncated": false, "comments": 2, "unsupportedLinks": 1, "...": 0 }
}
```

## Licence

Mozilla Public License 2.0, see `LICENSE` and `NOTICE` in the package. Built and maintained by
[RelyBytes Consulting](https://relybytes.com).
