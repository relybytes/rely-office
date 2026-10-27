// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

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

/** Thrown by every function but `detectFormat` when a document cannot be read. */
export class RelyOfficeError extends Error {
  readonly name: 'RelyOfficeError';
  readonly code: ErrorCode;
}

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
  | { type: 'link'; href: string };

export type Inline =
  | { type: 'text'; text: string; marks?: Mark[] }
  | { type: 'hardBreak' }
  /** `media` indexes `DocumentTree.media`; sizes are CSS pixels. */
  | { type: 'image'; media: number; alt?: string; width?: number; height?: number }
  /** A footnote or endnote: `note` indexes `DocumentTree.notes`. */
  | { type: 'noteReference'; note: number };

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

export interface DocumentTree {
  blocks: Block[];
  media: MediaItem[];
  report: ImportReport;
  /** The footnotes and endnotes, in the order the text refers to them. */
  notes: Note[];
  headersFooters: HeadersFooters;
  page: PageSize;
}

/** The format of a document; the name only tells CSV from TSV and never overrides the bytes. */
export function detectFormat(bytes: DocumentBytes, fileName?: string): DocumentFormat;

/** The text of a document for a search index. */
export function extractText(
  bytes: DocumentBytes,
  fileName?: string,
  limits?: Limits,
): ExtractedText;

/** The sheets of a spreadsheet (xlsx, xls, ods, csv, tsv) as tables of displayed values. */
export function readSheets(bytes: DocumentBytes, fileName?: string, limits?: Limits): Workbook;

/** A docx as a neutral tree of blocks for a rich-text editor, with its images and a report. */
export function readDocumentTree(bytes: DocumentBytes, limits?: Limits): DocumentTree;

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
   * `layout`: the document's own structure and page (docx, spreadsheets); `text`: its text on
   * A4, for the formats with no layout reader yet (pptx, odt, odp, doc, ppt).
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

/**
 * A document (docx, xlsx, xls, ods, csv, tsv, pptx, odt, odp, doc, ppt) typeset through Typst
 * with the fonts of the package, to PDF and PNG. The typesetter loads on the first call.
 */
export function render(bytes: DocumentBytes, fileName?: string, options?: RenderOptions): Rendered;

/** A document tree to write as a docx: its blocks, the images they name by index, its page. */
export interface DocxInput {
  blocks: Block[];
  /** The bytes of each image; `Inline.media` indexes this list. The type is read from the bytes. */
  media?: DocumentBytes[];
  /** A4 with 2.5 cm margins when left out. */
  page?: PageSize;
  notes?: Note[];
  headersFooters?: HeadersFooters;
}

/** A docx Word opens and `readDocumentTree` reads back to the same tree. */
export function writeDocx(tree: DocxInput): Uint8Array;

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
  /** Today's date for TODAY() and NOW(), which WebAssembly cannot read; null unsets it. */
  setToday(today: Date | number | null): void;
  /** How a typed text would be read, without typing it. */
  previewInput(text: string): 'number' | 'text' | 'bool' | 'formula' | 'empty' | 'invalid';
  /** The workbook as xlsx, each formula with its computed value. */
  save(): Uint8Array;
  free(): void;
}

/** A new workbook with one empty sheet, Foglio1. */
export function createSheet(options?: SheetOptions): SheetDocument;

/** A workbook opened from xlsx, xls, ods, csv or tsv; a CSV is read in the locale. */
export function openSheet(bytes: DocumentBytes, fileName?: string, options?: SheetOptions): SheetDocument;

/** The version of the library inside the package. */
export const version: string;
