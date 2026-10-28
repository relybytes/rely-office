# The document tree

`read_document_tree` (Rust) and `readDocumentTree` (Node) read a `.docx`, a `.doc` (Word 97-2003,
and Word 6 and 95) or an `.odt`
as a neutral tree for import into a rich-text editor. It is not HTML and not any editor's schema:
the caller maps each block and mark onto its own. The JSON names below are those of the `serde`
feature and of the npm package; the Rust names are the same in snake case. The sections below
describe the docx; [the odt](#the-odt) and [the doc](#the-doc) give the same tree from their own
structures.

## Result

```text
{
  blocks: Block[], media: MediaItem[], report: ImportReport,
  notes: Note[], headersFooters: HeadersFooters, page: PageSize, sections?: Section[],
  comments?: Comment[], objects?: EmbeddedObject[]
}
```

- `notes`: the footnotes and endnotes, `{ kind: "footnote" | "endnote", blocks }`, in the order
  the text refers to them; a `noteReference` inline points at one by index.
- `headersFooters`: `{ header, footer, firstHeader?, firstFooter?, evenHeader?, evenFooter? }`,
  each a list of blocks, of the last section. The first-page pair is present only when the
  section has a different first page, the even pair only when odd and even pages differ.
- `page`: `{ widthTwips, heightTwips, marginsTwips }`, the margins top, right, bottom, left, in
  twentieths of a point, of the last section.
- `sections`: present only when the document has more than one section, each
  `{ endBlock, page, headersFooters, continuous }`: the block it ends before, its own page and
  headers, and whether it goes on on the page the section before ends on. A section that names no
  header or footer of a kind keeps the one of the section before, as Word does; the tree carries
  each section's resolved parts. `writeDocx` writes the sections back.
- `comments`: present when the document has any, each `{ author, initials?, date?, blocks,
  parent?, resolved }`: `date` as the file stores it (ISO 8601), `parent` the index of the
  comment it answers, `resolved` whether the discussion is done. The text a comment is on carries
  a `comment` mark; a comment on a point is a `commentAnchor` inline. `writeDocx` writes them
  back, with the replies and done states of `commentsExtended`.
- `objects`: present when the document embeds any, each `{ media, program?, name, package,
  bytes }`: an OLE object or an Office file inside the document, shown by the image whose `media`
  it names (an `image` inline with that index is the object). `program` is its ProgID
  (`Excel.Sheet.12`), `package` says whether `bytes` is an Office package (xlsx, docx, pptx)
  rather than a compound file holding an OLE object. `writeDocx` writes each back under
  `word/embeddings`, where Word and LibreOffice open it as the object again.

## Blocks

| `type` | Fields | From |
| --- | --- | --- |
| `heading` | `level` 1 to 6, `content`, `align` | a `heading N` or `Title` style, else the outline level of the style or the paragraph; levels past 6 become 6 |
| `paragraph` | `content`, `align` | any other paragraph, an empty one included |
| `listItem` | `ordered`, `level` (0 outermost), `listId`, `content` | a paragraph with numbering, from the paragraph or its style; `numFmt` `bullet` is unordered, any other format ordered, `none` is no list |
| `table` | `rows`: `{ header, cells: { colSpan, rowSpan, blocks }[] }[]` | `w:tbl`; `tblHeader` marks a header row, `gridSpan` the columns, a `vMerge` continuation folds into the cell above |
| `blockquote` | `blocks` | quote styles; consecutive quote paragraphs share one blockquote |
| `codeBlock` | `text` | code styles; consecutive code paragraphs are the lines of one block |
| `horizontalRule` | | an empty paragraph with a VML rule, a bottom border, or a horizontal line style |

`align` is `left`, `center`, `right` or `justify`, from the paragraph or its style, and absent
when neither sets one.

In Rust, paragraphs, headings, list items, tables and text runs also carry a `look`: spacing,
line spacing, indents, page breaks, fonts, sizes and colours, column widths, cell margins and
borders, and the tree has a `layout` with the page size, margins and body font. A floating image
carries its `anchor` (what its position is measured from, its offset or alignment, its size and
how the text goes around it, a tight wrap with its outline), and the blocks of a floating text box
a `frame` with the same anchor, the box's fill and outline and the preset shape it is drawn as.
The renderer lays
the document out from them; they are not serialized, since an editor has its own styles. The tree
also deserializes from the same JSON, which is how `writeDocx` reads it.

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
| `noteReference` | `note`, the index of the note in `notes`; the number it shows is its position among the notes of its kind |
| `math` | `latex`, `display`: an equation as LaTeX, in the line or, with `display`, on a line of its own |
| `commentAnchor` | `comment`, the index of a comment on a point rather than on text |

Adjacent runs with the same marks are merged. Tabs are `\t` in the text. A page number or a page
count field is text whose look says which (`PAGE`, `NUMPAGES`, `text:page-number`), so a
renderer can draw the page it lands on.

Marks, in this order: `bold`, `italic`, `underline`, `strike` (single or double), `code` (a
monospace font or a character style named like code or verbatim), `superscript`, `subscript`,
`textStyle` with the `fontFamily`, `fontSize` (`"14pt"`) and `color` (`"#1F3864"`) the run sets
itself (the paragraph's own font is not repeated), `highlight` with its `color`, `link` with
`href`, `comment` with the index of the comment the text is under, and, only when changes are
kept, `insertion` and `deletion` with the `author` and `date` of the change. A run's character
style applies first and its own properties over it; the old formatting a tracked change keeps is
ignored. A link's underline is not repeated as a mark, nor is the underline of a link that was
dropped.

Tracked changes are accepted by default: inserted text is in the tree, deleted text is not.
`read_document_tree_with(bytes, limits, &TreeOptions { tracked_changes: TrackedChanges::Keep })`,
or `readDocumentTree(bytes, limits, { trackedChanges: 'keep' })`, keeps both, each run marked with
its change, from `w:ins` and `w:del`, the tracked-changes regions of an odt and the revision marks
and reviewer table of a doc; `writeDocx` writes them back as `w:ins` and `w:del`.

Equations are read as LaTeX, the language an editor's formula field takes: from the Office Math
of a docx and from the MathML of an odt formula object, read before the picture that stands in for
it. One formula tree sits under the conversions, so a formula read, edited and written back stays
the same; `writeDocx` writes Office Math.

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
`application/octet-stream`. WMF and EMF are returned as they are: the renderer draws them, and in
Rust `convert_metafile_to_svg` turns one into SVG for a caller that shows it elsewhere. An SVG is
returned as it is and must be sanitized before a browser shows it.

Images come from DrawingML pictures (`a:blip r:embed`) and from VML (`v:imagedata r:id`, sized by
the shape's `width` and `height` in `pt` or `px`). A chart, a surface chart and the Office 2016
kinds included, becomes an `image/svg+xml` image drawn from the values the chart stores, with its
title as `alt`; written back, it stays a picture. A SmartArt becomes an `image/svg+xml` image of
the drawing Office keeps of it. An OLE object (`w:object`) is the image of its replacement picture,
and its object goes to `objects`. A drawn shape or group with no text box of its own (a docx `wps` shape or `wpg` group, an odt
rectangle, ellipse, line, polygon, path or custom shape, a doc's OfficeArt shape) becomes an
`image/svg+xml` image of its outline, fill, line and text, anchored where the shape floats. An
image is returned only within `maxImageBytes` each, `maxImages` in number and
`maxTotalImageBytes` together.

## Report

Counts of what the tree leaves out or changes:

| Field | Counts |
| --- | --- |
| `truncated` | `true` when `maxTextChars` cut the document; the rest is left out |
| `textBoxes` | DrawingML and VML text boxes; their content follows the paragraph that anchors them, an editor getting it in the flow |
| `footnotes`, `endnotes` | note references; the notes are in `notes` |
| `comments` | comment references; the comments are in `comments` |
| `headersAndFooters` | header and footer parts; their content is in `headersFooters` |
| `fieldsKeptAsText` | fields kept as their result |
| `charts` | charts, drawn as SVG images when their values can be read |
| `smartArt` | SmartArt, drawn as SVG images when Office kept their drawing |
| `equations` | equations, read as `math` inlines |
| `shapes` | shapes and groups that draw nothing (no size, no fill and no line, or a geometry this reader does not know), left out |
| `embeddedObjects` | OLE objects, shown as their picture and kept in `objects` when the object itself is in the file |
| `unsupportedLinks` | links dropped by the target rule |
| `oversizedImages` | images past `maxImageBytes` |
| `imagesOverLimit` | images past `maxImages` or `maxTotalImageBytes` |
| `missingImages` | images whose part is missing or linked from outside the package |
| `trackedInsertions` | tracked insertions and moves in: accepted, their text is in the tree, or kept, marked `insertion` |
| `trackedDeletions` | tracked deletions, moves out and deleted rows: accepted, their text is gone, or kept, marked `deletion` |
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

## The odt

An odt gives the same tree. Its styles are resolved through their parents to the family's
default style, the automatic styles of `content.xml` applying to the body and those of
`styles.xml` to the headers and footers.

| Block | From |
| --- | --- |
| `heading` | `text:h`, its `text:outline-level` (past 6 becomes 6), labelled by the outline numbering when it numbers |
| `paragraph` | `text:p`, its text collapsed as `OpenDocument` lays it out: a run of spaces is one, `text:s` keeps its count |
| `listItem` | a paragraph of `text:list`, numbered by the list style of the list or of its paragraph's style, each level in its own format; `text:continue-numbering` carries the count on, `text:list-header` has no label |
| `table` | `table:table` with its header rows, spans, column widths, the padding of its first cell as the cell margins, and each cell's fill, borders and vertical alignment; repeated rows and cells are repeated, a run of empty repeated rows is kept once |
| `blockquote`, `codeBlock` | the `Quotations` and `Preformatted Text` styles and their Italian names |

Notes are `text:note`; images `draw:image` in a `draw:frame`; a text box's paragraphs follow the
one that anchors it, and a custom shape whose enhanced geometry names a preset is a text box of
that shape; a chart object (`draw:object`) is drawn as an SVG image from the table it carries, a
formula object is read as its MathML, and a `draw:object-ole` is its picture with the compound
file kept in `objects`; comments are `office:annotation`, their ranges closed by
`office:annotation-end`; sections and indexes are read through. The page, columns, headers and footers come from
the master page of the first paragraph's style, else `Standard`: a header's height and spacing
are added to the top margin, as the page lays it out.

## The doc

A Word 97-2003 file is read from its own structures ([MS-DOC]): the File Information Block, the
piece table that says where each run of characters lies, the formatted disk pages that give each
run and paragraph its modifiers, the style sheet they apply over, the font table, the lists and
their overrides, and the section table. Character properties toggle against their style as Word
toggles them.

| Block | From |
| --- | --- |
| `heading` | a paragraph whose style is the built-in heading 1 to 9, or whose outline level is set |
| `paragraph` | a run of text up to its paragraph mark, with alignment, indents, spacing, line spacing, page breaks, shading and borders; contextual spacing drops the space between paragraphs of one style |
| `listItem` | a paragraph naming a list override, its label counted in the level's number format and template, a start override restarting it |
| `table` | the paragraphs in a table, a cell ending at its cell mark and a row at its row mark, whose cell layout (`sprmTDefTable`) gives the grid, the merges going across and down, the header rows, the cell fills and borders; a nested table, whose cells end at paragraph marks flagged as inner cell ends, becomes a table in the cell that holds it |

Inline pictures are read from the Data stream (`PICF` and the drawing records that hold their
bytes); floating pictures and text boxes from the drawings the main text anchors, their position
from the shape's own properties over its anchor's. Footnotes, endnotes, headers and footers come
from their stories; fields keep their result, `HYPERLINK` makes a link and `PAGE` and `NUMPAGES`
the page fields. Comments come from their reference and text tables, with their author from the
owners table; an embedding field keeps its picture, and the storage of the object pool its
separator names is kept in `objects` as a compound file of its own. Hidden text, deletions and the
codes of fields are left out and counted. A doc is written as a docx with `writeDocx`, or in one
call with `convertToDocx`.

A Word 6 or Word 95 file (`nFib` below 192) is read into the same tree: its shorter File
Information Block, its 8-bit text, its formatting pages with two-byte page numbers and seven-byte
paragraph entries, its style sheet of 8-bit names and its one-byte property modifiers are each
rewritten as Word 97 stores them, so the same converter builds the tree. Text, styles, headings,
character and paragraph formatting, tables, sections and footnotes are read; comments, drawings
and pictures, which Word 6 stores another way, are left out.

## Trying it

```sh
cargo run -p rely-office-cli -- tree contract.docx
cargo run -p rely-office-cli -- tree verbale.odt
cargo run -p rely-office-cli -- tree relazione.doc
```

prints the blocks, the media list and the report in Rust's debug form.
