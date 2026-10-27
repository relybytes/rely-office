# Editing

rely-office edits in two ways, one per kind of document:

- **Word documents** are edited as a document tree. `read_document_tree` turns a docx into neutral
  blocks an editor maps onto its own schema; `write_docx` turns the tree back into a docx. The
  library has no editor of its own for text: the tree is the contract with the editor.
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
| `text` marks | `w:b`, `w:i`, `w:u`, `w:strike`, `w:vertAlign`, the `Code Char` style, and a `w:hyperlink` to an external relationship for `link` |
| `hardBreak`, tabs | `w:br`, `w:tab` |
| `image` | an inline drawing of its size, cut to the text width, the image stored once in `word/media/` |
| the layout's page | `w:sectPr` with the page size, orientation and margins |

An image whose bytes are not a type Word shows is left out, its alternative text kept as text.
Text is escaped, and the characters XML 1.0 cannot hold are dropped. Reading the written file
gives back the same blocks, marks, alignment, spans, images and page (tested in
`document/write/tests.rs`); list ids are renumbered, as numbering instances are the file's own.

From JavaScript: `writeDocx({ blocks, media, page })`, where `media` lists the bytes of each image
in the order `image.media` indexes them, and `page` is `{ widthTwips, heightTwips, marginsTwips }`.

## The spreadsheet model

`rely-office-sheet` holds a `Workbook` of up to 256 sheets and 2,000,000 cells with content, on
Excel's grid of 1,048,576 rows by 16,384 columns. A cell keeps what was typed (`Input`: a number, a
text, a boolean or a parsed formula) and how its number is shown (`NumberFormat`: general, date,
date and time, percent, or a fixed number of decimals). Every edit recalculates, so `value` and
`display` always answer with current results.

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
- **Functions** (53): `SUM`, `PRODUCT`, `AVERAGE`, `MIN`, `MAX`, `COUNT`, `COUNTA`, `COUNTBLANK`,
  `IF`, `IFERROR`, `AND`, `OR`, `NOT`, `TRUE`, `FALSE`, `ROUND`, `ROUNDUP`, `ROUNDDOWN`, `ABS`, `INT`,
  `SIGN`, `MOD`, `POWER`, `SQRT`, `PI`, `CONCATENATE`, `CONCAT`, `LEN`, `LEFT`, `RIGHT`, `MID`,
  `UPPER`, `LOWER`, `TRIM`, `ISBLANK`, `ISNUMBER`, `ISTEXT`, `ISERROR`, `SUMIF`, `COUNTIF`,
  `AVERAGEIF`, `SUMIFS`, `COUNTIFS` (criteria `>5`, `<>x`, `=x` and wildcards `*` `?`), `VLOOKUP`,
  `HLOOKUP`, `MATCH`, `INDEX`, `DATE`, `YEAR`, `MONTH`, `DAY`, `TODAY`, `NOW`.
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
and value, dates as dates, column widths and sheet names. A CSV holds values, not formulas, so a
leading `=` stays text; its numbers are read in the locale. `write_xlsx` saves each formula with
its computed value cached, so a viewer that does not recalculate (this crate's own previews
included) shows the results, and asks Excel to recalculate on opening; numbers, inline text,
booleans, number formats and column widths are kept. LibreOffice opens and recalculates the files
(checked by hand on the corpus); the round trip is tested in `xlsx/tests.rs`.

Not kept: cell styles other than number formats (fonts, fills, borders, alignment), merged cells,
charts, comments, data validation, defined names, and formulas with functions outside the list,
which show `#NAME?` after an edit recalculates them.

## From JavaScript

`createSheet({ locale })` and `openSheet(bytes, name, { locale })` return a `SheetDocument`; the
browser entry `@relybytes/rely-office/web` has the same class behind `await initSheets()`. An
editor asks for the cells of the window it shows (`cells(sheet, { top, left, bottom, right })`,
each with its `input`, `display`, `kind` and `format`) and sends each action as a call: `setInput`,
`setInputs`, `setFormat`, `setColumnWidth`, `clearRange`, `copyRange`, `insertRows`, `deleteRows`,
`insertColumns`, `deleteColumns`, `addSheet`, `renameSheet`, `removeSheet`, `setToday`, `save`.
Undo is the editor's: it keeps what it changed. Failures throw a `RelyOfficeError` with a stable
code (`INVALID_FORMULA`, `NO_SUCH_SHEET`, `DUPLICATE_SHEET_NAME`, `INVALID_SHEET_NAME`,
`LAST_SHEET`, `OUT_OF_BOUNDS`, `INVALID_INPUT`, `INVALID_LOCALE`, and the reading codes on open).
