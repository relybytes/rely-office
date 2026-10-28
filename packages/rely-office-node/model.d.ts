// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// The model the packages share: the backend package and the frontend package declare the same
// shapes, and this file is the one source of both.

/** A document's bytes. A Node `Buffer` is a `Uint8Array`. */
export type DocumentBytes = Uint8Array | ArrayBuffer | ArrayBufferView;

/** What a document is, read from its bytes. */
export type DocumentFormat =
  | 'docx'
  | 'xlsx'
  | 'pptx'
  | 'odt'
  | 'ods'
  | 'odp'
  | 'doc'
  | 'xls'
  | 'ppt'
  | 'csv'
  | 'tsv'
  | 'unknown';

/** Bounds applied while reading; a field left out keeps its default. */
export interface Limits {
  /** The largest input accepted, in bytes. Default 50 MiB. */
  maxInputBytes?: number;
  /** The largest archive member or stream decompressed, in bytes. Default 128 MiB. */
  maxMemberBytes?: number;
  /** The most characters of text returned. Default 1,000,000. */
  maxTextChars?: number;
  /** The most rows of each sheet. Default 1,000. */
  maxRows?: number;
  /** The most columns of each sheet. Default 60. */
  maxColumns?: number;
  /** The largest image returned by `readDocumentTree`, in bytes. Default 10 MiB. */
  maxImageBytes?: number;
  /** The most images returned by `readDocumentTree`. Default 200. */
  maxImages?: number;
  /** The most bytes of all the images of `readDocumentTree` together. Default 100 MiB. */
  maxTotalImageBytes?: number;
}

/** The stable code of a `RelyOfficeError`. */
export type ErrorCode =
  | 'UNSUPPORTED_FORMAT'
  | 'DAMAGED_CONTAINER'
  | 'TOO_LARGE'
  | 'MALFORMED_XML'
  | 'UNREADABLE_WORKBOOK'
  | 'UNREADABLE_DOCUMENT'
  | 'ENCRYPTED'
  | 'INVALID_LIMITS'
  | 'INVALID_OPTIONS'
  | 'TYPESET_FAILED'
  | 'EXPORT_FAILED'
  | 'INVALID_TREE'
  | 'INVALID_LOCALE'
  | 'INVALID_INPUT'
  | 'INVALID_FORMULA'
  | 'NO_SUCH_SHEET'
  | 'DUPLICATE_SHEET_NAME'
  | 'INVALID_SHEET_NAME'
  | 'LAST_SHEET'
  | 'OUT_OF_BOUNDS'
  | 'INVALID_STYLE'
  | 'INTERNAL';

export interface ExtractedText {
  format: DocumentFormat;
  text: string;
  /** Whether the document holds more text than `maxTextChars` let through. */
  truncated: boolean;
}

export interface Sheet {
  /** Empty for CSV and TSV. */
  name: string;
  /** The displayed values from A1, row by row; every row has the same length. */
  rows: string[][];
  /** Rows in the sheet, before the cut to `maxRows`. */
  totalRows: number;
  /** Columns in the sheet, before the cut to `maxColumns`. */
  totalColumns: number;
}

export interface Workbook {
  format: DocumentFormat;
  sheets: Sheet[];
}

export type Mark =
  | { type: 'bold' }
  | { type: 'italic' }
  | { type: 'underline' }
  | { type: 'strike' }
  | { type: 'code' }
  | { type: 'superscript' }
  | { type: 'subscript' }
  /** The font, size and colour a run sets itself, as CSS values: `Arial`, `14pt`, `#1F3864`. */
  | { type: 'textStyle'; fontFamily?: string; fontSize?: string; color?: string }
  /** A highlight behind the text, as a CSS colour. */
  | { type: 'highlight'; color: string }
  /** Only `http:`, `https:` and `mailto:` targets are kept. */
  | { type: 'link'; href: string }
  /** Text a comment is on: `comment` indexes `DocumentTree.comments`. */
  | { type: 'comment'; comment: number }
  /** A tracked insertion, read only with `{ trackedChanges: 'keep' }`; `date` is ISO 8601. */
  | { type: 'insertion'; author: string; date?: string }
  /** A tracked deletion, still in the text until accepted; read only with `keep`. */
  | { type: 'deletion'; author: string; date?: string };

export type Inline =
  | { type: 'text'; text: string; marks?: Mark[] }
  | { type: 'hardBreak' }
  /** `media` indexes `DocumentTree.media`; sizes are CSS pixels. */
  | { type: 'image'; media: number; alt?: string; width?: number; height?: number }
  /** A footnote or endnote: `note` indexes `DocumentTree.notes`. */
  | { type: 'noteReference'; note: number }
  /** A comment on a point rather than on text: `comment` indexes `DocumentTree.comments`. */
  | { type: 'commentAnchor'; comment: number }
  /** An equation in LaTeX, `\frac{a}{b}`; `display` sets it on a line of its own. */
  | { type: 'math'; latex: string; display: boolean };

/** A comment, or a reply to one. */
export interface Comment {
  author: string;
  initials?: string;
  /** ISO 8601, as the file stores it. */
  date?: string;
  blocks: Block[];
  /** The comment this one answers, an index of `DocumentTree.comments`. */
  parent?: number;
  /** The discussion is marked as done. */
  resolved: boolean;
}

/** How a document is read. */
export interface TreeOptions {
  /**
   * `accept` (the default) reads the document as if every tracked change were accepted;
   * `keep` keeps both, each run marked `insertion` or `deletion`.
   */
  trackedChanges?: 'accept' | 'keep';
}

export interface TableCell {
  colSpan: number;
  /** A cell merged with the ones below it; the covered cells are not listed. */
  rowSpan: number;
  blocks: Block[];
}

export interface TableRow {
  /** Whether the row repeats as the table header. */
  header: boolean;
  cells: TableCell[];
}

/** How a paragraph's lines sit between the margins; absent means the start of the line. */
export type Alignment = 'left' | 'center' | 'right' | 'justify';

export type Block =
  | { type: 'heading'; level: 1 | 2 | 3 | 4 | 5 | 6; content: Inline[]; align?: Alignment }
  | { type: 'paragraph'; content: Inline[]; align?: Alignment }
  /**
   * One list item. Consecutive items with the same `listId` form one list; `level` is the
   * nesting depth, 0 for the outermost.
   */
  | { type: 'listItem'; ordered: boolean; level: number; listId: number; content: Inline[] }
  | { type: 'table'; rows: TableRow[] }
  | { type: 'blockquote'; blocks: Block[] }
  | { type: 'codeBlock'; text: string }
  | { type: 'horizontalRule' };

export interface MediaItem {
  index: number;
  /** Sniffed from the bytes: `image/png`, `image/jpeg`, ..., or `application/octet-stream`. */
  contentType: string;
  bytes: Uint8Array;
}

/** What the tree leaves out or changes, counted by kind. */
export interface ImportReport {
  /** The text reached `maxTextChars` and the rest of the document was left out. */
  truncated: boolean;
  textBoxes: number;
  footnotes: number;
  endnotes: number;
  comments: number;
  headersAndFooters: number;
  /** Fields kept as the text they last showed. */
  fieldsKeptAsText: number;
  charts: number;
  smartArt: number;
  equations: number;
  shapes: number;
  embeddedObjects: number;
  /** Links whose target is not http, https or mailto; the text stays. */
  unsupportedLinks: number;
  /** Images larger than `maxImageBytes`. */
  oversizedImages: number;
  /** Images past `maxImages` or `maxTotalImageBytes`. */
  imagesOverLimit: number;
  /** Images whose part is missing or linked from outside the document. */
  missingImages: number;
  /** Tracked insertions, accepted. */
  trackedInsertions: number;
  /** Tracked deletions, accepted (their text is gone). */
  trackedDeletions: number;
  hiddenRuns: number;
}

export interface Note {
  kind: 'footnote' | 'endnote';
  blocks: Block[];
}

/**
 * The headers and footers of the last section. The first-page pair is set only when the
 * section has a different first page, the even-page pair only when odd and even pages differ.
 */
export interface HeadersFooters {
  header: Block[];
  footer: Block[];
  firstHeader?: Block[];
  firstFooter?: Block[];
  evenHeader?: Block[];
  evenFooter?: Block[];
}

/** In twentieths of a point; margins top, right, bottom, left. */
export interface PageSize {
  widthTwips: number;
  heightTwips: number;
  marginsTwips: [number, number, number, number];
}

/**
 * One section of a document of several: the blocks before `endBlock` that follow the section
 * before, on a page and with headers of their own; `continuous` goes on without a new page.
 */
export interface Section {
  endBlock: number;
  page: PageSize;
  headersFooters: HeadersFooters;
  continuous: boolean;
}

export interface DocumentTree {
  blocks: Block[];
  media: MediaItem[];
  report: ImportReport;
  /** The footnotes and endnotes, in the order the text refers to them. */
  notes: Note[];
  /** The headers, footers and page of the last section. */
  headersFooters: HeadersFooters;
  page: PageSize;
  /** Every section, the last included, when there are more than one. */
  sections?: Section[];
  /** The comments on the text, when there are any. */
  comments?: Comment[];
  /** The objects embedded in the text, when there are any. */
  objects?: EmbeddedObject[];
}

/**
 * An object embedded in a document (an OLE object, a workbook inside a document), which the
 * text shows as the image `media` indexes. `writeDocx` writes it back with that image.
 */
export interface EmbeddedObject {
  /** The image that shows it: an `Inline` image with this `media` is the object. */
  media: number;
  /** What opens it, as its ProgID: `Excel.Sheet.12`, `Word.Document.8`, `Package`. */
  program?: string;
  /** The name it was stored under: `Microsoft_Excel_Worksheet.xlsx`, `oleObject1.bin`. */
  name: string;
  /** An Office package (xlsx, docx, pptx) rather than an OLE object in a compound file. */
  package?: boolean;
  bytes: Uint8Array;
}

/** What to produce from one render. */
export interface RenderOptions {
  /** Write the PDF. Default true. */
  pdf?: boolean;
  /** A PNG of the first page this many pixels wide, at most 2,000. */
  thumbnailWidth?: number;
  /** A PNG of each of the first `maxPages` pages, at `pixelsPerPoint` (at most 8). */
  pageImages?: { pixelsPerPoint: number; maxPages: number };
  /**
   * Reading limits; the defaults are those of `Limits` except `maxTextChars`, 200,000, so a
   * render stays within a request.
   */
  limits?: Limits;
}

export interface Rendered {
  format: DocumentFormat;
  /**
   * `layout`: the document's own structure and page; `text`: its extracted text on A4, for a
   * doc or a ppt that cannot be read.
   */
  fidelity: 'layout' | 'text';
  pageCount: number;
  /** Whether the document held more than the limits let through. */
  truncated: boolean;
  pdf: Uint8Array | null;
  /** PNG. */
  thumbnail: Uint8Array | null;
  /** PNG, one per page. */
  pageImages: Uint8Array[];
}

/** A document tree to write as a docx: its blocks, the images they name by index, its page. */
export interface DocxInput {
  blocks: Block[];
  /** The bytes of each image; `Inline.media` indexes this list. The type is read from the bytes. */
  media?: DocumentBytes[];
  /** A4 with 2.5 cm margins when left out. */
  page?: PageSize;
  notes?: Note[];
  headersFooters?: HeadersFooters;
  /** The sections of a document of several, as `readDocumentTree` gives them. */
  sections?: Section[];
  /** The comments the marks and anchors of the blocks name. */
  comments?: Comment[];
  /** The objects the images of the blocks show, as `readDocumentTree` gives them. */
  objects?: EmbeddedObject[];
}

/** A point of a path, in EMU from the top-left corner of its frame. */
export interface PathPoint {
  x: number;
  y: number;
}

export type PathCommand =
  | { type: 'moveTo'; to: PathPoint }
  | { type: 'lineTo'; to: PathPoint }
  | { type: 'cubicTo'; controls: [PathPoint, PathPoint]; to: PathPoint }
  | { type: 'close' };

/** One path of a shape's outline. */
export interface ShapePath {
  commands: PathCommand[];
  filled: boolean;
  stroked: boolean;
  /** How the fill is shaded against the shape's colour, as a face of a cube. */
  shade: 'normal' | 'darken' | 'darkenLess' | 'lighten' | 'lightenLess';
}

export type ShapeGeometry =
  | { type: 'rectangle' }
  | { type: 'roundedRectangle' }
  | { type: 'ellipse' }
  /** From the top-left to the bottom-right corner of the frame, or the other diagonal. */
  | { type: 'line'; flipped: boolean }
  | { type: 'path'; paths: ShapePath[] }
  /** A preset the library does not know, drawn as its rectangle. */
  | { type: 'other' };

export interface Gradient {
  /** In order of their offset, from 0 to 1. */
  stops: { offset: number; colour: string }[];
  kind:
    | { type: 'linear'; angle: number }
    /** The centre as fractions of the frame's width and height. */
    | { type: 'radial'; centre: [number, number] };
}

/** What fills a shape or a slide; colours are `RRGGBB`. */
export type Paint =
  | { type: 'solid'; colour: string }
  | { type: 'gradient'; gradient: Gradient }
  /** An image of the presentation's `media`, stretched, or tiled at `tileEmu`. */
  | { type: 'picture'; media: number; tileEmu?: [number, number] };

/** A rectangle in EMU (914,400 to the inch) from the top-left corner of the slide. */
export interface Frame {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface SlideRun {
  text: string;
  /** Hundredths of a point. */
  size: number;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
  colour: string;
  family: string;
  /** A line break inside the paragraph instead of text. */
  lineBreak: boolean;
}

export interface SlideParagraph {
  level: number;
  align: 'left' | 'center' | 'right' | 'justify';
  bullet: { type: 'character'; text: string } | { type: 'number'; text: string } | null;
  marginLeftEmu: number;
  /** A negative indent hangs the bullet. */
  indentEmu: number;
  lineSpacingPercent: number;
  /** Hundredths of a point. */
  spaceBefore: number;
  spaceAfter: number;
  runs: SlideRun[];
}

export interface TextBody {
  paragraphs: SlideParagraph[];
  anchor: 'top' | 'middle' | 'bottom';
  /** Left, top, right, bottom, in EMU. */
  insetsEmu: [number, number, number, number];
  wrap: boolean;
}

export interface SlideTable {
  columnWidthsEmu: number[];
  rows: {
    heightEmu: number;
    cells: {
      body: TextBody;
      fill: string | null;
      colSpan: number;
      rowSpan: number;
      /** Covered by a merge from a cell before it, and not drawn. */
      covered: boolean;
    }[];
  }[];
}

export type ShapeContent =
  | { type: 'none' }
  | ({ type: 'text' } & TextBody)
  | { type: 'picture'; media: number }
  | ({ type: 'table' } & SlideTable);

export interface SlideShape {
  /** The shape's id on its slide, kept when the slide is written; leave it out on a new shape. */
  id?: number;
  /** Drawn from the slide's layout or master: an editor shows it locked. */
  inherited?: boolean;
  frame: Frame;
  /** Clockwise, in degrees. */
  rotation: number;
  geometry: ShapeGeometry;
  fill: Paint | null;
  line: { colour: string; widthEmu: number } | null;
  content: ShapeContent;
}

/** How a slide comes in. `effect` is a PresentationML name: `fade`, `push`, `wipe`, `split`, ... */
export interface Transition {
  effect: string;
  /** `l`, `r`, `u`, `d`, `horz`, `vert`, `in`, `out`, as the effect takes it. */
  direction?: string;
  durationMs?: number;
  advanceOnClick: boolean;
  advanceAfterMs?: number;
}

/** An effect on one shape, by the preset number of PowerPoint's gallery. */
export interface Animation {
  /** The index of the shape in `shapes`. */
  shape: number;
  class: 'entrance' | 'emphasis' | 'exit' | 'path';
  /** Entrance and exit: 1 appear, 2 fly, 10 fade, 22 wipe, 53 zoom; emphasis: 6 grow, 8 spin. */
  preset: number;
  presetSubtype: number;
  trigger: 'onClick' | 'withPrevious' | 'afterPrevious';
  delayMs: number;
  durationMs: number;
  /** For a path: PowerPoint's path language over fractions of the slide, `M 0 0 L 0.25 0 E`. */
  motionPath?: string;
}

export interface Slide {
  background?: Paint | null;
  /** In drawing order, the first at the back. */
  shapes?: SlideShape[];
  /** The speaker notes. */
  notes?: TextBody | null;
  /** Skipped when the show plays. */
  hidden?: boolean;
  transition?: Transition | null;
  animations?: Animation[];
}

export interface Presentation {
  /** The slide size in EMU. */
  widthEmu: number;
  heightEmu: number;
  slides: Slide[];
  /** The images the shapes and fills name by index. */
  media: MediaItem[];
  /** The file held more than the limits let through. */
  truncated: boolean;
}

/** Slides to write as a pptx; `media` holds the bytes, or the items `readPresentation` gave. */
export interface PptxInput {
  widthEmu: number;
  heightEmu: number;
  slides: Slide[];
  media?: (DocumentBytes | MediaItem)[];
}

/** How a cell's number is shown. */
export type CellFormat =
  | { type: 'general' }
  | { type: 'date' }
  | { type: 'dateTime' }
  | { type: 'percent' }
  | { type: 'decimals'; decimals: number }
  /** An Excel number format code, `#,##0.00 "€"`, `dd mmmm yyyy`. */
  | { type: 'code'; code: string };

/** A border edge: its line, its width in eighths of a point and its colour as `RRGGBB`. */
export interface Border {
  style: 'none' | 'single' | 'double' | 'dotted' | 'dashed';
  widthEighths: number;
  colour?: string | null;
}

/** How a cell looks; `stylesJson` lists them and every cell names one by index. */
export interface CellStyle {
  font: {
    family?: string | null;
    /** In tenths of a point. */
    sizeTenths?: number | null;
    /** `RRGGBB`; unset is automatic. */
    colour?: string | null;
    bold: boolean;
    italic: boolean;
    underline: boolean;
    strike: boolean;
  };
  /** `RRGGBB`. */
  fill?: string | null;
  borders: { top?: Border | null; right?: Border | null; bottom?: Border | null; left?: Border | null };
  horizontal?: 'left' | 'center' | 'right' | 'justify' | null;
  vertical?: 'top' | 'center' | 'bottom' | null;
  wrap: boolean;
  /** An Excel number format code; unset is General. */
  numberFormat?: string | null;
}

/**
 * A change to the look of cells: what is set changes, the rest stays. Colours are `RRGGBB` or
 * `#RRGGBB`; an empty text, a size of 0, `general` or `bottom` go back to the default.
 */
export interface StylePatch {
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strike?: boolean;
  fontFamily?: string;
  /** In points. */
  fontSize?: number;
  fontColour?: string;
  fill?: string;
  horizontal?: 'general' | 'left' | 'center' | 'right' | 'justify';
  vertical?: 'bottom' | 'center' | 'top';
  wrap?: boolean;
  /** An Excel number format code, `#,##0.00 "€"`; empty is General. */
  numberFormat?: string;
}

/** Which edges of a window a border goes on. */
export type BorderPlacement = 'all' | 'outline' | 'inside' | 'top' | 'bottom' | 'left' | 'right';

/** A merged range, zero-based and inclusive; its top-left cell shows the content. */
export interface CellRange {
  firstRow: number;
  firstColumn: number;
  lastRow: number;
  lastColumn: number;
}

/** A zero-based, inclusive rectangle of cells. */
export interface CellWindow {
  top: number;
  left: number;
  bottom: number;
  right: number;
}

export interface SheetSummary {
  name: string;
  /** The used area from A1: one past the last row and column with content. */
  rows: number;
  columns: number;
  /** `[column, width in characters]` for the columns not of the default width. */
  columnWidths: [number, number][];
  /** `[row, height in points]` for the rows that set one; 0 is hidden. */
  rowHeights: [number, number][];
  /** The pivot tables the sheet carries from its file; `save` writes them back unchanged. */
  pivotTables: number;
}

export type ComparisonOperator =
  | 'between'
  | 'notBetween'
  | 'equal'
  | 'notEqual'
  | 'greater'
  | 'less'
  | 'greaterOrEqual'
  | 'lessOrEqual';

/** A point of a colour scale, a data bar or an icon set, in the values of its ranges. */
export type ScaleValue =
  | { kind: 'min' }
  | { kind: 'max' }
  | { kind: 'number' | 'percent' | 'percentile' | 'formula'; value: string };

/** When a rule holds; formulas are in Excel notation, without `=`, for the first cell. */
export type Condition =
  | { kind: 'cellIs'; operator: ComparisonOperator; formulas: string[] }
  | { kind: 'expression'; formula: string }
  | { kind: 'colorScale'; points: { value: ScaleValue; colour: string }[] }
  | { kind: 'dataBar'; min: ScaleValue; max: ScaleValue; colour: string; showValue: boolean }
  | { kind: 'iconSet'; icons: string; thresholds: ScaleValue[]; reverse: boolean; showValue: boolean }
  | { kind: 'top'; rank: number; percent: boolean; bottom: boolean }
  | { kind: 'average'; below: boolean; orEqual: boolean; deviations: number | null }
  | { kind: 'text'; operator: 'contains' | 'notContains' | 'beginsWith' | 'endsWith'; text: string }
  | { kind: 'duplicates'; unique: boolean }
  | { kind: 'blanks'; blank: boolean }
  | { kind: 'errors'; error: boolean }
  /** `today`, `yesterday`, `tomorrow`, `last7Days`, `thisWeek`, `lastWeek`, `nextWeek`, `thisMonth`, `lastMonth`, `nextMonth`. */
  | { kind: 'timePeriod'; period: string };

/** What a rule changes in the look of a cell; unset keeps the cell's own. */
export interface DifferentialLook {
  fill?: string | null;
  fontColour?: string | null;
  bold?: boolean | null;
  italic?: boolean | null;
  underline?: boolean | null;
  strike?: boolean | null;
  numberFormat?: string | null;
}

export interface ConditionalRule {
  condition: Condition;
  look?: DifferentialLook | null;
  /** Lower first; 0 is unset, after every rule that sets one. */
  priority?: number;
  stopIfTrue?: boolean;
}

export interface ConditionalFormat {
  ranges: CellRange[];
  rules: ConditionalRule[];
}

export interface DataValidation {
  ranges: CellRange[];
  kind: 'any' | 'whole' | 'decimal' | 'list' | 'date' | 'time' | 'textLength' | 'custom';
  operator?: ComparisonOperator;
  /** One formula, or two for `between`; a list's items as `"a,b,c"` or a range. */
  formulas?: string[];
  allowBlank?: boolean;
  /** Whether a list offers its items in the cell. */
  dropdown?: boolean;
  errorStyle?: 'stop' | 'warning' | 'information';
  showError?: boolean;
  errorTitle?: string | null;
  error?: string | null;
  showPrompt?: boolean;
  promptTitle?: string | null;
  prompt?: string | null;
}

/** The look the conditional formats give one cell. */
export interface ConditionalLook {
  row: number;
  column: number;
  fill: string | null;
  fontColour: string | null;
  bold: boolean | null;
  italic: boolean | null;
  underline: boolean | null;
  strike: boolean | null;
  numberFormat: string | null;
  /** `length` is the share of the cell the bar fills, 0.1 to 0.9. */
  bar: { length: number; colour: string; showValue: boolean } | null;
  /** `index` counts the icons of the set from the lowest. */
  icon: { set: string; index: number; showValue: boolean } | null;
}

export interface CellValidation {
  validation: DataValidation | null;
  /** Whether the value passes; an empty cell always does. */
  valid: boolean;
  /** The items a list offers. */
  items: string[];
}

export interface SheetCell {
  row: number;
  column: number;
  /** What was typed, as a formula bar shows it: `=SUM(A1:A3)`. */
  input: string;
  /** What the cell shows, in the document's locale. */
  display: string;
  kind: 'number' | 'text' | 'bool' | 'error' | 'empty';
  format: CellFormat;
  /** The index of the cell's look in `styles()`, 0 for the default. */
  style: number;
}

export interface SheetOptions {
  /** How numbers are typed and shown: `it` (1.234,5, default) or `en` (1,234.5). */
  locale?: 'it' | 'en';
}

/**
 * An editable workbook. Every edit recalculates the formulas; sheets, rows and columns are
 * zero-based. Call `free` when done: the workbook lives in WebAssembly memory.
 */
export interface SheetDocument {
  sheets(): SheetSummary[];
  /** The cells with content or a look of their own in a window, at most 250,000 cells. */
  cells(sheet: number, window: CellWindow): SheetCell[];
  input(sheet: number, row: number, column: number): string;
  /** Type into a cell: `=` starts a formula; an empty text clears the cell. */
  setInput(sheet: number, row: number, column: number, text: string): void;
  /** Several cells in one recalculation, all or none: `[[row, column, text], ...]`. */
  setInputs(sheet: number, inputs: [number, number, string][]): void;
  setFormat(sheet: number, row: number, column: number, format: CellFormat): void;
  /** A width in characters; null restores the default. */
  setColumnWidth(sheet: number, column: number, width: number | null): void;
  /** Empty a window; its look stays. */
  clearRange(sheet: number, window: CellWindow): void;
  /** Change the look of a window, at most 1,000,000 cells. */
  setStyle(sheet: number, window: CellWindow, patch: StylePatch): void;
  /** Put a border on the edges of a window a placement names, or take it off with null. */
  setBorders(sheet: number, window: CellWindow, placement: BorderPlacement, border: Border | null): void;
  clearStyles(sheet: number, window: CellWindow): void;
  /** The looks the cells name by index; the first is the default. */
  styles(): CellStyle[];
  merges(sheet: number): CellRange[];
  /** Merge a window into one cell, keeping the content of its top-left cell. */
  mergeCells(sheet: number, window: CellWindow): void;
  /** Split the merged ranges a window overlaps. */
  unmergeCells(sheet: number, window: CellWindow): void;
  /** A height in points, 0 hiding the row; null lets it fit its content. */
  setRowHeight(sheet: number, row: number, height: number | null): void;
  /** Copy a window so its top-left lands on (row, column); relative references move with it. */
  copyRange(sheet: number, window: CellWindow, targetSheet: number, row: number, column: number): void;
  insertRows(sheet: number, at: number, count?: number): void;
  deleteRows(sheet: number, at: number, count?: number): void;
  insertColumns(sheet: number, at: number, count?: number): void;
  deleteColumns(sheet: number, at: number, count?: number): void;
  /** Add a sheet at the end and return its index. */
  addSheet(name: string): number;
  renameSheet(sheet: number, name: string): void;
  removeSheet(sheet: number): void;
  conditionalFormats(sheet: number): ConditionalFormat[];
  /** Replace the conditional formats of a sheet; their formulas must parse. */
  setConditionalFormats(sheet: number, formats: ConditionalFormat[]): void;
  validations(sheet: number): DataValidation[];
  setValidations(sheet: number, validations: DataValidation[]): void;
  /** The cells the conditional formats change, with their look, from the current values. */
  conditionalLooks(sheet: number): ConditionalLook[];
  /** The validation of a cell; an editor sets an input, asks, and takes it back on `stop`. */
  validation(sheet: number, row: number, column: number): CellValidation;
  /** `[row, column]` of the cells their validation refuses. */
  invalidCells(sheet: number): [number, number][];
  /** Today's date for TODAY() and NOW(), which WebAssembly cannot read; null unsets it. */
  setToday(today: Date | number | null): void;
  /** How a typed text would be read, without typing it. */
  previewInput(text: string): 'number' | 'text' | 'bool' | 'formula' | 'empty' | 'invalid';
  /** The workbook as xlsx, each formula with its computed value. */
  save(): Uint8Array;
  /** The workbook as ods; pictures, charts and pivot tables are kept by `save` only. */
  saveOds(): Uint8Array;
  free(): void;
}
