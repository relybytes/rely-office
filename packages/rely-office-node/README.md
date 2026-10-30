# @relybytes/rely-office

Office documents on the backend, in Node, without an office suite, through
[rely-office](https://github.com/relybytes/rely-office) compiled to WebAssembly: the format of a
file, its text for a search index, the sheets of a spreadsheet as tables, a `.docx`, `.doc`,
`.odt`, Markdown or HTML file as a neutral document tree for a rich-text editor and back as docx,
odt, Markdown or HTML, a presentation as slides and back as pptx or odp, PDF and PNG pages through
Typst, and an editable spreadsheet with Excel's function library, names, comments, charts,
conditional formats and validation that saves xlsx, ods and csv.

Everything runs in process; there is no native addon to build and no external program to call.
Every read is bounded by limits, so an uploaded file cannot exhaust memory.

For the browser, the same reading and writing and the editor components (a document editor, a
spreadsheet grid and a slide editor in React) are the frontend package,
[`@relybytes/rely-office-react`](../rely-office-react/README.md). Both declare the same model.

## Install

```sh
npm install @relybytes/rely-office
```

Node 18 or later. Every release goes to two registries with the same tarball: the public npm
registry, where a plain `npm install` needs no configuration, and GitHub Packages. Until a
release reaches the public registry, or to read it from GitHub Packages, point the `@relybytes`
scope there with a token holding `read:packages`, which GitHub asks for even to install a public
package:

```ini
# .npmrc
@relybytes:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=${NPM_GITHUB_TOKEN}
```

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
| `readDocumentTree(bytes, limits?, options?, fileName?)` | docx, doc (Word 97-2003, 6 and 95), odt; Markdown and HTML given their name | `{ blocks, media, report, notes, headersFooters, page, sections?, comments?, objects? }` |
| `readPresentation(bytes, limits?)` | pptx, odp, ppt | `{ widthEmu, heightEmu, slides, media, truncated }` |
| `render(bytes, fileName?, options?)` | docx, doc, odt, xlsx, xls, ods, csv, tsv, pptx, ppt, odp (layout); a doc or ppt that cannot be read (text) | `{ format, fidelity, pageCount, truncated, pdf, thumbnail, pageImages }` |
| `writeDocx({ blocks, media?, page?, sections?, comments?, objects? })` | a document tree | the docx bytes |
| `writeOdt(tree)`, `writeMarkdown(tree)`, `writeHtml(tree, title?)` | a document tree | the odt bytes, the Markdown or the HTML page as UTF-8 bytes |
| `writePptx({ widthEmu, heightEmu, slides, media? })`, `writeOdp(...)` | slides | the pptx or the odp bytes |
| `fontStack(family, generic?)`, `fontSubstitute(family, generic?)` | a font family | a CSS stack ending with the bundled family of the same metrics, or that family |
| `convertToDocx(bytes, limits?)` | doc, odt, docx | the docx bytes |
| `convertToPptx(bytes, limits?)` | ppt, odp, pptx | the pptx bytes |
| `createSheet(options?)`, `openSheet(bytes, fileName?, options?)` | xlsx, xls, ods, csv, tsv | a `SheetDocument` |
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

## Rendering

```js
const rendered = office.render(bytes, 'contratto.docx', {
  pdf: true,                                     // default
  thumbnailWidth: 320,                           // a PNG of the first page, at most 2,000 pixels
  pageImages: { pixelsPerPoint: 1.5, maxPages: 3 },
  limits: { maxTextChars: 200000 },              // the default of a render
});
// { format: 'docx', fidelity: 'layout', pageCount: 2, truncated: false,
//   pdf: Uint8Array, thumbnail: Uint8Array, pageImages: [Uint8Array, ...] }
```

The typesetter, 31 MB of WebAssembly, loads on the first call with the fonts the package ships
(Carlito, Caladea and the Liberation fonts, matching Calibri, Cambria, Arial, Times New Roman and
Courier New, and DejaVu Math TeX Gyre for equations). `fidelity` is `text` for the formats set as their text on A4. Errors add
`INVALID_OPTIONS`, `TYPESET_FAILED` and `EXPORT_FAILED` to the codes above.

## Writing a docx

```js
const tree = office.readDocumentTree(bytes);
const docx = office.writeDocx({
  blocks: tree.blocks,
  media: tree.media.map((item) => item.bytes),   // image.media indexes this list
  page: { widthTwips: 11906, heightTwips: 16838, marginsTwips: [1417, 1417, 1417, 1417] },
});
```

`readDocumentTree` reads the written file back to the same blocks. A tree that is not one throws
`INVALID_TREE`. Pass `comments` and `objects` as `readDocumentTree` gave them to keep the
comments and the embedded objects; read with `{ trackedChanges: 'keep' }` to keep tracked
changes as marks, which `writeDocx` writes back as changes:

```js
const tree = office.readDocumentTree(bytes, undefined, { trackedChanges: 'keep' });
const docx = office.writeDocx({
  blocks: tree.blocks,
  media: tree.media.map((item) => item.bytes),
  sections: tree.sections,
  comments: tree.comments,
  objects: tree.objects,                         // each { media, program?, name, bytes }
});
```

## Presentations

```js
const presentation = office.readPresentation(readFileSync('riunione.pptx'));
presentation.slides[0].notes;                    // the speaker notes, a text body
presentation.slides[0].transition;               // { effect: 'fade', durationMs: 500, ... }
const pptx = office.writePptx(presentation);     // media can stay as readPresentation gave it
```

A slide holds its `background`, its `shapes` in drawing order (each with its `id`, `frame`,
`rotation`, `geometry`, `fill`, `line` and `content`: text, a picture or a table), its `notes`,
whether it is `hidden`, its `transition` and its `animations` (each naming its shape by index, its
class, PowerPoint's preset number, its trigger, delay and duration). A shape drawn from the
layout or the master is resolved onto the slide and marked `inherited`. A pptx, an odp or a ppt
reads the same way; `writePptx` writes a pptx and `writeOdp` an odp.

## Spreadsheets

```js
const sheet = office.openSheet(readFileSync('conti.xlsx'), 'conti.xlsx', { locale: 'it' });
try {
  sheet.setInput(0, 9, 1, '=SOMMA(B1:B9)');          // stored as =SUM(B1:B9)
  sheet.setFormat(0, 9, 1, { type: 'decimals', decimals: 2 });
  sheet.insertRows(0, 0, 1);                         // formulas follow the cells
  const header = { top: 0, left: 0, bottom: 0, right: 3 };
  sheet.setStyle(0, header, { bold: true, fill: 'D9E2F3', horizontal: 'center' });
  sheet.setBorders(0, header, 'outline', { style: 'single', widthEighths: 4 });
  sheet.mergeCells(0, { top: 12, left: 0, bottom: 12, right: 3 });
  sheet.cells(0, { top: 0, left: 0, bottom: 40, right: 10 });
  // [{ row, column, input: '=SUM(B2:B10)', display: '1.234,50', kind: 'number', format }, ...]
  writeFileSync('conti.xlsx', sheet.save());
} finally {
  sheet.free();
}
```

Rows, columns and sheets are zero-based. The locale (`it`, the default, or `en`) decides how
numbers are typed and shown. `TODAY()` and `NOW()` read the date set with `setToday`. Failures
throw `INVALID_FORMULA`, `NO_SUCH_SHEET`, `DUPLICATE_SHEET_NAME`, `INVALID_SHEET_NAME`,
`LAST_SHEET`, `OUT_OF_BOUNDS`, `INVALID_INPUT`, `INVALID_LOCALE`, `INVALID_STYLE`, `INVALID_NAME`
or `INVALID_CHART`. Each cell of
`cells` carries its `style`; `stylesJson` and `mergesJson` give the styles and merged ranges a grid
draws, `clearStyles`, `unmergeCells` and `setRowHeight` complete the set. Saving keeps the styles,
merges and row heights, and writes the pictures, charts and pivot tables of the file back as they
were.

`conditionalFormats` and `setConditionalFormats` read and replace the conditional formats of a
sheet, `validations` and `setValidations` its data validations; `conditionalLooks` gives the fill,
font, data bar or icon each cell takes from the current values, `validation(sheet, row, column)`
what a cell accepts and the items of its list, and `invalidCells` the cells their validation
refuses. `input(sheet, row, column)` gives what a cell holds as the document's locale writes it, so in
Italian `=SOMMA(A1;0,5)`, and it reads back the same through `setInput`. `undo()` and `redo()`
go back and forth over the edits, up to 100 of them, with `canUndo()` and `canRedo()` for the
buttons.

`macros()` says what became of the macros of the file read: `kept` for an xlsm, which `save`
writes back as an xlsm, `dropped` for an xls, `none` otherwise. They are never run; a docm and a
pptm keep theirs the same way, as the `macroProject` that `readDocumentTree` and
`readPresentation` give and `writeDocx` and `writePptx` write back.

`definedNames()`, `setDefinedName(name, formula, sheet?)` and `removeDefinedName(name, sheet?)`
manage the names formulas read, a `LAMBDA` among them; `comments(sheet)` and
`setComment(sheet, row, column, text, author?)` the comments of the cells, and
`startCommentThread(sheet, row, column, text, author?, date?)`, `replyToComment(...)`,
`setCommentResolved(sheet, row, column, resolved)` and `removeCommentReply(sheet, row, column,
index)` the threads of Excel 365, each comment carrying `threaded`, `resolved`, `date` and its
`replies`. `charts(sheet)` lists
the charts with where they sit and, when they can be changed, what they show;
`chartForRange(sheet, window, 'column')` suggests the chart of a selection, which `addChart(sheet,
chart, placement)` adds, `setChart(sheet, index, { chart?, placement? })` changes or moves and
`removeChart(sheet, index)` removes:

```js
const chart = sheet.chartForRange(0, { top: 0, left: 0, bottom: 12, right: 2 }, 'line');
sheet.addChart(0, { ...chart, title: 'Vendite' }, { row: 1, column: 5, width: 4389120, height: 2651760 });
```

`pivotTables(sheet)` lists the pivot tables of a sheet, `pivotSourceFields(sheet, window)` the
fields of a range with their items, and `addPivotTable(sheet, definition)`, `setPivotTable`,
`refreshPivotTable`, `refreshPivotTables` and `removePivotTable` make, change, compute again and
remove them:

```js
sheet.addPivotTable(0, {
  sourceSheet: 0,
  source: { top: 0, left: 0, bottom: 120, right: 3 },
  anchor: { row: 0, column: 6 },
  rows: [{ field: 0 }],
  columns: [{ field: 2 }],
  values: [{ field: 3, function: 'sum' }],
  layout: 'tabular', // or 'compact', the default, or 'outline'
  subtotals: 'bottom', // or 'top', the default, or 'none'
  repeatLabels: true,
});

sheet.addPivotTable(0, {
  sourceSheet: 0,
  source: { top: 0, left: 0, bottom: 120, right: 3 },
  anchor: { row: 0, column: 12 },
  rows: [{ field: 0 }],
  values: [{ field: 3, function: 'sum' }, { field: 3, function: 'count' }],
  valuesAxis: 'rows', // Excel's Σ Values among the row fields; 'columns' is the default
  valuesPosition: 0, // the outermost; after the last row field when left out
});
```

A cell whose records hold only blanks for its field stays empty, as in Excel.

`save()` writes an xlsx, `saveOds()` an ods, its pivot tables as data pilot tables, and
`saveCsv(sheet, separator?)` one sheet as csv, by default with the separator of the csv it was
read from (`delimiter()`) or of the locale. The formulas, functions, rules and edits are described in
[docs/editing.md](https://github.com/relybytes/rely-office/blob/main/docs/editing.md).

## The document tree

`readDocumentTree` returns blocks, not HTML, so the caller maps them onto its editor's schema. A
docx, a doc and an odt give the same tree; `writeDocx` writes it back as a docx.

- **Blocks**: `heading` (`level` 1 to 6, `align`), `paragraph` (`align`: `left`, `center`, `right`,
  `justify`, absent when unset), `listItem` (`ordered`, `level` from 0,
  `listId`: consecutive items with the same id form one list), `table` (rows with `header`, cells
  with `colSpan`, `rowSpan` and their own blocks), `blockquote` (consecutive quote paragraphs),
  `codeBlock` (consecutive code paragraphs as lines of `text`), `horizontalRule`.
- **Inline content**: `text` with optional `marks` (`bold`, `italic`, `underline`, `strike`,
  `code`, `superscript`, `subscript`, `textStyle` with `fontFamily`, `fontSize` and `color`,
  `highlight` with `color`, `link` with `href`, `comment` with the index of its comment, and with
  `trackedChanges: 'keep'` `insertion` and `deletion` with `author` and `date`), `hardBreak`,
  `image` with the `media` index, `alt`, `width` and `height` in CSS pixels (a chart or a SmartArt
  is an SVG image), `math` with its `latex` and whether it is on a line of its own (`display`),
  `noteReference` with the index of its note in `notes`, and `commentAnchor` for a comment on a
  point.
- **Comments**: `comments`, each with its `author`, `initials`, `date`, `blocks`, the `parent` it
  answers and whether it is `resolved`.
- **Embedded objects**: `objects`, each the `media` index of the image that shows it, its
  `program` (the ProgID, `Excel.Sheet.12`), its `name`, whether it is an Office `package`, and its
  `bytes`.
- **Around the body**: `notes` (footnotes and endnotes), `headersFooters` (with the first-page
  and even-page variants) and `page` (size and margins in twentieths of a point).
- **Links** keep only `http:`, `https:` and `mailto:` targets; any other target is dropped (its
  text stays) and counted in the report.
- **Media** holds each embedded image once, with the type sniffed from its bytes. An SVG is
  returned as it is: sanitize it before serving it to a browser.
- **Report** counts what the tree holds, leaves out or changes: `textBoxes`, `footnotes`,
  `endnotes`, `comments`, `headersAndFooters`, `fieldsKeptAsText`, `charts`, `smartArt`,
  `equations`, `shapes`, `embeddedObjects`, `unsupportedLinks`, `oversizedImages`,
  `imagesOverLimit`, `missingImages`, `trackedInsertions`, `trackedDeletions`, `hiddenRuns`, and
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
[RelyBytes S.r.l.](https://relybytes.com).
