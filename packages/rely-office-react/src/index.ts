// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

export * from './runtime/index.js';

export {
  DocumentEditor,
  type DocumentEditorHandle,
  type DocumentEditorProps,
} from './document/document-editor.js';
export { openDocument, saveDocument, type OpenDocumentOptions } from './document/document-files.js';
export type {
  DocumentAside,
  EditorDocument,
  EditorMappingReport,
  TreeToEditorOptions,
} from './document/editor-document.js';
export { editorToDocumentTree } from './document/editor-to-tree.js';
export { createRelyDocumentExtensions, type RelyDocumentExtensionOptions } from './document/extensions.js';
export {
  ENGLISH_DOCUMENT_EDITOR_LABELS,
  ITALIAN_DOCUMENT_EDITOR_LABELS,
  type DocumentEditorLabels,
} from './document/labels.js';
export { mediaDataUrl, sniffImageType } from './document/media.js';
export { documentTreeToEditor } from './document/tree-to-editor.js';

export {
  GridAxis,
  cellName,
  columnName,
  parseCellName,
  parseTabSeparated,
  toTabSeparated,
  type CellPosition,
} from './sheet/geometry.js';
export { ENGLISH_SHEET_GRID_LABELS, ITALIAN_SHEET_GRID_LABELS, type SheetGridLabels } from './sheet/labels.js';
export { SheetGrid, type SheetGridHandle, type SheetGridProps } from './sheet/sheet-grid.js';

export { bodyFromEditedElement } from './slides/edited-text.js';
export { ENGLISH_SLIDE_EDITOR_LABELS, ITALIAN_SLIDE_EDITOR_LABELS, type SlideEditorLabels } from './slides/labels.js';
export { SlideEditor, type SlideEditorHandle, type SlideEditorProps } from './slides/slide-editor.js';
export * from './slides/slide-model.js';
export { ShapeView, SlideView, TextBodyView, type MediaUrls } from './slides/slide-view.js';
