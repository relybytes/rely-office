# Editing

rely-office edits in three ways, one per kind of document:

- **Text documents** are edited as a document tree. `read_document_tree` turns a docx, a doc or an
  odt into neutral blocks an editor maps onto its own schema, and `read_document_tree_named` reads
  a Markdown or an HTML file the same way; `write_docx`, `write_odt`, `write_markdown` and
  `write_html` turn the tree back into a file, so a document is saved in the format it came in or
  any other of these. The tree is the contract with the editor; the frontend package has the
  editor built on it.
- **Presentations** are edited as slides. `read_presentation` turns a pptx, an odp or a ppt into
  the slide model, serializable as JSON like the tree; `write_pptx` and `write_odp` write it back.
- **Spreadsheets** are edited through `rely-office-sheet`, a workbook model with formulas that an
  editor drives cell by cell, and that opens xlsx, xls, ods, csv and tsv and saves xlsx, ods and
  csv.

The old binary formats are converted, not edited in place: a doc becomes a docx or an odt and a
ppt a pptx or an odp through the same structures the new formats are read into, so nothing
downstream knows the old formats and a file saved after an edit is always in a new one.

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
| `image` | an inline drawing of its size, cut to the text width, the image stored once in `word/media/`; EMF and WMF pictures are kept as they are |
| an `image` an embedded object names | a `w:object` with a VML shape of the picture and an `o:OLEObject` naming the object, stored in `word/embeddings/` as a package (`relationships/package`) or an OLE compound file (`relationships/oleObject`, `.bin`) |
| `math` | Office Math, `m:oMath` in the line or `m:oMathPara` on a line of its own |
| `comment` marks, `commentAnchor` and `comments` | `w:commentRangeStart`, `w:commentRangeEnd` and `w:commentReference`, the comments in `comments.xml` and their replies and done states in `commentsExtended.xml` |
| `insertion` and `deletion` marks | `w:ins` and `w:del` with their author and date, a deleted run as `w:delText` |
| `formatChange` marks | `w:rPrChange` inside the run's `w:rPr`, with its author, date and the `previous` marks as the old `w:rPr` |
| `noteReference` and `notes` | `w:footnoteReference` and `w:endnoteReference`, the notes in `footnotes.xml` and `endnotes.xml` with their separators |
| `headersFooters` | header and footer parts with their own relationships, `w:titlePg` for a first page, `w:evenAndOddHeaders` in `settings.xml` for even pages |
| the looks (Rust) | paragraph borders, shading, tabs, spacing and indents; table widths, borders and cell margins; cell fills, borders and vertical alignment; a numbered heading keeps its label as text |
| the layout's page | `w:sectPr` with the page size, orientation, margins, columns and the default tab stop |
| `sections` | a `w:sectPr` in the paragraph that ends each section, with its own page, headers and footers and `w:type` `continuous` when it goes on on the same page |
| a floating image's `anchor` | a `wp:anchor` with its position, size and wrapping, instead of `wp:inline`; a tight wrap with its outline as `wp:wrapPolygon` |
| a floating text box's `frame` | a `wps:wsp` text box at its anchor, with its fill, outline and the `prstGeom` of its shape, holding its blocks |
| the spacing rule of the layout | `w:doNotUseHTMLParagraphAutoSpacing` in `settings.xml` when the spaces between paragraphs add up |

An image whose bytes are not a type Word shows is left out, its alternative text kept as text.
Text is escaped, and the characters XML 1.0 cannot hold are dropped. Reading the written file
gives back the same blocks, marks, alignment, spans, images and page (tested in
`document/write/tests.rs`); list ids are renumbered, as numbering instances are the file's own.

What an editor changes is all in the tree: the text of a shape is the blocks of its text box, an
equation its LaTeX, a comment its blocks, a tracked change the mark on its run. An embedded object
is not edited, only kept: its picture moves with the text and its bytes are written back as they
were read.

From JavaScript: `writeDocx({ blocks, media, page, notes, headersFooters, sections, comments,
objects })`, where `media` lists the bytes of each image in the order `image.media` indexes them,
`objects` the embedded objects as `readDocumentTree` gave them, and `page` is `{ widthTwips,
heightTwips, marginsTwips }`. `convertToDocx(bytes)` reads a doc, an odt or a docx and writes it
as a docx in one call.

### What a paragraph sets itself

A paragraph, a heading and a list item carry `format`, what they set themselves beside what their
style gives them: one of the named styles of Word (`title`, `subtitle`, `intenseQuote`, `caption`,
`noSpacing`, read from the style's name in English or Italian and written with the style
definition), the space before and after, the line spacing (a multiple, exact or at least), the
indents and a page break before. A paragraph that sets nothing has no `format`, so a tree read
before stays the same. A table carries `columnWidthsTwips` when the file gives its grid.

## Writing an odt

`write_odt(&DocumentTree) -> Vec<u8>` writes the same tree as an OpenDocument text LibreOffice and
Word open, its looks in automatic styles and its structure in LibreOffice's own named styles:

| Tree | odt |
| --- | --- |
| `heading` 1 to 6, `blockquote`, `codeBlock`, `horizontalRule` | `text:h` in `Heading_20_1` to `_6`, paragraphs in `Quotations`, `Preformatted_20_Text` and `Horizontal_20_Line` |
| `paragraph` with `align` and `format` | a paragraph style with its alignment, spacing, indents, line spacing and page break, kept once per look |
| `listItem` | `text:list` nested by level, one list style per kind of list, bullets and numbers by level |
| `table` | `table:table` with its column widths, header rows, cells spanned and the cells they cover, each paragraph of a cell in `Table_20_Contents` |
| `text` marks | bold, italic, underline, strike, super- and subscript, font, size and colour, highlight, code as `Source_20_Text`, links as `text:a` in `Internet_20_link` |
| `image` | a `draw:frame` in the line, the image stored once in `Pictures/`; a floating image's `anchor` as a frame anchored to its paragraph, with a graphic style for its wrapping (parallel, none, run-through in front or behind), position and margins |
| `math` | the LaTeX as text, since no formula object is written |
| `noteReference` and `notes` | `text:note` with its citation and body, in place |
| `comment` marks, `commentAnchor` and `comments` | `office:annotation` and `office:annotation-end` around the text, replies after their parent with `loext:parent-name`, a comment on no text at its point |
| `insertion` and `deletion` marks | `text:tracked-changes` with each change's author and date, `text:change-start` and `text:change-end` in the text, deleted text inside the change |
| page number and page count runs | `text:page-number` and `text:page-count` |
| `headersFooters`, `sections` | a master page per section with its page layout, headers and footers in `styles.xml`, each section starting on its master page |

The `mimetype` member comes first and uncompressed, as OpenDocument asks. Reading the written file
gives back the same blocks, marks, lists, tables, notes, comments and changes (tested in
`opendocument/text_write/tests.rs`), and LibreOffice draws it as the tree says (checked by hand).

## Markdown and HTML

`read_document_tree_named(bytes, file_name, limits, options)` reads a Markdown file (`.md`, by its
name) with CommonMark and its common extensions, and an HTML page (by its name or a doctype or
`<html>` at its start):

- **Markdown** (through `pulldown-cmark`): headings, paragraphs, emphasis, strong, strike,
  inline code and code blocks, block quotes, nested lists and task lists (`☑`, `☐`), tables,
  links, footnotes, inline and display math (`$…$`, `$$…$$`), and the inline HTML
  a Markdown file uses for what it has no syntax for (`<u>`, `<sup>`, `<sub>`, `<mark>`, `<br>`).
  An image written as a `data:` URL is kept as an image; one on the web stays a link, since
  nothing is fetched.
- **HTML** (through `html5gum`, which tokenizes and never runs anything): headings, paragraphs,
  the inline formatting tags, lists, tables with their header cells and spans,
  block quotes, `pre`, `hr`, links and `data:` images. Scripts, styles, frames, forms and the head
  are left out, and only links to the web and to mail are kept.

`write_markdown(&tree)` writes CommonMark with the same extensions, keeping in inline HTML what
Markdown cannot say, and the images as `data:` URLs; `write_html(&tree, title)` writes a page of
its own with the images inside, the notes at its end, a floating image floated to its side and
math as LaTeX in a `span` a math renderer can typeset. Text is escaped in both, and a link that is
not to the web or to mail is written as text.

## Writing a pptx

`write_pptx(&Presentation) -> Vec<u8>` writes the slide model `read_presentation` gives, from a
pptx, an odp or a ppt, as a pptx PowerPoint and LibreOffice open. What the source drew from its
masters and layouts is already resolved into each slide's shapes, so the file has one blank master
and layout and every shape on the slide itself:

| Slide model | pptx |
| --- | --- |
| the page size | `p:sldSz`, clamped to the 1 to 56 inch PowerPoint accepts |
| `background` | a `p:bg` with its solid, gradient or picture fill |
| a rectangle, rounded rectangle, ellipse or line | `p:sp` with its `prstGeom`, a line flipped when it runs the other way |
| a `Path` geometry | `a:custGeom` in the shape's own size, each path with its fill and stroke switches |
| fill, line, rotation | `a:solidFill`, `a:gradFill` with its stops and its angle or centre, `a:blipFill` stretched or tiled with the image stored once; `a:ln` with its width, `rot` on `a:xfrm` |
| `Text` | a `p:txBody` with its insets, anchor and wrapping; each paragraph's level, alignment, indents, line spacing, spacing and bullet or numbering; runs with size, bold, italic, underline, strike, colour and font, line breaks as `a:br` |
| `Picture` | `p:pic` with the image stored once in `ppt/media/`, EMF and WMF included; an image PowerPoint does not show (unknown bytes) is left out |
| `Table` | a `graphicFrame` holding an `a:tbl` with column widths, row heights, cell text and fills, `gridSpan`, `rowSpan`, and `hMerge` on a covered cell |
| `notes` | a notes slide over a notes master, the text in its body placeholder |
| `hidden` | `show="0"` on the slide |
| `transition` | `p:transition` with its effect, direction, speed, `advClick` and `advTm` |
| `animations` | `p:timing` with one effect per shape in order, its class, preset, subtype, trigger, delay, duration and motion path |
| a shape's `id` | its `p:cNvPr` id, which the animations name |

A shape with neither fill, line nor content draws nothing and is left out; a presentation with no
slides gets one blank slide. A colour that is not six hex digits is written as black, since the
JSON can come from outside the library. Reading the written file gives back the same slides
(tested in `slides/write/tests.rs`).

The slides are read with their show: the speaker notes, whether the slide is hidden, its
transition and the effects of its shapes come from the pptx timing tree and notes slides, from
the odp animation tree, drawing-page style and notes page, and from the ppt slide show atom,
notes pages, PowerPoint 2002 timeline and PowerPoint 97 builds. Each shape keeps its id; one drawn
from the layout or the master is marked `inherited`, so an editor can show it locked.

From JavaScript: `readPresentation(bytes, limits?)` returns `{ widthEmu, heightEmu, slides, media,
truncated }`, the slides tagged by `type` like the tree's blocks, and `writePptx({ widthEmu,
heightEmu, slides, media })` and `writeOdp(...)` write them; `convertToPptx(bytes)` reads a ppt, an
odp or a pptx and writes it as a pptx in one call.

## Writing an odp

`write_odp(&Presentation) -> Vec<u8>` writes the same slides as an OpenDocument presentation, every
page on one empty master: a text shape as a `draw:frame` with its text box, any other geometry as a
`draw:custom-shape` (rectangles, rounded rectangles and ellipses by type, a path by its enhanced
path), lines, pictures and tables; rotation in `draw:transform`; fills (solid, gradient, picture),
outlines, text anchors, insets and wrapping in graphic styles; paragraphs with their bullets and
numbers in list styles. The speaker notes, hidden slides, transitions (type, direction, speed and
how they advance) and animations (an `anim:par` tree with LibreOffice's presets, triggers and
delays) are written too, and read back to the same slides.

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

### Conditional formats and validation

Each sheet holds its conditional formats and data validations (`sheet_rules` in
`rely-office-core`, the model the renderer shares):

- `set_conditional_formats(sheet, formats)` replaces the formats of a sheet, each a list of ranges
  and rules: a cell value against one or two values, a formula, a colour scale of two or three
  points, a data bar, an icon set, the top or bottom ranks, above or below the average, text that
  contains, starts or ends with, duplicates and unique values, blanks, errors and date periods,
  each with its differential look (font, fill, borders, number format) and its stop flag.
- `conditional_looks(sheet)` computes the look each cell takes from the current values, rules in
  priority order, so an editor draws what Excel draws.
- `set_validations(sheet, validations)` replaces the validations: whole and decimal numbers, a
  list (inline or from a range), dates, times, text length or a formula, with the input and error
  messages and their style. `validation_at`, `is_valid`, `invalid_cells` and `validation_items`
  (the items a list offers) answer an editor checking a cell.

Ranges move with inserted and deleted rows and columns, and the formulas of rules follow the cells
they read, sheets renamed and removed included. A rule's formula must parse; a sheet holds at most
10,000 formats and 10,000 validations (`MAX_RULES`).

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
`=SE(A1>1,5;"sì";"no")` is stored as `IF(A1>1.5,"sì","no")`. Typed in the Italian locale, a `,`
between two digits is a decimal comma even with no `;` in the formula (`=A1*1,5`), as Italian Excel
reads it.

An editor shows a formula as its locale writes it: `Input::text_in` and `print_formula_in` give
Italian names, `;` between arguments and `,` in numbers (`=SOMMA(A1;0,5)`), `VERO` and `FALSO`, and
a number as `1234,5`; a text that would read as something else gets its leading `'`. What they
give reads back to the same cell, and the file keeps the English syntax. A function Italian Excel
does not rename (`XOR`, `INT`) keeps its name.

- **References**: `A1`, `$A$1`, `A$1`, ranges `A1:B9`, whole columns `A:C`, other sheets
  `Riepilogo!A1` and `'Vendite 2026'!B2:C9`, sheet names case-insensitive.
- **Operators** with Excel's precedence: comparison `= <> < <= > >=`, `&`, `+ -`, `* /`, `^` (left
  to right, so `2^3^2` is 64), unary signs (tighter than `^`, so `-2^2` is 4), postfix `%`.
- **Functions** (489, listed by `FUNCTION_NAMES` and, in the locale's names, by `function_names`
  for an editor's suggestions), in families: mathematics (`SUM`, `SUMPRODUCT`, `ROUND`, `MROUND`, `TRUNC`,
  `FLOOR`, `GCD`, `LOG`, the trigonometric functions, ...), statistics (`AVERAGE`, `MEDIAN`,
  `MODE.SNGL`, `STDEV.S`, `VAR.P`, `LARGE`, `SMALL`, `RANK.EQ`, `MAXIFS`, `MINIFS`, ...), logic (`IF`,
  `IFS`, `SWITCH`, `IFERROR`, `IFNA`, `XOR`, ...), text (`TEXT`, `TEXTJOIN`, `SUBSTITUTE`, `SEARCH`,
  `PROPER`, `VALUE`, ...), lookup (`VLOOKUP`, `HLOOKUP`, `XLOOKUP`, `LOOKUP`, `MATCH`, `INDEX`,
  `CHOOSE`, `ROW`, `COLUMN`, ...), dates (`DATE`, `EOMONTH`, `EDATE`, `WORKDAY`, `NETWORKDAYS`,
  `DATEDIF`, `WEEKDAY`, `ISOWEEKNUM`, `YEARFRAC`, ...) and finance (`PMT`, `IPMT`, `PPMT`, `FV`, `PV`,
  `NPER`, `RATE`, `NPV`, `IRR`), with the criteria of the `IF` functions (`>5`, `<>x`, `=x` and the
  wildcards `*` `?`). A second set covers what a workbook reaches for after those: hyperbolic and
  reciprocal trigonometry, combinations and permutations, `ROMAN`, `BASE`, `SUBTOTAL`; the
  statistics of samples and pairs (`GEOMEAN`, `KURT`, `SKEW`, `PERCENTILE.INC` and `.EXC`,
  `QUARTILE`, `PERCENTRANK`, `RANK.AVG`, `TRIMMEAN`, `CORREL`, `SLOPE`, `INTERCEPT`, `FORECAST`,
  `COVARIANCE.S`, `STEYX`, the `A` aggregates); the normal, exponential, Poisson and binomial
  distributions, `CONFIDENCE.NORM`, `GAMMA` and `FISHER`; depreciation (`SLN`, `SYD`, `DDB`, `DB`),
  `EFFECT`, `NOMINAL`, `CUMIPMT`, `CUMPRINC`, `MIRR`, `XNPV`, `XIRR`, `PDURATION`, `RRI`; dates with
  custom weekends (`WORKDAY.INTL`, `NETWORKDAYS.INTL`) and `DAYS360`; engineering (`BIN2DEC` and
  the eleven other base conversions, the `BIT` functions, `CONVERT` over length, mass, volume,
  time, speed, area, energy, power, pressure, information and temperature); text (`FIXED`,
  `DOLLAR`, `NUMBERVALUE`, `TEXTBEFORE`, `TEXTAFTER`, `UNICHAR`); information (`TYPE`,
  `ERROR.TYPE`, `ISREF`, `ADDRESS`). A third set completes Excel's library: the beta,
  chi-squared, F, gamma, hypergeometric, lognormal, negative binomial, Student's t and Weibull
  distributions with their inverses and their legacy names, `BINOM.DIST.RANGE`, `CHISQ.TEST`,
  `F.TEST`, `T.TEST` (paired, equal and unequal variances), `Z.TEST`, `CONFIDENCE.T`, `PROB`,
  `SKEW.P`, `ERF`, `ERFC` and `GAUSS`; complex numbers (`COMPLEX` and the 26 `IM` functions, in
  `i` or `j`, written with Excel's fifteen digits) and the Bessel functions; the database functions
  (`DSUM`, `DAVERAGE`, `DCOUNT`, `DGET` and the rest, a plain text criterion matching what begins
  with it); bonds and discounted securities (the six `COUP` functions, `PRICE`, `YIELD`,
  `DURATION`, `MDURATION`, `ACCRINT`, `ACCRINTM`, `DISC`, `INTRATE`, `RECEIVED`, `PRICEDISC`,
  `PRICEMAT`, `YIELDDISC`, `YIELDMAT`, the three Treasury bill functions, `AMORLINC`,
  `AMORDEGRC`, `VDB`) on Excel's five day-count bases; regression and matrices (`LINEST` and
  `LOGEST` with their statistics, `TREND`, `GROWTH`, `MINVERSE`, `MDETERM`, `MODE.MULT`); the
  references a formula computes (`OFFSET`, `INDIRECT` in A1 and R1C1, `TRIMRANGE`), `XMATCH`,
  `AGGREGATE` with its options, `CELL`, `SHEET`, `SHEETS`, `AREAS`, `FORMULATEXT`, `ISFORMULA`;
  random numbers (`RAND`, `RANDBETWEEN`, `RANDARRAY`, drawn again at each recalculation); regular
  expressions (`REGEXTEST`, `REGEXEXTRACT` in its three modes, `REGEXREPLACE` with `$1`
  groups and an occurrence, Unicode classes so `\w` takes `à`); and `SERIESSUM`, `PERCENTOF`,
  `ENCODEURL`, `VALUETOTEXT`, `ARRAYTOTEXT`, `ASC`, `DBCS` and the byte variants of the text
  functions. Every function has its Italian name where Italian Excel gives it one. `TEXT` reads
  its format code in Italian notation too. The `_xlfn.` prefix files store before the newer
  functions is read and written back, and `_xlfn._xlws.` before `FILTER` and `SORT`.

  Left out on purpose, and `#NAME?` like any unknown name: what calls outside the workbook
  (`CALL`, `REGISTER.ID`, `RTD`, `WEBSERVICE`, `STOCKHISTORY`, `IMAGE`, `TRANSLATE`, `FILTERXML`,
  `INFO`), what a pivot table answers (`GETPIVOTDATA`, `GROUPBY`, `PIVOTBY`), the Japanese and Thai
  text functions (`PHONETIC`, `BAHTTEXT`), `EUROCONVERT`, the exponential smoothing of
  `FORECAST.ETS` and its three companions, and the odd-period bond functions (`ODDFPRICE`,
  `ODDFYIELD`, `ODDLPRICE`, `ODDLYIELD`).
- **Arrays**: a formula whose result is an array spills it into the cells below and to the right
  of its own (`=FILTER(A2:C9;B2:B9="Nord")`). `FILTER`, `SORT`, `SORTBY`, `UNIQUE`, `SEQUENCE`,
  `TRANSPOSE`, `TAKE`, `DROP`, `CHOOSEROWS`, `CHOOSECOLS`, `VSTACK`, `HSTACK`, `TOCOL`, `TOROW`,
  `WRAPROWS`, `WRAPCOLS`, `EXPAND`, `MMULT`, `MUNIT`, `FREQUENCY` and `TEXTSPLIT` return one, and so
  does a range or an operator over ranges (`=C2:C9*2`), computed value by value with a single value
  stretched over the other side; the aggregates read an array as they read a range
  (`=SUM(C2:C9*2)`). Array constants are typed `{1,2;3,4}` in English and `{1;2|3;4}` in Italian.
  The spilled cells hold no input, and a formula reads them like any other cell; a spill that
  would cover a cell with content, another spill or the grid's edge gives `#SPILL!` in the
  formula's cell, and a `FILTER` that keeps nothing gives `#CALC!`. `spills(sheet)` lists the
  ranges and `spill_anchor(sheet, cell)` the formula a cell is filled by. An xlsx stores the
  formula as Excel's dynamic array (`t="array"`, `cm="1"` and `xl/metadata.xml`) and an ods as a
  matrix formula; both read back to the same spill.
- **Values and errors** follow Excel: numbers before text before booleans in comparisons, text
  without case; `#DIV/0!`, `#VALUE!`, `#REF!`, `#NAME?` (an unknown function or name), `#N/A`,
  `#NUM!` (an infinite result), and `#CIRC!` for a formula that reaches itself, which Excel stops
  at with a warning and an xlsx stores as `#REF!`.
- **Dates** are serial day counts from 30 December 1899, as in Excel's 1900 system. `TODAY` and
  `NOW` read a date the host sets with `set_today`, since WebAssembly has no clock.
- **Names**: a word that is no cell is a name. `LET(x, 2, y, x*3, x+y)` binds names for the
  formula it ends with, `LAMBDA(x, y, x*y)` makes a function called in place (`LAMBDA(x, x*2)(3)`)
  or by `MAP`, `REDUCE`, `SCAN`, `BYROW`, `BYCOL` and `MAKEARRAY`, `ISOMITTED` says a parameter
  was left out, and a function sees the names around the `LAMBDA` that made it. The workbook
  defines names (`set_defined_name`, `remove_defined_name`, `defined_names`), for the whole
  workbook or for one sheet, which wins on its own sheet: a range, a value or a formula, a
  `LAMBDA` among them, which formulas then call by its name, recursion included. A name defined
  from a formula keeps the sheet of each reference, as Excel's name manager does, and follows the
  rows and columns inserted and deleted and the sheets renamed and removed. Names compare without
  case; one Excel refuses (`A1`, `R1C1`, `TRUE`, a space) is refused with `InvalidName`. An xlsx
  keeps them as `definedName` (a function's parameters with the `_xlpm.` prefix Excel writes), an
  ods as `table:named-expressions`; both read back.
- **References a formula computes**: `OFFSET`, `INDIRECT` and `TRIMRANGE` give a range known only
  while computing, so the formula cells they reach are found then, and the formula is computed
  again after them; one that reaches its own cell is `#CIRC!`. A function that only describes a
  reference, `ROW`, `COLUMNS` or `OFFSET`'s first argument, makes no dependency on it.
- **Limits**: 8,192 characters and 128 levels of nesting per formula; functions and names call
  one another at most 100 levels deep, then `#NUM!`.

Recalculation computes every formula once, each after the formulas it reads, walking its own stack
rather than recursing: a chain of 100,000 formulas computes in WebAssembly without exhausting the
stack (tested).

### Undo

`EditHistory` keeps the workbook as it was before each edit (`record`, then `undo` and `redo`),
and a new edit drops what could be redone. A step is a whole copy rather than the inverse of the
edit, because an edit that moves rows rewrites formulas on every sheet: a copy cannot be wrong.
The copies are bounded, at most 100 steps and 2,000,000 cells together, the oldest going first, so
a large workbook keeps fewer steps rather than its size a hundred times over.

### Sorting, filtering, freezing and filling

- `sort_range(sheet, window, keys, has_header)` orders the rows of a rectangle by up to 64 keys,
  each a column and a direction: numbers before text (without case) before booleans before
  errors, blank cells last in either direction, equal rows in the order they had. The cells move
  with their looks, and a formula that moves points from its new row, as Excel moves it.
- `set_auto_filter(sheet, window)` puts filter buttons on the first row of a rectangle;
  `set_filter_column(sheet, column, shown)` chooses the values a column shows, and the rows whose
  value is left out are hidden, each keeping the height it had to come back to. `filter_choices`
  gives the values a column can show, as keys (what a file stores) and as the locale shows them.
  The filter is read from and written to an xlsx with the hidden name Excel keeps for it, and it
  follows the rows and columns inserted or deleted around it.
- `set_frozen(sheet, rows, columns)` freezes the first rows and columns; an xlsx keeps them as a
  frozen pane.
- `fill_range(sheet, source, target)` is the fill handle: the target holds the source and reaches
  past it on one side, and each row or column of the source makes its own series. Numbers follow
  their least-squares trend (`1, 3` gives `5, 7`), a single date counts days, a text ending in a
  number counts on keeping its zeros (`Lotto 08`, `Lotto 09`), month and weekday names in Italian
  and English follow the calendar in the case they were written, formulas move their relative
  references, and anything else repeats with its look. It is one step of the history.

### Comments

`set_comment(sheet, cell, author, text)` writes the comment of a cell, an empty text removing it,
and `Sheet::comments` and `Sheet::comment` read them. A comment moves with its cell when rows and
columns are inserted and deleted, and goes when its row or column does. An xlsx keeps the comments
in its comments part with the VML note boxes Excel shows them in; an ods as `office:annotation`;
both read back with the author and the lines.

A comment is a note, as older Excel and LibreOffice write them, or a thread, as Excel 365 does:
`start_comment_thread(sheet, cell, author, text, date)` opens one, `reply_to_comment` answers it,
`set_comment_resolved` resolves or reopens it and `remove_comment_reply` takes a reply away; a
note answered or resolved becomes a thread, and `set_comment` on a thread changes its text and
keeps its author and replies. A date is ISO 8601 (`2026-10-30T09:00:00.000Z`), passed in by the
host, since WebAssembly has no clock; a thread keeps at most 1,000 replies.

An xlsx keeps a thread in its `threadedComments` part, the people who wrote it in
`xl/persons/person.xml`, and beside it the note Excel writes for the versions that do not read
threads, its author `tc=` and the thread's id and its text the thread spelled out. Reading an
xlsx takes the threads and the notes on the cells no thread is on. The ids are GUIDs made from
where each thread sits, so the same workbook is always written the same way. ODF has no
threads: an ods writes a thread as the annotation of its cell, the replies following its text as
paragraphs that open with their author, and reads it back as one note.

### Charts

`charts(sheet)` lists the charts of a sheet, each with where it sits (`ChartPlacement`: the cell
of its corner, the offset into it and the size, in EMU) and, when an editor can change it, a
`ChartSpec`: its type (column, bar, line, area, pie, doughnut, scatter), whether it stacks, its
title and legend, and the ranges its categories and series read, as chart formulas
(`'Vendite'!$B$2:$B$7`). `chart_for_range(sheet, range, type)` makes the spec a selection
suggests: a series for each column, or each row of a selection wider than tall, a first row of
text naming them and a first column of text giving the categories. `add_chart`, `set_chart` (what
it shows, where it sits, or both) and `remove_chart` edit them; a range on a sheet the workbook
does not have is refused with `InvalidChart`.

The charts live in the drawing part the sheet carries: a new chart is an anchor and a chart part
added to it, a changed one has its part written again, and everything else the drawing holds,
pictures and charts no editor can describe, stays as the file had it. A chart part of one plot
whose series read ranges is described, so a chart made here, or a simple one from Excel or
LibreOffice, stays editable after a save. The parts are written with explicit colours, so the
chart looks the same in a workbook without a theme, and with no cached values: Excel and
LibreOffice compute them from the cells. Charts stay anchored to their cells as they were when
rows or columns move.

`Workbook::pictures(sheet)` draws the pictures and charts of a sheet, each chart from the values
its cells hold now, so a chart follows an edit; the drawing is kept until the workbook changes, a
change of a chart included.

### Edits that move cells

Inserting and deleting rows and columns move the cells and every formula of the workbook follows:
references after the edit shift, ranges grow or shrink, a reference to a deleted cell becomes
`#REF!`, and an insertion that would push content off the grid is refused. Renaming a sheet
rewrites the formulas that name it; removing one turns them into `#REF!`; the last sheet stays.
`copy_range` pastes a rectangle with relative references moved by the offset and absolute ones
kept, clearing the target first, as a spreadsheet's paste does.

### Opening and saving

`read_spreadsheet(bytes, name, limits, locale)` opens xlsx, xls, ods, csv and tsv: every formula
and value, dates as dates, column widths and sheet names, and from an xlsx and an ods the cell
styles, merged cells, row heights, conditional formats and validations. An ods's formulas are
read from OpenFormula and its ISO dates and durations become serial numbers. A CSV holds values,
not formulas, so a
leading `=` stays text; its numbers are read in the locale. `write_xlsx` saves each formula with
its computed value cached, so a viewer that does not recalculate (this crate's own previews
included) shows the results, and asks Excel to recalculate on opening; numbers, inline text,
booleans, number formats, styles, merged cells, column widths and row heights are kept. The
pictures and charts of each sheet, and the workbook's theme they take their colours from, are
written back exactly as the file had them: they stay anchored to the cells they had, so rows
inserted above a chart leave it where it was. Conditional formats are written with their
differential formats, validations with their messages, and pivot tables are carried with their
caches: the cells they show are ordinary cells, which a refresh in Excel rebuilds. LibreOffice
opens and recalculates the files (checked by hand on the corpus); the round trip is tested in
`xlsx/tests.rs`.

`write_ods` saves the workbook as an ods: cells with their computed values, formulas in
OpenFormula, styles with data styles built from the number formats, merges, column widths and row
heights, conditional formats and validations as LibreOffice writes them, the defined names and the
comments. Each picture is written as an image and each chart a spec describes as a chart object
reading the same cells, both in a frame anchored to their cell; a chart no spec describes is
written as the picture it is drawn as. A sheet read from an ods gets the same drawing a sheet of an
xlsx carries, built from its pictures and charts, so they are edited the same way and an xlsx saved
from it keeps them as charts. Pivot tables are kept by `write_xlsx` only. An xls is always saved as
xlsx.

`write_csv(workbook, sheet, locale, separator)` saves one sheet as UTF-8 with its byte order mark
and CRLF line ends, each cell as it is shown, fields quoted when they hold the separator, a quote
or a line break; `default_separator(locale)` is `;` in Italian and `,` in English, and
`Workbook::delimiter` gives the one of the csv or tsv the workbook was read from, so a save writes
the file back as it came.

An ods keeps the frozen panes in `settings.xml` and the filter as a `table:database-range` with its
conditions, the rows it hides marked as filtered.

Not kept: formulas with functions outside the list, which show `#NAME?` after an edit
recalculates them.

## From JavaScript

`createSheet({ locale })` and `openSheet(bytes, name, { locale })` return a `SheetDocument`; the
frontend package `@relybytes/rely-office-react` has the same class behind `await initSheets()`,
and its `SheetGrid` is an editor built on it. An
editor asks for the cells of the window it shows (`cells(sheet, { top, left, bottom, right })`,
each with its `input`, `display`, `kind` and `format`) and sends each action as a call: `setInput`,
`setInputs`, `setFormat`, `setColumnWidth`, `setRowHeight`, `setStyle`, `setBorders`,
`clearStyles`, `mergeCells`, `unmergeCells`, `clearRange`, `copyRange`, `insertRows`, `deleteRows`,
`insertColumns`, `deleteColumns`, `addSheet`, `renameSheet`, `removeSheet`,
`setConditionalFormats`, `setValidations`, `fillRange`, `sortRange`, `setAutoFilter`,
`setFilterColumn`, `setFrozen`, `setDefinedName`, `removeDefinedName`, `setComment`,
`startCommentThread`, `replyToComment`, `setCommentResolved`, `removeCommentReply`, `addChart`,
`setChart`, `removeChart`, `setToday`, `save`, `saveOds`, `saveCsv`; `filterChoices`, `pictures`
with `pictureBytes`, `definedNames`, `comments`, `charts`, `chartForRange`, `delimiter` and
`functionNames` serve an editor's menus; `stylesJson` and
`mergesJson` give the styles and merges to draw, `conditionalFormats`, `validations`,
`conditionalLooks`, `validation` and `invalidCells` the rules and what they make of the cells. A
cell of `cells` carries its `style`.
Every call that changes the workbook is a step of its history: `undo()` and `redo()` move along
it and `canUndo()` and `canRedo()` say whether they can; a call that is refused leaves no step.
`input` gives a cell as the document's locale writes it, Italian names and `;` in Italian. Failures
throw a `RelyOfficeError` with a stable
code (`INVALID_FORMULA`, `NO_SUCH_SHEET`, `DUPLICATE_SHEET_NAME`, `INVALID_SHEET_NAME`,
`LAST_SHEET`, `OUT_OF_BOUNDS`, `INVALID_INPUT`, `INVALID_LOCALE`, `INVALID_STYLE`, `INVALID_NAME`,
`INVALID_CHART`, and the reading codes on open).
