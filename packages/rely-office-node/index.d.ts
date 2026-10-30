// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type {
  DocumentBytes,
  DocumentFormat,
  Limits,
  ErrorCode,
  ExtractedText,
  Sheet,
  Workbook,
  Mark,
  Inline,
  Comment,
  TreeOptions,
  TableCell,
  TableRow,
  Alignment,
  Block,
  MediaItem,
  ImportReport,
  Note,
  HeadersFooters,
  PageSize,
  Section,
  DocumentTree,
  DocumentFont,
  DefaultFont,
  FaceStyle,
  FontFace,
  FontGeneric,
  EmbeddedObject,
  RenderOptions,
  Rendered,
  DocxInput,
  PathPoint,
  PathCommand,
  ShapePath,
  ShapeGeometry,
  Gradient,
  Paint,
  Frame,
  SlideRun,
  SlideParagraph,
  TextBody,
  SlideTable,
  ShapeContent,
  SlideShape,
  Transition,
  Animation,
  Slide,
  Presentation,
  PptxInput,
  CellFormat,
  Border,
  CellStyle,
  StylePatch,
  BorderPlacement,
  CellRange,
  CellComment,
  CommentReply,
  PivotFunction,
  PivotLayout,
  PivotSubtotals,
  PivotValuesAxis,
  PivotFieldPlacement,
  PivotValueField,
  PivotTableDefinition,
  SheetPivotTable,
  PivotSourceField,
  DefinedName,
  ChartType,
  ChartSeries,
  ChartDefinition,
  ChartPlacement,
  SheetChart,
  CellWindow,
  SheetSummary,
  ComparisonOperator,
  ScaleValue,
  Condition,
  DifferentialLook,
  ConditionalRule,
  ConditionalFormat,
  DataValidation,
  ConditionalLook,
  CellValidation,
  SheetCell,
  SheetOptions,
  SheetDocument,
} from './model';

export * from './model';

/** Thrown by every function but `detectFormat` when a document cannot be read. */
export class RelyOfficeError extends Error {
  readonly name: 'RelyOfficeError';
  readonly code: ErrorCode;
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

/**
 * A docx, a doc, an odt, a Markdown file or an HTML page as a neutral tree of blocks for a
 * rich-text editor, with its images and a report. Markdown and HTML carry no signature, so
 * `fileName` names them (an HTML page that starts with its doctype is known without it).
 * Charts come as SVG images; the tree is written back with `writeDocx`, `writeMarkdown` or
 * `writeHtml`.
 */
export function readDocumentTree(
  bytes: DocumentBytes,
  limits?: Limits,
  options?: TreeOptions,
  fileName?: string,
): DocumentTree;

/**
 * A document (docx, xlsx, xls, ods, csv, tsv, pptx, odt, odp, doc, ppt) typeset through Typst
 * with the fonts of the package, to PDF and PNG. The typesetter loads on the first call.
 */
export function render(bytes: DocumentBytes, fileName?: string, options?: RenderOptions): Rendered;

/** A docx Word opens and `readDocumentTree` reads back to the same tree. */
export function writeDocx(tree: DocxInput): Uint8Array;

/**
 * A CSS font stack for a family: the family, the bundled family of the same metrics (Carlito
 * for Calibri and Aptos, Caladea for Cambria, Liberation for Arial, Times New Roman and Courier
 * New), then the generic family. `generic` comes from the document's font table when known.
 */
export function fontStack(family: string, generic?: FontGeneric): string;

/** The bundled family that sets a family where it is not installed, or the nearest of its kind. */
export function fontSubstitute(family: string, generic?: FontGeneric): string;

/**
 * The tree as Markdown with GitHub's tables, footnotes and math, its images in `data:`
 * addresses. Fonts, colours, alignment, merged cells, headers, footers and comments have no
 * Markdown and are left out.
 */
export function writeMarkdown(tree: DocxInput): string;

/** The tree as a page of HTML that stands alone, its images in `data:` addresses. */
export function writeHtml(tree: DocxInput, title?: string): string;

/**
 * The tree as an odt that LibreOffice opens and `readDocumentTree` reads back: looks as
 * automatic styles, headings, quotes and code in LibreOffice's named styles, sections as master
 * pages with their headers and footers, notes, comments with replies and tracked changes. An
 * equation is written as its LaTeX.
 */
export function writeOdt(tree: DocxInput): Uint8Array;

/**
 * A doc, an odt or a docx as a docx: the document read into its tree and written back, so a
 * Word 97-2003 file can be kept in the format an editor saves.
 */
export function convertToDocx(bytes: DocumentBytes, limits?: Limits): Uint8Array;

/**
 * A ppt, an odp or a pptx as a pptx: each slide's shapes, text, pictures and tables written on
 * the slide itself, over a blank layout.
 */
export function convertToPptx(bytes: DocumentBytes, limits?: Limits): Uint8Array;

/**
 * A pptx, an odp or a ppt as slides for an editor: what each slide draws from its layout and
 * master is resolved onto the slide and marked `inherited`. Written back with `writePptx`.
 */
export function readPresentation(bytes: DocumentBytes, limits?: Limits): Presentation;

/** A pptx PowerPoint opens and `readPresentation` reads back to the same slides. */
export function writePptx(presentation: PptxInput): Uint8Array;

/**
 * The slides as an odp that LibreOffice Impress opens and `readPresentation` reads back: shapes,
 * text, tables, pictures, fills, notes, hidden slides, transitions and animations. What a slide
 * draws from its layout is written on the slide itself.
 */
export function writeOdp(presentation: PptxInput): Uint8Array;

/** A new workbook with one empty sheet, Foglio1. */
export function createSheet(options?: SheetOptions): SheetDocument;

/** A workbook opened from xlsx, xls, ods, csv or tsv; a CSV is read in the locale. */
export function openSheet(bytes: DocumentBytes, fileName?: string, options?: SheetOptions): SheetDocument;

/** The version of the library inside the package. */
export const version: string;
