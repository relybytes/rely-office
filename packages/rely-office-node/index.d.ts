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
  /** Only `http:`, `https:` and `mailto:` targets are kept. */
  | { type: 'link'; href: string };

export type Inline =
  | { type: 'text'; text: string; marks?: Mark[] }
  | { type: 'hardBreak' }
  /** `media` indexes `DocumentTree.media`; sizes are CSS pixels. */
  | { type: 'image'; media: number; alt?: string; width?: number; height?: number };

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

export interface DocumentTree {
  blocks: Block[];
  media: MediaItem[];
  report: ImportReport;
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

/** The version of the library inside the package. */
export const version: string;
