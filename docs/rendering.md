# Rendering

`rely-office-render` turns a document into PDF and PNG pages without an office suite. It reads the
document with `rely-office-core`, writes it out as Typst source and typesets that source with
Typst 0.15 in process, with the fonts the caller hands in. Nothing is read from the file system or
the network while rendering, so the same input and fonts give the same pages everywhere,
WebAssembly included.

## What each format becomes

| Format | Fidelity | How |
| --- | --- | --- |
| docx | `layout` | the document tree with its looks: page size and margins of the last section, body font, paragraph spacing, line spacing, indents, alignment, page breaks, run fonts, sizes and colours, list indents from the numbering part, table grids, cell margins and borders, images |
| xlsx | `layout` | every sheet with its print settings: orientation, margins, column widths, hidden columns, header and footer, grid lines, default font |
| xls, ods, csv, tsv | `layout` | the sheets with Excel's print defaults (portrait A4, 0.7 and 0.75 inch margins, 8.43 character columns, no grid) |
| pptx, odt, odp, doc, ppt | `text` | the extracted text, one paragraph per line, on A4 |

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

`fuzz/` is a cargo-fuzz workspace of its own with four targets: `extract_text`, `read_workbook`,
`read_document_tree` and `render`. The last one fails when the source written from any readable
document does not typeset, which would be a bug of the writer. `.github/workflows/fuzz.yml` runs
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

- Headers, footers, footnotes, text boxes and floating images of a docx are not drawn; the import
  report counts them.
- Table cell shading, per-cell borders and vertical alignment are not read; a table draws all its
  borders or none.
- Tab stops are drawn as a space; numbered list labels are decimal, letters and roman numerals by
  level, not the numbering part's own formats.
- Sheets print values only: no cell formats, fonts, fills, merged cells or charts.
- pptx and the binary formats print their text, not their layout.
