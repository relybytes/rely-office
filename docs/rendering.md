# Rendering

`rely-office-render` turns a document into PDF and PNG pages without an office suite. It reads the
document with `rely-office-core`, writes it out as Typst source and typesets that source with
Typst 0.15 in process, with the fonts the caller hands in. Nothing is read from the file system or
the network while rendering, so the same input and fonts give the same pages everywhere,
WebAssembly included.

## What each format becomes

| Format | Fidelity | How |
| --- | --- | --- |
| docx, odt, doc | `layout` | the document tree with its looks: each section's page size, orientation, margins and columns, headers and footers with their first and even variants, page numbers, body font, paragraph spacing, line spacing, indents, tabs with their leaders, alignment, borders and shading, page breaks, run fonts, sizes, colours and highlights, list labels in the numbering's own formats, tables with their style's regions, per-cell borders, fills and vertical alignment, tables nested in cells, footnotes and endnotes, text boxes and the shapes they are drawn as, images and drawn shapes where they float with the text flowing around their outline, EMF and WMF pictures, charts, SmartArt, equations, embedded objects as their picture |
| xlsx, ods | `layout` | every sheet with its print settings and its cells' formats: orientation, margins, column widths and row heights, hidden rows and columns, header and footer, grid lines, number formats, fonts, fills, borders, alignment and wrapping, merged cells, conditional formats (fills, fonts, data bars, icon sets); its pictures and charts over the cells they are anchored in |
| xls, csv, tsv | `layout` | the sheets with Excel's print defaults (portrait A4, 0.7 and 0.75 inch margins, 8.43 character columns, no grid) |
| pptx, odp, ppt | `layout` | one page per slide at the slide's size: background, the shapes of the master and layout behind the slide's own, placeholders placed and styled as their layout and master say, text frames, every `DrawingML` preset shape, custom and free-form outlines, lines, groups, rotations, solid, gradient and picture fills, tables, pictures, charts and SmartArt |
| doc, ppt that cannot be read | `text` | the extracted text, one paragraph per line, on A4 |

`Rendered::fidelity` says which of the two a render had, so a viewer can tell a laid-out preview
from a text one.

## How a docx is laid out

The rules follow Word, the program most docx files come from, rather than Typst's defaults:

- **The larger space wins.** Between two paragraphs the larger of the space after the first and
  the space before the second is kept, as Word does since 2007 and LibreOffice does. The two add up
  only where the document asks for Word 2003's rule (`PageLayout::spacing_adds_up`): a docx that
  sets `doNotUseHTMLParagraphAutoSpacing` or a compatibility mode below 12, a doc, and an odt whose
  `AddParaTableSpacing` is on, as it is by default. Every block is set with no spacing of its own
  and the gap is written as a weak `v`, which disappears at the top of a page; the space before the
  first block of the document or of a cell stays.
- **A line is the office suite's single line.** Word's single line is the larger of the font's
  Windows ascent plus descent and its typographic line with the line gap (`metrics.rs`, per family
  from the fonts the renderer holds). A paragraph's text is set with that line as its box, through
  Typst's `top-edge` and `bottom-edge`, so a paragraph of one line, a table cell among them, is as
  tall as Word makes it; the leading carries only what a larger line spacing adds, under every line
  but the last.
- **Lines fill greedily** (`linebreaks: "simple"`), as office suites break them, not with Typst's
  optimized breaking.
- **Fonts are substituted by metrics**: Calibri, Calibri Light and Aptos by Carlito, Cambria by
  Caladea, Arial, Helvetica, Segoe UI, Verdana, Tahoma and Trebuchet MS by Liberation Sans, Times
  New Roman, Georgia, Garamond and Book Antiqua by Liberation Serif, Courier New and Consolas by
  Liberation Mono (`SUBSTITUTES` in rely-office-core, the table the editor's `fontStack` shares).
  A family the table does not know falls back by the kind the document's font table gives it:
  roman to Liberation Serif, swiss to Liberation Sans, modern to Liberation Mono. The document's
  own family is listed first, then its alternative name, so a renderer given the real font uses
  it.
- **Embedded faces are used**: the faces a docx or odt carries (de-obfuscated, see
  [document-tree.md](document-tree.md)) join the renderer's fonts for that render only, so a
  document set in a font the server lacks is drawn in it.
- **Headings** keep their PDF bookmarks but draw only their style's look; a heading whose style
  names no size takes the sizes of Word's default template.
- **Images** Typst cannot decode fail the whole document in Typst, so a failed render with images
  is retried with every image drawn as an empty frame of its size.
- **Each section sets its own page.** A section starts a `#set page` with its size, margins,
  headers and footers, which starts a new page; a continuous section on the same paper goes on
  where the last one stopped, and columns are a `#columns` block inside the section. A section's
  first-page header is chosen against the page its marker lands on.
- **Floating objects stand where their anchor puts them**, the nearest Typst can: an object in
  front of or behind the text is placed at its point of the page (measured from the page, the
  margins, the column or the paragraph); one with text above and below on a line of its own. The
  pictures the text goes around are set at their sides, a second one on the same side beside the
  first, and the paragraph is broken line by line, each line as wide as the room all of them leave
  at its height, then the rest of the words at the full width below them, justified as the
  paragraph is (`rely-flow` in the generated source). A line with no room for its next word stays
  empty, as Word leaves it. A tab moves the text after it to the next stop from the margin, the
  paragraph's own or a default one, measured on the line it lands on. A square wrap leaves the
  picture's rectangle; a tight or through wrap keeps its polygon as an outline of sixteen bands
  (`WrapOutline`), which the lines follow. A text box draws its own fill and outline at its
  anchored size, in the outline of the preset its shape names.
- **Drawn shapes are pictures.** A docx `wps` shape or `wpg` group, an odt rectangle, ellipse,
  line, polygon, path or custom shape, and a doc's OfficeArt shape become an SVG picture of their
  frame (`geometry/picture.rs`): the outline of their geometry, their fill and line, and the text
  they carry centred in them. The picture is anchored where the shape floats; a group is one
  picture, its shapes placed through the group's child frame. A shape that holds a text box (a
  docx shape with `wps:txbx`, an odt shape whose geometry names a preset, a doc shape with text)
  is a text box instead, whose `TextFrame::shape` names the preset: its text stays in the tree,
  where an editor changes it, and the outline is drawn behind it.
- **SmartArt is drawn from the drawing Office keeps of it.** A docx or pptx SmartArt names, in its
  data model, the diagram drawing Office saved beside it (or its part relates to only one); its
  `dsp` shapes and groups, with their outlines, fills, lines and text, become one SVG picture at the
  frame's size. The layout algorithm of the diagram is not run, so a SmartArt saved without its
  drawing is left out.
- **Equations are Typst math.** An equation of the tree, LaTeX read from Office Math or MathML, is
  set as Typst math, in the line or on a line of its own, with DejaVu Math TeX Gyre; no text of the
  formula becomes syntax.
- **An embedded object is its picture.** An OLE object (a workbook in a document, a package) is
  drawn as the replacement picture Office saved with it, which is what Word shows too.

Text only ever enters the generated source as a Typst string literal, so no document content can
become markup or code.

## How a sheet is printed

The used range from A1, cut to the reading limits. Column widths are converted as Excel converts
them (256ths of a character to pixels at a 7 pixel digit, then points). Columns that do not fit
across the page continue on the next pages, all rows of one group of columns first, as the default
page order does. Cells never wrap: text longer than its column runs over the next one. Values that
read as numbers are aligned right. The header and footer codes `&L &C &R`, `&A` (sheet name), `&P`
(page), `&N` (pages), `&&`, `&"font,style"` and a font size are drawn; other codes are dropped. A
number wider than its column shows as `#` signs filling it, as a spreadsheet shows it.

Conditional formats are drawn as Excel draws them: the rules of each range in priority order give
a cell its fill and font, a colour scale its blended fill, a data bar a bar from the cell's left
edge, an icon set its icon before the value. The looks are computed through the editable sheet
model of `rely-office-sheet`, from the values the cells hold, so the preview and the editor never
disagree.

An ods is read into the same layout as an xlsx (`opendocument/spreadsheet.rs`): column widths,
row heights, hidden rows and columns, the cell styles with their data styles turned into Excel
format codes (`opendocument/spreadsheet/number_styles.rs`), merges, the page layout with its header and footer,
and the drawings and charts of each sheet. Dates and durations stored as ISO text become serial
numbers so they go through the cell's format.

The pictures and charts of a sheet are drawn over the cells they are anchored in: the cell of
each anchor carries an invisible marker, and the page's foreground places each drawing at its
marker with the anchor's offset. A drawing whose anchor cell is not printed goes after the cells
instead.

## How a slide is drawn

A pptx and an odp are read into one slide model (`read_presentation`): the page size, and for
each slide its background and its shapes in drawing order. A pptx slide is drawn over its layout
and master, a placeholder taking the frame and the text styles the layout's and master's
placeholder of the same index or kind give it, then the master's text styles and the theme; theme
colours keep their luminance, tint and shade changes, and `normAutofit` scales the text as
PowerPoint does. An odp page is drawn over its master page: the master's title and outline frames
stay templates, its footer, date and page number show when the page asks for them, and each
outline level takes its own presentation style (`Default-outline2` one level down). Text frames
keep their insets, anchor, bullets and numbering, and every shape is placed absolutely on a Typst
page of the slide's size.

A shape's outline comes from its geometry (`geometry.rs`). A `DrawingML` preset is evaluated from
the guide formulas of ECMA-376 against the shape's size and adjust values (`presets.rs` holds the
presets in that language, one statement a line); an `a:custGeom` with the same engine. An
`OpenDocument` enhanced geometry is interpreted with its equations, modifiers and the commands of
`draw:enhanced-path`, and a shape that came from `DrawingML` (`draw:type="ooxml-..."`) is drawn
again from its preset, since its saved path can miss the arcs. Polygons, polylines and SVG paths
are read as outlines. Paths are curves filled even-odd, so a ring or a frame keeps its hole.

The presets `presets.rs` does not write in guide language (curved and circular arrows, ribbons,
waves, braces, scrolls, the action buttons and the rest of the 187) are drawn from
`enhanced_presets.rs`, the `oox-drawingml-cs-presets` table of LibreOffice (MPL-2.0) as enhanced
geometries, which `scripts/enhanced-presets.py` regenerates. A face of a preset that is shaded
darker or lighter (the side of a cube, the fold of a ribbon) is drawn in its shade, and every
face is filled before any outline is drawn.

A shape or a slide background is filled with a solid colour, a gradient or a picture (`Paint`).
A linear gradient keeps its angle and a radial one its centre; an ODF axial gradient is drawn as
the linear gradient it mirrors; a picture fill is stretched or tiled at its size.

A ppt is read into the same slide model ([MS-PPT]): the persist directories of every saved edit
locate the document, its slides and their masters; the slide list holds the text of placeholders;
each shape gives its frame (mapped through the groups it is in), its type, which draws the
`DrawingML` preset of the same shape, or its own vertices and segments with the guide formulas a
vertex can name, its fill, line, rotation, picture and text. Text is styled by its paragraph and
character runs over the master's text styles, and colours that name the scheme take the slide's or
the master's.

## How a metafile is drawn

EMF and WMF pictures ([MS-EMF], [MS-WMF]) are converted to SVG (`metafile.rs`, also the CLI's
`svg` command): their records are played on a device context as Windows plays them, with its
mapping modes, world transforms, pens, brushes, fonts, paths and clipping regions, and every line,
shape, arc, Bezier, text and bitmap becomes an SVG element. Bitmaps are decoded from their DIBs,
RLE-compressed ones included, and a mask drawn with the source-paint and source-and pair becomes
the picture's transparency. The EMF+ records a file carries in comments are skipped, since such a
file also holds the plain EMF drawing. A docx, pptx or doc keeps its metafiles when it is written
back.

## How a chart is drawn

A chart is read from the values the file stores beside each formula (`c:numCache`), from the
workbook's own cells when it stores none, or, in an `OpenDocument` file, from the table the chart
object carries; the chart of an ods finds the cells of the sheet it names in that table, where
LibreOffice notes each range it copied. It is drawn as SVG in the look of Office's default chart
style: columns and bars clustered, stacked or to a whole, lines with or without markers, areas,
pies, doughnuts, scatter and bubble charts, radar charts outlined, marked or filled, and stock
charts with their high-low lines, up-down bars and close ticks, with the title, legend, axis
titles, gridlines, data labels and a secondary value axis. A surface chart is drawn from above,
each cell of its grid shaded by the band of the value axis it falls in, with a legend of the
bands. The chart kinds of Office 2016, a `cx:chartSpace` found under `cx:chart` in the first
choice of an `mc:AlternateContent`, are drawn too: waterfall, funnel, treemap, sunburst,
histogram, Pareto and box and whisker. Axis steps are round numbers (1, 2 or 5 times a power of
ten) unless the file fixes them. The SVG becomes an image of the document tree in a docx or an
odt, a picture of the slide in a pptx or an odp, and a drawing over the cells of an xlsx or ods
sheet.

## Fonts

The npm package ships 19 faces, in `fonts/` with their licence texts: under the SIL Open Font
License 1.1, Carlito and Caladea (four styles each), Liberation Sans and Liberation Serif (four
styles each) and Liberation Mono (regular and bold); under the licence of Bitstream Vera Fonts,
DejaVu Math TeX Gyre for equations. `Renderer::new` takes any TrueType or OpenType files;
the CLI reads the folder `RELY_OFFICE_FONTS` names, by default `./fonts`.

## The corpus and the pixel comparison

`corpus/sources/` holds Italian documents written for the comparison: a service contract, a
payment reminder, meeting minutes, a quote with a priced table, a three-page technical report, and
two semicolon-separated sheets, one of them wider than a page. `scripts/corpus.mjs`:

1. has LibreOffice write each HTML source as docx and each CSV as xlsx, so the documents carry
   LibreOffice's own styles rather than hand-made XML;
2. has LibreOffice print each document to PDF, rasterized to grey PNG at 72 dpi (`pdftoppm`, or
   Ghostscript with `RASTERIZER=gs`);
3. runs `rely-office-corpus`, which renders each document at 72 dpi and compares the pages.

The distance of two pages is the mean absolute difference of their grey levels averaged over 8
pixel blocks, from 0 (equal) to 1. Averaging forgives a line set a pixel or two away and still sees
a missing paragraph, table or image. A page passes at 0.03 or less; pages of different sizes, and
pages one side has and the other has not, fail. The report lands in `target/corpus/report.json`,
our pages in `target/corpus/ours/`.

Measured on 2026-10-28 with LibreOffice on Windows and Ghostscript as rasterizer: **11 of 11 pages
pass (100%)**, the worst at 0.023. Every docx has the page count LibreOffice prints. What lines the
docx pages up is the spacing rule and the single line above, and the room a table row gives the
double rule under it: two lines and the gap between them, three times the border's width, which
Word and LibreOffice add to the row's height.

CI runs the comparison on every push with a regression floor of 40%, below the local figure since
the Linux LibreOffice and Poppler differ; the job uploads the report and both sets of pages.

## Fuzzing

`fuzz/` is a cargo-fuzz workspace of its own with six targets: `extract_text`, `read_workbook`,
`read_document_tree`, `render`, `formula` and `spreadsheet`. `render` fails when the source written
from any readable document does not typeset, which would be a bug of the writer; `formula` when a
printed formula does not parse back to itself; `spreadsheet` when a workbook it read does not
survive a write and a read. `.github/workflows/fuzz.yml` runs
each target for five minutes every Monday and by hand, on nightly, and uploads a crashing input.

```sh
cargo install cargo-fuzz
cd fuzz && cargo +nightly fuzz run render -- -max_total_time=60
```

## Trying it locally

```sh
cargo run -p rely-office-cli -- pdf contratto.docx      # writes contratto.docx.pdf
cargo run -p rely-office-cli -- png vendite.xlsx        # writes vendite.xlsx-1.png, ...
cargo run -p rely-office-cli -- typst contratto.docx    # prints the generated Typst source
SOFFICE=soffice node scripts/corpus.mjs                 # the corpus comparison
```

## Known gaps

- A tab beside a picture goes to a left stop: a centred, right or decimal stop is taken as a left
  one there. A list item, and a paragraph with an object nearly as wide as the column, keep the
  picture beside the whole paragraph.
- A drawing anchored in a sheet is placed at its anchor cell and offset at its own size; it does
  not stretch with the cells of a two-cell anchor.
- A table cell's fill is a solid colour; gradients and pictures fill shapes and backgrounds only.
- A Word 6 or Word 95 document's drawing objects are drawn in front of the text, and their
  shadows and line dashes are left out; its pictures linked from outside the file are missing.
  The reader was checked against LibreOffice on synthetic files only.
- An OLE object is drawn as its replacement picture, as Word draws it; nothing runs the program
  that made it. An odt's embedded ODF document other than a chart or a formula is its picture too.
- EMF+ drawings are drawn from the plain EMF records beside them, so what only EMF+ holds
  (antialiased gradients, GDI+ text effects) is lost.
- A ppt's or a doc's shape takes the adjust values it sets for about sixty shape types, each
  mapping checked against the preset's defaults; the bent, curved, circular and U-turn arrows,
  the left-up and left-right-up arrows, the quad arrow callout, the elliptical ribbons and the
  action buttons are drawn with the preset's defaults. LibreOffice writes the shapes it
  exports to ppt from `DrawingML` as free forms with guide constants cut to sixteen bits, so those
  outlines come out wrong; LibreOffice cannot draw them back either.
- A preview draws the slides, not their speaker notes, transitions or animations, which the slide
  model carries for an editor (see `editing.md`).
- A SmartArt saved without the drawing Office keeps of it is left out: its layout is not computed.
