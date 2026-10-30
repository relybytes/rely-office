// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

export * from './runtime/index.js';

export {
  DocumentEditor,
  type DocumentCollaboration,
  type DocumentEditorHandle,
  type DocumentEditorProps,
} from './document/document-editor.js';
export {
  openDocument,
  saveDocument,
  saveDocumentAs,
  type OpenDocumentOptions,
  type SavedDocumentFormat,
} from './document/document-files.js';
export type {
  DocumentAside,
  EditorDocument,
  EditorMappingReport,
  TreeToEditorOptions,
} from './document/editor-document.js';
export { BlocksEditor } from './document/blocks-editor.js';
export { DocumentSidePanel } from './document/document-side-panel.js';
export { editorContentToBlocks, editorToDocumentTree } from './document/editor-to-tree.js';
export {
  compactComments,
  compactNotes,
  contentReferences,
  threadRoot,
  type ContentReferences,
} from './document/references.js';
export { createRelyDocumentExtensions, type RelyDocumentExtensionOptions } from './document/extensions.js';
export { FONT_FAMILIES, FONT_SIZES, LINE_SPACINGS, RelyParagraphFormat, cleanFormat } from './document/formatting.js';
export {
  BUNDLED_FONT_FAMILIES,
  fontGeneric,
  fontStack,
  fontSubstitute,
  isFontAvailable,
  loadDocumentFonts,
  pickerFamilies,
  stackFamily,
  usedFamilies,
  type LoadedFaces,
} from './document/fonts.js';
export { RelyPagination, pageBreaks, type PaginationOptions } from './document/pagination.js';
export { RelySearch, findMatches, type SearchMatch, type SearchState } from './document/search.js';
export { RelyTrackRecording, TRACK_SKIP } from './document/track-changes.js';
export {
  ENGLISH_DOCUMENT_EDITOR_LABELS,
  ITALIAN_DOCUMENT_EDITOR_LABELS,
  type DocumentEditorLabels,
} from './document/labels.js';
export { mapBlockImages, mapBlockInlines, mediaDataUrl, sniffImageType } from './document/media.js';
export { blocksToEditorContent, documentTreeToEditor } from './document/tree-to-editor.js';

export {
  GridAxis,
  cellName,
  columnName,
  fillTarget,
  parseCellName,
  parseTabSeparated,
  toTabSeparated,
  type CellPosition,
} from './sheet/geometry.js';
export { ENGLISH_SHEET_GRID_LABELS, ITALIAN_SHEET_GRID_LABELS, type SheetGridLabels } from './sheet/labels.js';
export {
  SheetGrid,
  type SheetGridCollaboration,
  type SheetGridHandle,
  type SheetGridProps,
} from './sheet/sheet-grid.js';
export { acceptFunction, functionTokenAt, suggestFunctions, type FunctionToken } from './sheet/formula-hints.js';

export { bodyFromEditedElement } from './slides/edited-text.js';
export { ENGLISH_SLIDE_EDITOR_LABELS, ITALIAN_SLIDE_EDITOR_LABELS, type SlideEditorLabels } from './slides/labels.js';
export {
  SlideEditor,
  type SlideCollaboration,
  type SlideEditorHandle,
  type SlideEditorProps,
} from './slides/slide-editor.js';
export * from './slides/slide-model.js';
export { SlideEffectsPanel } from './slides/slide-effects-panel.js';
export { SlideShow, type SlideShowProps } from './slides/slide-show.js';
export * from './slides/slide-tools.js';
export { ShapeView, SlideView, TextBodyView, type MediaUrls, type TableCellPlace } from './slides/slide-view.js';

export { ENGLISH_TEXT_EDITOR_LABELS, ITALIAN_TEXT_EDITOR_LABELS, type TextEditorLabels } from './text/labels.js';
export {
  TextEditor,
  shiftOffset,
  textChange,
  type TextCollaboration,
  type TextEditorHandle,
  type TextEditorProps,
} from './text/text-editor.js';
export {
  dominantLineEnding,
  lineAndColumn,
  readTextFile,
  textNeedsUnicode,
  writeTextFile,
  type LineEnding,
  type TextEncodingName,
  type TextFile,
} from './text/text-files.js';
