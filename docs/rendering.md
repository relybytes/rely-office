# Rendering

`rely-office-render` turns a document into PDF and PNG pages without an office suite. It reads the
document with `rely-office-core`, writes it out as Typst source and typesets that source with
Typst 0.15 in process, with the fonts the caller hands in. Nothing is read from the file system or
the network while rendering, so the same input and fonts give the same pages everywhere,
WebAssembly included.

## What each format becomes

| Format | Fidelity | How |
| --- | --- | --- |
| docx, odt | `layout` | the document tree with its looks: page size, margins and columns, headers and footers with their first and even variants, page numbers, body font, paragraph spacing, line spacing, indents, tabs with their leaders, alignment, borders and shading, page breaks, run fonts, sizes, colours and highlights, list labels in the numbering's own formats, tables with their style's regions, per-cell borders, fills and vertical alignment, footnotes and endnotes, text boxes, images and charts |
| xlsx | `layout` | every sheet with its print settings and its cells' formats: orientation, margins, column widths and row heights, hidden rows and columns, header and footer, grid lines, number formats, fonts, fills, borders, alignment and wrapping, merged cells; its pictures and charts after the cells |
| xls, ods, csv, tsv | `layout` | the sheets with Excel's print defaults (portrait A4, 0.7 and 0.75 inch margins, 8.43 character columns, no grid) |
| pptx, odp | `layout` | one page per slide at the slide's size: background, the shapes of the master and layout behind the slide's own, placeholders placed and styled as their layout and master say, text frames, custom shapes, lines, groups, rotations, tables, pictures and charts |
| doc, ppt | `text` | the extracted text, one paragraph per line, on A4 |

`Rendered::fidelity` says which of the two a render had, so a viewer can tell a laid-out preview
from a text one.

## How a docx is laid out

The rules follow Word, the program most docx files come from, rather than Typst's defaults:

- **Spacing adds up.** The space after a paragraph and the space before the next are summed, as
  Word does. Every block is set with no spacing of its own and the gap is written as a weak `v`,
  which disappears at the top of a page; the space before the first block of the document or of a
  cell stays.
- **Line height comes from the font.** Word's single line is the larger of the font's Windows
  ascent plus descent and its typographic line with the line gap. Typst sets a line between its
  ascender and descender, so the leading is the difference, computed per family from the fonts the
  renderer holds (`metrics.rs`). Word also adds that extra height below the last line of a
  paragraph, so it goes into the space after.
- **Lines fill greedily** (`linebreaks: "simple"`), as office suites break them, not with Typst's
  optimized breaking.
- **Fonts are substituted by metrics**: Calibri by Carlito, Cambria by Caladea, Arial and Helvetica
  by Liberation Sans, Times New Roman by Liberation Serif, Courier New by Liberation Mono. The
  document's own family is listed first, so a renderer given the real font uses it.
- **Headings** keep their PDF bookmarks but draw only their style's look; a heading whose style
  names no size takes the sizes of Word's default template.
- **Images** Typst cannot decode fail the whole document in Typst, so a failed render with images
  is retried with every image drawn as an empty frame of its size.

Text only ever enters the generated source as a Typst string literal, so no document content can
become markup or code.

## How a sheet is printed

The used range from A1, cut to the reading limits. Column widths are converted as Excel converts
them (256ths of a character to pixels at a 7 pixel digit, then points). Columns that do not fit
across the page continue on the next pages, all rows of one group of columns first, as the default
page order does. Cells never wrap: text longer than its column runs over the next one. Values that
read as numbers are aligned right. The header and footer codes `&L &C &R`, `&A` (sheet name), `&P`
(page), `&N` (pages), `&&`, `&"font,style"` and a font size are drawn; other codes are dropped.

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

## How a chart is drawn

A chart is read from the values the file stores beside each formula (`c:numCache`), from the
workbook's own cells when it stores none, or, in an `OpenDocument` file, from the table the chart
object carries. It is drawn as SVG in the look of Office's default chart style: columns and bars
clustered, stacked or to a whole, lines with or without markers, areas, pies, doughnuts and
scatter charts, with the title, legend, axis titles, gridlines, data labels and a secondary value
axis. Axis steps are round numbers (1, 2 or 5 times a power of ten) unless the file fixes them.
The SVG becomes an image of the document tree in a docx or an odt, a picture of the slide in a
pptx or an odp, and a drawing after the cells of an xlsx sheet. Radar, stock, bubble and surface
charts are left out, and so are the newer `chartex` charts (waterfall, treemap).

## Fonts

The npm package ships 18 faces under the SIL Open Font License 1.1, in `fonts/` with their licence
texts: Carlito and Caladea (four styles each), Liberation Sans and Liberation Serif (four styles
each), Liberation Mono (regular and bold). `Renderer::new` takes any TrueType or OpenType files;
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

Measured on 2026-10-27 with LibreOffice on Windows and Ghostscript as rasterizer: **6 of 11 pages pass (54.5%)**. Every sheet
page passes, and every docx has the page count LibreOffice prints. The docx pages that fail drift
vertically by a few pixels per heading: LibreOffice collapses part of the spacing around headings
where Word adds it up, and this renderer follows Word. LibreOffice is the reference because it is
the suite CI can run, not because it is the target.

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

- A docx is laid out with the page of its last section: a document that turns one section
  landscape prints every page portrait.
- Floating images and text boxes are set after the paragraph that anchors them, not where they
  float.
- The pictures and charts of a sheet print after its cells, at their size, not over the cells
  they are anchored to; an ods sheet prints no drawings.
- Shapes PowerPoint and Impress draw from a preset other than rectangles, rounded rectangles,
  ellipses and lines (arrows, stars, callouts) are drawn as their rectangle; gradients show their
  first colour, pictures as fills are not drawn.
- doc and ppt print their text, not their layout.
