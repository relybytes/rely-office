# The docx document tree

`read_document_tree` (Rust) and `readDocumentTree` (Node) read a `.docx` as a neutral tree for
import into a rich-text editor. It is not HTML and not any editor's schema: the caller maps each
block and mark onto its own. The JSON names below are those of the `serde` feature and of the npm
package; the Rust names are the same in snake case.

## Result

```text
{ blocks: Block[], media: MediaItem[], report: ImportReport }
```

## Blocks

| `type` | Fields | From |
| --- | --- | --- |
| `heading` | `level` 1 to 6, `content` | a `heading N` or `Title` style, else the outline level of the style or the paragraph; levels past 6 become 6 |
| `paragraph` | `content` | any other paragraph, an empty one included |
| `listItem` | `ordered`, `level` (0 outermost), `listId`, `content` | a paragraph with numbering, from the paragraph or its style; `numFmt` `bullet` is unordered, any other format ordered, `none` is no list |
| `table` | `rows`: `{ header, cells: { colSpan, rowSpan, blocks }[] }[]` | `w:tbl`; `tblHeader` marks a header row, `gridSpan` the columns, a `vMerge` continuation folds into the cell above |
| `blockquote` | `blocks` | quote styles; consecutive quote paragraphs share one blockquote |
| `codeBlock` | `text` | code styles; consecutive code paragraphs are the lines of one block |
| `horizontalRule` | | an empty paragraph with a VML rule, a bottom border, or a horizontal line style |

Lists are flat: an editor that nests lists builds the nesting from `level`, grouping consecutive
items with the same `listId`. A heading wins over list numbering, a list over a quote style.
Content controls, smart tags, custom XML and the choice of `mc:AlternateContent` are read through;
an empty paragraph holding only a page break is dropped.

Style names are matched in English, the names Word stores whatever the language of its interface,
and in Italian, the names LibreOffice writes from an Italian interface:

| Role | Name contains |
| --- | --- |
| quote | `quot`, `block text`, `citazione` |
| code | `code`, `preformatted`, `codice`, `preformattato` |
| horizontal rule | `horizontal line`, `linea orizzontale` |

The style chain (`w:basedOn`) is followed for up to 16 steps.

## Inline content

| `type` | Fields |
| --- | --- |
| `text` | `text`, `marks` (left out when empty) |
| `hardBreak` | a line break inside the block (`w:br` without a type, `w:cr`) |
| `image` | `media` (index into `media`), `alt`, `width`, `height` in CSS pixels (each left out when unknown) |

Adjacent runs with the same marks are merged. Tabs are `\t` in the text.

Marks, in this order: `bold`, `italic`, `underline`, `strike` (single or double), `code` (a
monospace font or a character style named like code or verbatim), `superscript`, `subscript`, and
`link` with `href`. A run's character style applies first and its own properties over it; the old
formatting a tracked change keeps is ignored. A link's underline is not repeated as a mark, nor is
the underline of a link that was dropped.

Links come from `w:hyperlink` (an external relationship, with its anchor appended) and from
`HYPERLINK` fields. Only `http:`, `https:` and `mailto:` targets are kept; anything else (internal
bookmarks, `javascript:`, `file:`, a missing relationship) keeps its text, loses the link and is
counted in `unsupportedLinks`.

Fields keep the text they last showed and drop their instructions; an instruction left without a
separator at the end of a paragraph is closed there, so a damaged field cannot swallow the rest of
the document.

## Media

`{ index, contentType, bytes }`, each image once however many times it is referenced. The type is
sniffed from the bytes: PNG, JPEG, GIF, BMP, TIFF, WebP, WMF, EMF, SVG, else
`application/octet-stream`. WMF and EMF are returned for the caller to convert or drop; an SVG is
returned as it is and must be sanitized before a browser shows it.

Images come from DrawingML pictures (`a:blip r:embed`) and from VML (`v:imagedata r:id`, sized by
the shape's `width` and `height` in `pt` or `px`). An image is returned only within
`maxImageBytes` each, `maxImages` in number and `maxTotalImageBytes` together.

## Report

Counts of what the tree leaves out or changes:

| Field | Counts |
| --- | --- |
| `truncated` | `true` when `maxTextChars` cut the document; the rest is left out |
| `textBoxes` | DrawingML and VML text boxes, left out |
| `footnotes`, `endnotes` | note references; the notes are left out |
| `comments` | comment references; comments are left out, the commented text stays |
| `headersAndFooters` | header and footer parts, left out |
| `fieldsKeptAsText` | fields kept as their result |
| `charts`, `smartArt`, `equations` | left out |
| `shapes` | shapes and groups without a picture, left out |
| `embeddedObjects` | OLE objects, left out |
| `unsupportedLinks` | links dropped by the target rule |
| `oversizedImages` | images past `maxImageBytes` |
| `imagesOverLimit` | images past `maxImages` or `maxTotalImageBytes` |
| `missingImages` | images whose part is missing or linked from outside the package |
| `trackedInsertions` | tracked insertions and moves in, accepted: their text is in the tree |
| `trackedDeletions` | tracked deletions, moves out and deleted rows, accepted: their text is gone |
| `hiddenRuns` | runs formatted as hidden, left out |

## Example

```json
{
  "blocks": [
    { "type": "heading", "level": 1, "content": [{ "type": "text", "text": "Contratto di fornitura" }] },
    {
      "type": "paragraph",
      "content": [
        { "type": "text", "text": "Il " },
        { "type": "text", "text": "fornitore", "marks": [{ "type": "bold" }] },
        { "type": "text", "text": " accetta, vedi " },
        { "type": "text", "text": "il sito", "marks": [{ "type": "link", "href": "https://relybytes.com/" }] },
        { "type": "hardBreak" },
        { "type": "image", "media": 0, "alt": "Logo", "width": 100, "height": 50 }
      ]
    },
    { "type": "listItem", "ordered": false, "level": 0, "listId": 1, "content": [{ "type": "text", "text": "Primo punto" }] },
    { "type": "listItem", "ordered": false, "level": 1, "listId": 1, "content": [{ "type": "text", "text": "Sotto punto" }] },
    {
      "type": "table",
      "rows": [
        {
          "header": true,
          "cells": [
            { "colSpan": 1, "rowSpan": 1, "blocks": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Mese" }] }] },
            { "colSpan": 1, "rowSpan": 1, "blocks": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Ricavi" }] }] }
          ]
        }
      ]
    },
    { "type": "blockquote", "blocks": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Una citazione" }] }] },
    { "type": "codeBlock", "text": "let a = 1;\nlet b = 2;" },
    { "type": "horizontalRule" }
  ],
  "media": [{ "index": 0, "contentType": "image/png", "bytes": "<Uint8Array>" }],
  "report": {
    "truncated": false, "textBoxes": 0, "footnotes": 0, "endnotes": 0, "comments": 1,
    "headersAndFooters": 1, "fieldsKeptAsText": 2, "charts": 0, "smartArt": 0, "equations": 0,
    "shapes": 0, "embeddedObjects": 0, "unsupportedLinks": 1, "oversizedImages": 0,
    "imagesOverLimit": 0, "missingImages": 0, "trackedInsertions": 0, "trackedDeletions": 0,
    "hiddenRuns": 0
  }
}
```

## Trying it

```sh
cargo run -p rely-office-cli -- tree contract.docx
```

prints the blocks, the media list and the report in Rust's debug form.
