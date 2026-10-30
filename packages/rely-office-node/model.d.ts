// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

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
  | 'md'
  | 'html'
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
  | 'INVALID_NAME'
  | 'INVALID_CHART'
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
  | { type: 'deletion'; author: string; date?: string }
  /**
   * A tracked formatting change, read only with `keep`: `previous` holds the marks the text had
   * before it, which rejecting the change puts back.
   */
  | { type: 'formatChange'; author: string; date?: string; previous: Mark[] };

export type Inline =
  /**
   * `field` marks the page number (`page`) or the page count (`pages`) a header or a footer shows:
   * `text` is the value it showed last, and each page fills in its own.
   */
  | { type: 'text'; text: string; marks?: Mark[]; field?: 'page' | 'pages' }
  | { type: 'hardBreak' }
  /**
   * `media` indexes `DocumentTree.media`; sizes are CSS pixels. A floating image has an
   * `anchor`; one without stands in the line.
   */
  | { type: 'image'; media: number; alt?: string; width?: number; height?: number; anchor?: ImageAnchor }
  /** A footnote or endnote: `note` indexes `DocumentTree.notes`. */
  | { type: 'noteReference'; note: number }
  /** A comment on a point rather than on text: `comment` indexes `DocumentTree.comments`. */
  | { type: 'commentAnchor'; comment: number }
  /** An equation in LaTeX, `\frac{a}{b}`; `display` sets it on a line of its own. */
  | { type: 'math'; latex: string; display: boolean };

/** What a floating image's position is measured from. */
export type AnchorBase = 'page' | 'margin' | 'column' | 'paragraph' | 'line';

/** One axis of a floating image's position: an offset from its base, or an alignment in it. */
export interface AnchorPosition {
  base: AnchorBase;
  /** In EMU, 914,400 to the inch. */
  offsetEmu: number;
  /** Left or top, the middle, right or bottom; it wins over the offset. */
  align?: 'start' | 'center' | 'end';
}

/** How the text goes around a floating image. */
export type ImageWrap =
  | { type: 'around' }
  /** Beside its outline, in bands from the top, each side in 255ths of the width. */
  | { type: 'contour'; outline: { left: number[]; right: number[] } }
  | { type: 'topAndBottom' }
  | { type: 'inFront' }
  | { type: 'behind' };

/** Where a floating image stands, its size in EMU and how the text goes around it. */
export interface ImageAnchor {
  horizontal: AnchorPosition;
  vertical: AnchorPosition;
  widthEmu: number;
  heightEmu: number;
  wrap: ImageWrap;
  /** The space kept from the text, in EMU: top, bottom, left, right. */
  distanceEmu: [number, number, number, number];
}

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

/** The built-in paragraph styles of Word a paragraph can take beside the headings. */
export type NamedStyle = 'title' | 'subtitle' | 'intenseQuote' | 'caption' | 'noSpacing';

/** How the lines of a paragraph are spaced. */
export type LineSpacing =
  /** A multiple of single spacing in 240ths: 240 single, 360 one and a half, 480 double. */
  | { rule: 'auto'; value: number }
  /** Exactly this line height, in twentieths of a point. */
  | { rule: 'exact'; value: number }
  /** At least this line height, in twentieths of a point. */
  | { rule: 'atLeast'; value: number };

/**
 * What a paragraph sets itself, beside what its style gives it; every field left out is the
 * style's. Lengths in twentieths of a point (twips).
 */
export interface ParagraphFormat {
  style?: NamedStyle;
  spaceBeforeTwips?: number;
  spaceAfterTwips?: number;
  lineSpacing?: LineSpacing;
  indentLeftTwips?: number;
  indentRightTwips?: number;
  /** Positive indents the first line, negative hangs it. */
  indentFirstLineTwips?: number;
  /** The paragraph starts a new page. */
  pageBreakBefore?: boolean;
}

export type Block =
  | {
      type: 'heading';
      level: 1 | 2 | 3 | 4 | 5 | 6;
      content: Inline[];
      align?: Alignment;
      format?: ParagraphFormat;
    }
  | { type: 'paragraph'; content: Inline[]; align?: Alignment; format?: ParagraphFormat }
  /**
   * One list item. Consecutive items with the same `listId` form one list; `level` is the
   * nesting depth, 0 for the outermost.
   */
  | {
      type: 'listItem';
      ordered: boolean;
      level: number;
      listId: number;
      content: Inline[];
      format?: ParagraphFormat;
    }
  /** The width of each column in twips, when the file gives them. */
  | { type: 'table'; rows: TableRow[]; columnWidthsTwips?: number[] }
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
  /**
   * The file holds macros: kept in `DocumentTree.macroProject` for a docm and written back by
   * `writeDocx`, left out of a doc; never run.
   */
  macros?: boolean;
  /** Embedded font faces left out: unreadable, not a font, or past the limits. */
  fontsLeftOut?: number;
}

/** The kind of a typeface, as Word's font table and OpenDocument name it. */
export type FontGeneric = 'auto' | 'roman' | 'swiss' | 'modern' | 'script' | 'decorative';

/** Which face of a family an embedded font file is. */
export type FaceStyle = 'regular' | 'bold' | 'italic' | 'boldItalic';

/** A face of a family a document carries; `file` indexes `fontFiles`. */
export interface FontFace {
  style: FaceStyle;
  file: number;
}

/**
 * A family a document names, from a docx font table or the font face declarations of an odt,
 * with the faces it embeds. A writer lists it again with what is known of it.
 */
export interface DocumentFont {
  name: string;
  /** Another name the family goes by. */
  altName?: string;
  generic?: FontGeneric;
  pitch?: 'default' | 'fixed' | 'variable';
  /** Word's character set, two hexadecimal digits. */
  charset?: string;
  /** The PANOSE classification, twenty hexadecimal digits. */
  panose?: string;
  faces?: FontFace[];
}

/** The font and size of body text, the document's defaults. */
export interface DefaultFont {
  family?: string;
  /** In half points: 22 is 11 pt. */
  sizeHalfPoints?: number;
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
  /** The macros of a docm, an opaque value `writeDocx` writes back as they were. */
  macroProject?: Uint8Array;
  /** The families the document names, when its file lists any. */
  fonts?: DocumentFont[];
  /** The font files the document embeds, plain TrueType or OpenType; `FontFace.file` indexes them. */
  fontFiles?: Uint8Array[];
  /** The font and size of body text, when the document sets them. */
  defaultFont?: DefaultFont;
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
  /** The macros `readDocumentTree` gave; the file written is then a docm. */
  macroProject?: Uint8Array;
  /**
   * The families to list in the font table, as `readDocumentTree` gives them; a family the
   * text uses and this leaves out is listed with what is known of it.
   */
  fonts?: DocumentFont[];
  /**
   * The font files the faces of `fonts` embed: a docx carries them obfuscated with
   * `embedTrueTypeFonts`, an odt under `Fonts/`, an HTML page inline while small. A file that is
   * not a font, or past the limits, is left out with its faces.
   */
  fontFiles?: DocumentBytes[];
  /** The font and size of body text; Calibri 11 when left out. */
  defaultFont?: DefaultFont;
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
  /** The file holds macros: kept in `macroProject` for a pptm, left out of a ppt; never run. */
  holdsMacros?: boolean;
  /** The macros of a pptm, an opaque value `writePptx` writes back as they were. */
  macroProject?: Uint8Array;
}

/** Slides to write as a pptx; `media` holds the bytes, or the items `readPresentation` gave. */
export interface PptxInput {
  widthEmu: number;
  heightEmu: number;
  slides: Slide[];
  media?: (DocumentBytes | MediaItem)[];
  /** The macros `readPresentation` gave; the file written is then a pptm. */
  macroProject?: Uint8Array;
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
  /** The indent from the text's edge, in Excel's steps (10 points each); 0 when none. */
  indent?: number;
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
  /** Steps of indent, 0 to 250; 0 removes it. */
  indent?: number;
  /** An Excel number format code, `#,##0.00 "€"`; empty is General. */
  numberFormat?: string;
}

/** Which edges of a window a border goes on. */
export type BorderPlacement = 'all' | 'outline' | 'inside' | 'top' | 'bottom' | 'left' | 'right';

/**
 * A rectangle of a sheet as it was, from `region`, for `restoreRegion` to put back: what a
 * collaborative editor undoes with. Inputs are typed in English whatever the document's locale.
 */
export interface SheetRegion {
  area: CellRange;
  cells: { row: number; column: number; input: string; format: unknown; style: CellStyle | null }[];
  merges: CellRange[];
  comments: {
    row: number;
    column: number;
    author: string;
    text: string;
    date?: string | null;
    threaded?: boolean;
    resolved?: boolean;
    replies?: { author: string; text: string; date?: string | null }[];
  }[];
}

/** A merged range, zero-based and inclusive; its top-left cell shows the content. */
export interface CellRange {
  firstRow: number;
  firstColumn: number;
  lastRow: number;
  lastColumn: number;
}

/** A name the workbook defines, `Aliquota` or a `LAMBDA` a formula calls by name. */
export interface DefinedName {
  name: string;
  /** The sheet the name belongs to; absent for the whole workbook. */
  sheet?: number;
  /** What the name refers to, with its `=`, as the document's locale writes it. */
  formula: string;
  /** Whether the file hid the name from its list. */
  hidden: boolean;
}

/** The kinds of chart a sheet makes. */
export type ChartType = 'column' | 'bar' | 'line' | 'area' | 'pie' | 'doughnut' | 'scatter';

/** One series of a chart: the formula of the cell naming it and of the range of its values. */
export interface ChartSeries {
  name?: string | null;
  values: string;
}

/** A chart its editor can change, its ranges as formulas, `'Vendite'!$B$2:$B$7`. */
export interface ChartDefinition {
  chartType: ChartType;
  /** Bars, lines or areas stacked on one another. */
  stacked: boolean;
  title?: string | null;
  legend: boolean;
  /** The labels of the categories, or the x values of a scatter chart. */
  categories?: string | null;
  series: ChartSeries[];
}

/** Where a chart sits, in EMU (914,400 an inch): the cell of its corner, how far into it, its size. */
export interface ChartPlacement {
  row: number;
  column: number;
  offsetX?: number;
  offsetY?: number;
  width: number;
  height: number;
}

/** A chart of a sheet; `spec` is null for one drawn from a file that the editor cannot change. */
export interface SheetChart {
  index: number;
  spec: ChartDefinition | null;
  placement: Required<ChartPlacement>;
}

/** The comment on a cell. */
export interface CellComment {
  row: number;
  column: number;
  author: string;
  /** The text, lines separated by `\n`. */
  text: string;
  /** When it was written, ISO 8601, when the file says. */
  date?: string;
  /** A thread of Excel 365, with replies and a resolved state, rather than a note. */
  threaded: boolean;
  resolved: boolean;
  replies: CommentReply[];
}

/** A reply in a comment thread. */
export interface CommentReply {
  author: string;
  text: string;
  date?: string;
}

/** How a value field of a pivot table summarises the records of a cell. */
export type PivotFunction =
  | 'sum'
  | 'count'
  | 'average'
  | 'max'
  | 'min'
  | 'product'
  | 'countNumbers'
  | 'stdDev'
  | 'stdDevP'
  | 'var'
  | 'varP';

/** A source column placed on a pivot table's rows, columns or filters, by its position. */
export interface PivotFieldPlacement {
  field: number;
  /** The labels of the items left out: a number as a formula writes it, `TRUE`, or `""` for blanks. */
  hidden?: string[];
}

/** A source column a pivot table summarises. */
export interface PivotValueField {
  field: number;
  /** `sum` when left out. */
  function?: PivotFunction;
  /** The caption over its column; `Sum of Amount` when left out. */
  name?: string;
}

/** A pivot table to make or change: a source with a header row, and where its cells start. */
export interface PivotTableDefinition {
  name?: string;
  /** The sheet of the source, by position. */
  sourceSheet: number;
  source: CellWindow;
  /** The top-left cell of the table on its sheet: the filters, a blank row, then the table. */
  anchor: { row: number; column: number };
  rows?: PivotFieldPlacement[];
  columns?: PivotFieldPlacement[];
  values?: PivotValueField[];
  filters?: PivotFieldPlacement[];
  /** Both shown when left out. */
  rowGrandTotals?: boolean;
  columnGrandTotals?: boolean;
  /** Excel's forms: `compact` (the default) indents the row fields in one column, `outline` and `tabular` give each its own. */
  layout?: PivotLayout;
  /** Where the total of each group of an outer field shows; `top` when left out. */
  subtotals?: PivotSubtotals;
  /** Every row shows the labels of its groups, in the outline and tabular forms. */
  repeatLabels?: boolean;
  /** The axis several values lay along, Excel's `Σ Values`: `columns` (the default) or `rows`. */
  valuesAxis?: PivotValuesAxis;
  /** Where `Σ Values` sits among the fields of its axis, from 0 for the outermost; after the last when left out. */
  valuesPosition?: number | null;
}

/** The axis a pivot table lays several values along. */
export type PivotValuesAxis = 'columns' | 'rows';

/** How a pivot table lays out its row labels. */
export type PivotLayout = 'compact' | 'outline' | 'tabular';

/** Where a pivot table shows a group's total: on its row, below it, or not at all. The tabular form shows it below. */
export type PivotSubtotals = 'top' | 'bottom' | 'none';

/** A pivot table of a sheet, with the names of its source's fields. */
export interface SheetPivotTable {
  index: number;
  name: string;
  /** The sheet of the source by position; null when it is gone. */
  sourceSheet: number | null;
  sourceSheetName: string;
  source: CellWindow;
  anchor: { row: number; column: number };
  /** The cells it filled when last computed. */
  area?: CellWindow;
  rows: Required<PivotFieldPlacement>[];
  columns: Required<PivotFieldPlacement>[];
  values: PivotValueField[];
  filters: Required<PivotFieldPlacement>[];
  rowGrandTotals: boolean;
  columnGrandTotals: boolean;
  layout: PivotLayout;
  subtotals: PivotSubtotals;
  repeatLabels: boolean;
  valuesAxis: PivotValuesAxis;
  valuesPosition: number | null;
  fields: string[];
}

/** A column of a source a pivot table could read: its header and its items' labels, in order. */
export interface PivotSourceField {
  name: string;
  items: string[];
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
  /** The rows and columns frozen at the top and the left. */
  frozenRows: number;
  frozenColumns: number;
  autoFilter: AutoFilterSummary | null;
  /** How many pictures and charts `pictures` lists. */
  pictures: number;
}

/** The filter buttons of a sheet, on the first row of their window. */
export interface AutoFilterSummary {
  top: number;
  left: number;
  bottom: number;
  right: number;
  /** The filtered columns, counted from `left`, and the keys of the values each shows. */
  columns: { column: number; shown: string[] }[];
  /** The rows the filter hides. */
  hiddenRows: number[];
}

/** A value a filter column can show: its key, and how the document's locale shows it. */
export interface FilterChoice {
  /** The value as a file stores it; an empty key stands for the blank cells. */
  key: string;
  label: string;
}

/** A column a sort orders by, counted on the sheet. */
export interface SortKey {
  column: number;
  descending?: boolean;
}

/** A picture or a chart of the file, anchored at a cell; sizes in EMU, 9525 to a pixel. */
export interface SheetPicture {
  row: number;
  column: number;
  offsetXEmu: number;
  offsetYEmu: number;
  widthEmu: number;
  heightEmu: number;
  /** A chart is `image/svg+xml`, drawn from the values the file held when it was opened. */
  contentType: string;
  description: string | null;
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
  /**
   * What was typed into a cell, as the document's locale writes it: in Italian
   * `=SOMMA(A1;0,5)`, `1234,5`, `VERO`. Typed back with `setInput`, it gives the same cell.
   */
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
  /** The names the workbook defines, the workbook's and each sheet's. */
  definedNames(): DefinedName[];
  /**
   * Define a name or change what it refers to: `formula` typed as in a cell, in the document's
   * locale; `sheet` keeps it to one sheet. Throws `INVALID_NAME` for a name Excel refuses.
   */
  setDefinedName(name: string, formula: string, sheet?: number): void;
  /** Remove a name; false when there is none. Formulas that read it show `#NAME?`. */
  removeDefinedName(name: string, sheet?: number): boolean;
  /** The charts of a sheet, in the order its drawing lists them. */
  charts(sheet: number): SheetChart[];
  /** Add a chart; the index it gets. Throws `INVALID_CHART` for ranges the workbook does not have. */
  addChart(sheet: number, chart: ChartDefinition, placement: ChartPlacement): number;
  /** Change what a chart shows, where it sits, or both. */
  setChart(sheet: number, index: number, change: { chart?: ChartDefinition; placement?: ChartPlacement }): void;
  removeChart(sheet: number, index: number): void;
  /**
   * The chart a window makes: a series for each column, or each row of a window wider than
   * tall, a first row of text naming them and a first column of text giving the categories;
   * null when it holds no numbers.
   */
  chartForRange(sheet: number, window: CellWindow, type: ChartType): ChartDefinition | null;
  /** The comments of a sheet's cells, row by row. */
  comments(sheet: number): CellComment[];
  /** Write the comment on a cell, a thread keeping its author and replies; an empty text removes it. */
  setComment(sheet: number, row: number, column: number, text: string, author?: string): void;
  /** Open a thread on a cell in place of any comment it had; `date` is ISO 8601. */
  startCommentThread(sheet: number, row: number, column: number, text: string, author?: string, date?: string): void;
  /** Answer the comment of a cell; a note answered becomes a thread. */
  replyToComment(sheet: number, row: number, column: number, text: string, author?: string, date?: string): void;
  /** The pivot tables of a sheet. */
  pivotTables(sheet: number): SheetPivotTable[];
  /** The fields of a range a pivot table could read: its header row's names and each column's items. */
  pivotSourceFields(sheet: number, window: CellWindow): PivotSourceField[];
  /** Add a pivot table to a sheet and compute it into its cells; the index it gets. */
  addPivotTable(sheet: number, definition: PivotTableDefinition): number;
  /** Change a pivot table and compute it again; its old cells are cleared. */
  setPivotTable(sheet: number, index: number, definition: PivotTableDefinition): void;
  /** Compute a pivot table again from its source, as Excel's refresh does. */
  refreshPivotTable(sheet: number, index: number): void;
  /** Compute every pivot table again; how many were. */
  refreshPivotTables(): number;
  /** Remove a pivot table and the cells it filled. */
  removePivotTable(sheet: number, index: number): void;
  /** Mark the thread of a cell resolved, or open again. */
  setCommentResolved(sheet: number, row: number, column: number, resolved: boolean): void;
  /** Remove a reply of the thread of a cell, by its position. */
  removeCommentReply(sheet: number, row: number, column: number, reply: number): void;
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
  /**
   * Continue a window over a target that holds it and reaches past it on one side, as the fill
   * handle does: numbers follow their trend, a date counts days, a text ending in a number
   * counts on, month and weekday names follow the calendar, formulas move, the rest repeats.
   */
  fillRange(sheet: number, source: CellWindow, target: CellWindow): void;
  /** Freeze the first rows and columns so they stay in view; 0 and 0 unfreezes. */
  setFrozen(sheet: number, rows: number, columns: number): void;
  /**
   * Sort the rows of a window by its columns, the first key first; with `hasHeader` the first
   * row stays. Numbers come before text, text before booleans; blank cells go last.
   */
  sortRange(sheet: number, window: CellWindow, keys: SortKey[], hasHeader: boolean): void;
  /** Put filter buttons on the first row of a window; null takes them off, showing every row. */
  setAutoFilter(sheet: number, window: CellWindow | null): void;
  /**
   * Choose the values a filter column, counted from the filter's first, shows, by the keys of
   * `filterChoices`; null shows the whole column. The rows are filtered again.
   */
  setFilterColumn(sheet: number, column: number, shown: string[] | null): void;
  /** The values a filter column can show, in sort order. */
  filterChoices(sheet: number, column: number): FilterChoice[];
  /** The pictures and charts of the file; `pictureBytes` gives each one's bytes. */
  pictures(sheet: number): SheetPicture[];
  pictureBytes(sheet: number, index: number): Uint8Array;
  /** The functions a formula can call, as the document's locale names them, sorted. */
  functionNames(): string[];
  /** Today's date for TODAY() and NOW(), which WebAssembly cannot read; null unsets it. */
  setToday(today: Date | number | null): void;
  /** How a typed text would be read, without typing it. */
  previewInput(text: string): 'number' | 'text' | 'bool' | 'formula' | 'empty' | 'invalid';
  /** The workbook as xlsx, each formula with its computed value. */
  save(): Uint8Array;
  /** The workbook as ods, its pivot tables as data pilot tables; pivot tables an xlsx carried unread are kept by `save` only. */
  saveOds(): Uint8Array;
  /**
   * One sheet as csv: each cell as it shows in the document's locale, a formula as its result,
   * UTF-8 with a byte order mark and CRLF line ends. The separator is `separator`, else the one
   * of the file read (`delimiter`), else the locale's: `;` in Italian, `,` in English.
   */
  saveCsv(sheet: number, separator?: string): Uint8Array;
  /** The separator of the csv or tsv the document was read from; null for any other file. */
  delimiter(): string | null;
  /**
   * The macros of the file read, never run: `kept` for an xlsm, whose `save` writes them back
   * as an xlsm, `dropped` for an xls, `none` otherwise.
   */
  macros(): 'none' | 'kept' | 'dropped';
  /**
   * Put back the workbook as it was before the last edit; false when there is none. Every call
   * that changes the workbook is a step, up to 100 steps and 2,000,000 cells held together.
   */
  undo(): boolean;
  /** Put back the edit the last `undo` took away; false when there is none. */
  redo(): boolean;
  canUndo(): boolean;
  canRedo(): boolean;
  /** A rectangle of a sheet as it is now, cells, looks, merges and comments. */
  region(sheet: number, window: CellWindow): SheetRegion;
  /** Put back a rectangle as `region` gave it: everything in it goes, then what it held returns. */
  restoreRegion(sheet: number, region: SheetRegion): void;
  /**
   * A whole sheet as it is, for `restoreSheet`: what undoing its removal needs. An opaque text,
   * the sheet alone written as an xlsx in base64 with the looks of its cells, 16 MB at most.
   */
  sheetSnapshot(sheet: number): string;
  /**
   * Put a sheet back at `index` as `sheetSnapshot` took it; the sheets from `index` on move one
   * place right. Parts of its drawing another sheet took the names of since get new names.
   */
  restoreSheet(index: number, snapshot: string): void;
  /** Keep the workbook as it is for `rollback`: a collaborative editor keeps what the room agreed. */
  checkpoint(): void;
  /** Return to the last checkpoint, which is used up; false when there is none. */
  rollback(): boolean;
  discardCheckpoint(): void;
  hasCheckpoint(): boolean;
  /** Turn the undo history on or off; off forgets it. */
  setHistoryEnabled(enabled: boolean): void;
  /** The locale inputs are typed and shown in. */
  locale(): 'it' | 'en';
  /** Type and show in another locale from now on. */
  setLocale(locale: 'it' | 'en'): void;
  free(): void;
}
