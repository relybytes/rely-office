# Editing

rely-office edits in two ways, one per kind of document:

- **Text documents** are edited as a document tree. `read_document_tree` turns a docx or an odt
  into neutral blocks an editor maps onto its own schema; `write_docx` turns the tree back into a
  docx, so an odt edited this way is saved as a docx. The library has no editor of its own for
  text: the tree is the contract with the editor.
- **Spreadsheets** are edited through `rely-office-sheet`, a workbook model with formulas that an
  editor drives cell by cell, and that opens and saves xlsx.

## Writing a docx

`write_docx(&DocumentTree) -> Vec<u8>` writes every block and mark of the tree
(see [document-tree.md](document-tree.md)) with Word's built-in styles, so Word, LibreOffice and
this crate read the file the same way:

| Tree | docx |
| --- | --- |
| `heading` 1 to 6 | paragraph styles `Heading1` to `Heading6` (`heading 1` ...), with the look of Word's default template |
| `paragraph` with `align` | a paragraph with `w:jc` |
| `listItem` | `List Paragraph` with `w:numPr`; one numbering instance per `listId`, so each list counts from one; bullets and numbers by level (`1.`, `a.`, `i.`) |
| `table` | `Table Grid`, equal columns over the text width, `w:gridSpan` for column spans, `w:vMerge` with continuation cells for row spans, `w:tblHeader` for header rows |
| `blockquote` | its paragraphs in the `Quote` style |
| `codeBlock` | one `Code` paragraph per line |
| `horizontalRule` | an empty paragraph with a bottom border |
| `text` marks | `w:b`, `w:i`, `w:u`, `w:strike`, `w:vertAlign`, the `Code Char` style, `w:rFonts`, `w:sz` and `w:color` for `textStyle`, `w:shd` for `highlight`, and a `w:hyperlink` to an external relationship for `link` |
| page number and page count runs | `fldSimple` with `PAGE` and `NUMPAGES` |
| `hardBreak`, tabs | `w:br`, `w:tab` |
| `image` | an inline drawing of its size, cut to the text width, the image stored once in `word/media/` |
| `noteReference` and `notes` | `w:footnoteReference` and `w:endnoteReference`, the notes in `footnotes.xml` and `endnotes.xml` with their separators |
| `headersFooters` | header and footer parts with their own relationships, `w:titlePg` for a first page, `w:evenAndOddHeaders` in `settings.xml` for even pages |
| the looks (Rust) | paragraph borders, shading, tabs, spacing and indents; table widths, borders and cell margins; cell fills, borders and vertical alignment; a numbered heading keeps its label as text |
| the layout's page | `w:sectPr` with the page size, orientation, margins, columns and the default tab stop |

An image whose bytes are not a type Word shows is left out, its alternative text kept as text.
Text is escaped, and the characters XML 1.0 cannot hold are dropped. Reading the written file
gives back the same blocks, marks, alignment, spans, images and page (tested in
`document/write/tests.rs`); list ids are renumbered, as numbering instances are the file's own.

From JavaScript: `writeDocx({ blocks, media, page, notes, headersFooters })`, where `media` lists
the bytes of each image in the order `image.media` indexes them, and `page` is
`{ widthTwips, heightTwips, marginsTwips }`.

## The spreadsheet model

`rely-office-sheet` holds a `Workbook` of up to 256 sheets and 2,000,000 cells with content, on
Excel's grid of 1,048,576 rows by 16,384 columns. A cell keeps what was typed (`Input`: a number, a
text, a boolean or a parsed formula) and how its number is shown (`NumberFormat`: general, date,
date and time, percent, a fixed number of decimals, or a format code of the file such as
`#,##0.00 "€"`). Every edit recalculates, so `value` and `display` always answer with current
results.

### Styles, merges and sizes

A cell can carry a `CellStyle`: font (family, size, bold, italic, underline, strike, colour),
fill, borders per side, horizontal and vertical alignment, wrapping, and a number format code.
Styles are kept once in the workbook and cells point at them.

- `set_style(sheet, range, patch)` merges a `StylePatch` into every cell of a range: only the
  fields the patch names change, so making a range bold keeps its fill.
- `set_borders(sheet, range, placement, border)` draws or clears borders as a toolbar does:
  `all`, `outline`, `inside`, `top`, `bottom`, `left`, `right`.
- `clear_styles(sheet, range)` returns a range to the default look.
- `merge_cells` merges a range, clearing the content of every cell but the first, as Excel asks;
  `unmerge_cells` splits it again. `set_row_height` sets a row's height, 0 hiding it.

Row and column edits move styles, merges, row heights and column widths with the cells. A range
can style at most 1,000,000 cells at once and a sheet hold 2,000,000 styled cells.

### Typing

`set_input(sheet, cell, text, locale)` reads text as a person types it:

- `=` starts a formula; a leading `'` keeps the rest as text;
- a number in the locale's notation is a number: `1.234,5` in Italian, `1,234.5` in English, with
  an optional sign, `%` or `€`;
- `TRUE` and `FALSE`, or `VERO` and `FALSO`, are booleans;
- an empty text clears the cell, keeping its format.

`set_inputs` types several cells in one recalculation and changes nothing when one is refused,
as a paste should.

### Formulas

Formulas are stored and printed in the syntax xlsx files use: English function names, `,` between
arguments, `.` in numbers. What a person types is accepted too: the Italian names of the common
functions (`SOMMA`, `SE`, `CERCA.VERT`, `SE.ERRORE`, `CONTA.SE`, ...) become the English ones, and a
formula that separates arguments with `;` reads `,` as the decimal separator, so
`=SE(A1>1,5;"sì";"no")` is stored as `IF(A1>1.5,"sì","no")`.

- **References**: `A1`, `$A$1`, `A$1`, ranges `A1:B9`, whole columns `A:C`, other sheets
  `Riepilogo!A1` and `'Vendite 2026'!B2:C9`, sheet names case-insensitive.
- **Operators** with Excel's precedence: comparison `= <> < <= > >=`, `&`, `+ -`, `* /`, `^` (left
  to right, so `2^3^2` is 64), unary signs (tighter than `^`, so `-2^2` is 4), postfix `%`.
- **Functions** (155), in families: mathematics (`SUM`, `SUMPRODUCT`, `ROUND`, `MROUND`, `TRUNC`,
  `FLOOR`, `GCD`, `LOG`, the trigonometric functions, ...), statistics (`AVERAGE`, `MEDIAN`,
  `MODE.SNGL`, `STDEV.S`, `VAR.P`, `LARGE`, `SMALL`, `RANK.EQ`, `MAXIFS`, `MINIFS`, ...), logic (`IF`,
  `IFS`, `SWITCH`, `IFERROR`, `IFNA`, `XOR`, ...), text (`TEXT`, `TEXTJOIN`, `SUBSTITUTE`, `SEARCH`,
  `PROPER`, `VALUE`, ...), lookup (`VLOOKUP`, `HLOOKUP`, `XLOOKUP`, `LOOKUP`, `MATCH`, `INDEX`,
  `CHOOSE`, `ROW`, `COLUMN`, ...), dates (`DATE`, `EOMONTH`, `EDATE`, `WORKDAY`, `NETWORKDAYS`,
  `DATEDIF`, `WEEKDAY`, `ISOWEEKNUM`, `YEARFRAC`, ...) and finance (`PMT`, `IPMT`, `PPMT`, `FV`, `PV`,
  `NPER`, `RATE`, `NPV`, `IRR`), with the criteria of the `IF` functions (`>5`, `<>x`, `=x` and the
  wildcards `*` `?`). `TEXT` reads its format code in Italian notation too. The `_xlfn.` prefix
  files store before the newer functions is read and written back.
- **Values and errors** follow Excel: numbers before text before booleans in comparisons, text
  without case; `#DIV/0!`, `#VALUE!`, `#REF!`, `#NAME?` (an unknown function or name), `#N/A`,
  `#NUM!` (an infinite result), and `#CIRC!` for a formula that reaches itself, which Excel stops
  at with a warning and an xlsx stores as `#REF!`.
- **Dates** are serial day counts from 30 December 1899, as in Excel's 1900 system. `TODAY` and
  `NOW` read a date the host sets with `set_today`, since WebAssembly has no clock.
- **Limits**: 8,192 characters and 128 levels of nesting per formula.

Recalculation computes every formula once, each after the formulas it reads, walking its own stack
rather than recursing: a chain of 100,000 formulas computes in WebAssembly without exhausting the
stack (tested).

### Edits that move cells

Inserting and deleting rows and columns move the cells and every formula of the workbook follows:
references after the edit shift, ranges grow or shrink, a reference to a deleted cell becomes
`#REF!`, and an insertion that would push content off the grid is refused. Renaming a sheet
rewrites the formulas that name it; removing one turns them into `#REF!`; the last sheet stays.
`copy_range` pastes a rectangle with relative references moved by the offset and absolute ones
kept, clearing the target first, as a spreadsheet's paste does.

### Opening and saving

`read_spreadsheet(bytes, name, limits, locale)` opens xlsx, xls, ods, csv and tsv: every formula
and value, dates as dates, column widths and sheet names, and from an xlsx the cell styles, merged
cells and row heights. A CSV holds values, not formulas, so a
leading `=` stays text; its numbers are read in the locale. `write_xlsx` saves each formula with
its computed value cached, so a viewer that does not recalculate (this crate's own previews
included) shows the results, and asks Excel to recalculate on opening; numbers, inline text,
booleans, number formats, styles, merged cells, column widths and row heights are kept. The
pictures and charts of each sheet, and the workbook's theme they take their colours from, are
written back exactly as the file had them: they stay anchored to the cells they had, so rows
inserted above a chart leave it where it was. LibreOffice opens and recalculates the files
(checked by hand on the corpus); the round trip is tested in `xlsx/tests.rs`.

Not kept: comments, data validation, conditional formatting, defined names, pivot tables, and
formulas with functions outside the list, which show `#NAME?` after an edit recalculates them.

## From JavaScript

`createSheet({ locale })` and `openSheet(bytes, name, { locale })` return a `SheetDocument`; the
browser entry `@relybytes/rely-office/web` has the same class behind `await initSheets()`. An
editor asks for the cells of the window it shows (`cells(sheet, { top, left, bottom, right })`,
each with its `input`, `display`, `kind` and `format`) and sends each action as a call: `setInput`,
`setInputs`, `setFormat`, `setColumnWidth`, `setRowHeight`, `setStyle`, `setBorders`,
`clearStyles`, `mergeCells`, `unmergeCells`, `clearRange`, `copyRange`, `insertRows`, `deleteRows`,
`insertColumns`, `deleteColumns`, `addSheet`, `renameSheet`, `removeSheet`, `setToday`, `save`;
`stylesJson` and `mergesJson` give the styles and merges to draw. A cell of `cells` carries its
`style`.
Undo is the editor's: it keeps what it changed. Failures throw a `RelyOfficeError` with a stable
code (`INVALID_FORMULA`, `NO_SUCH_SHEET`, `DUPLICATE_SHEET_NAME`, `INVALID_SHEET_NAME`,
`LAST_SHEET`, `OUT_OF_BOUNDS`, `INVALID_INPUT`, `INVALID_LOCALE`, `INVALID_STYLE`, and the
reading codes on open).
