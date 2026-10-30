// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type { DocumentBytes, DocumentTree, DocxInput, Limits, TreeOptions } from '../model.js';
import { readDocumentTree, writeDocx, writeHtml, writeMarkdown, writeOdt } from '../runtime/documents.js';
import type { EditorDocument, EditorMappingReport, TreeToEditorOptions } from './editor-document.js';
import { editorToDocumentTree } from './editor-to-tree.js';
import { documentTreeToEditor } from './tree-to-editor.js';

/** The fonts of a tree as a writer takes them: families, embedded files and body font. */
function fontInput(tree: DocumentTree): Pick<DocxInput, 'fonts' | 'fontFiles' | 'defaultFont'> {
  return {
    ...(tree.fonts === undefined ? {} : { fonts: tree.fonts }),
    ...(tree.fontFiles === undefined ? {} : { fontFiles: tree.fontFiles }),
    ...(tree.defaultFont === undefined ? {} : { defaultFont: tree.defaultFont }),
  };
}

/** How a file is opened for the editor. */
export interface OpenDocumentOptions extends TreeToEditorOptions {
  limits?: Limits;
  /** `keep` shows tracked changes as marks the editor can accept or reject. */
  trackedChanges?: TreeOptions['trackedChanges'];
  /** The file's name, which tells a Markdown file or an HTML page from its extension. */
  fileName?: string;
}

/** The formats the editor's document is saved in. */
export type SavedDocumentFormat = 'docx' | 'odt' | 'markdown' | 'html';

/**
 * A docx, doc, odt, Markdown file or HTML page opened for the editor, once `initDocuments` has
 * loaded the module. `saveDocument` saves a docx, `saveDocumentAs` an odt, Markdown or HTML.
 */
export function openDocument(
  bytes: DocumentBytes,
  options: OpenDocumentOptions = {},
): { document: EditorDocument; report: EditorMappingReport } {
  const tree = readDocumentTree(
    bytes,
    options.limits,
    options.trackedChanges === undefined ? undefined : { trackedChanges: options.trackedChanges },
    options.fileName,
  );
  return documentTreeToEditor(tree, options);
}

/** The editor's document as a docx Word opens. */
export function saveDocument(document: EditorDocument): Uint8Array {
  const tree = editorToDocumentTree(document);
  return writeDocx({
    blocks: tree.blocks,
    media: tree.media.map((item) => item.bytes),
    page: tree.page,
    notes: tree.notes,
    headersFooters: tree.headersFooters,
    ...(tree.sections === undefined ? {} : { sections: tree.sections }),
    ...(tree.comments === undefined ? {} : { comments: tree.comments }),
    ...(tree.objects === undefined ? {} : { objects: tree.objects }),
    ...(tree.macroProject === undefined ? {} : { macroProject: tree.macroProject }),
    ...fontInput(tree),
  });
}

/**
 * The editor's document in a format of a file: a docx, an odt, Markdown (what Markdown cannot
 * say is left out: fonts, colours, alignment, merged cells, headers, footers, comments) or a
 * page of HTML titled `title`. The text formats come as UTF-8 bytes.
 */
export function saveDocumentAs(
  document: EditorDocument,
  format: SavedDocumentFormat,
  title = '',
): Uint8Array {
  if (format === 'docx') return saveDocument(document);
  const tree = editorToDocumentTree(document);
  const input = {
    blocks: tree.blocks,
    media: tree.media.map((item) => item.bytes),
    page: tree.page,
    notes: tree.notes,
    headersFooters: tree.headersFooters,
    ...(tree.sections === undefined ? {} : { sections: tree.sections }),
    ...(tree.comments === undefined ? {} : { comments: tree.comments }),
    ...fontInput(tree),
  };
  if (format === 'odt') return writeOdt(input);
  const text = format === 'markdown' ? writeMarkdown(input) : writeHtml(input, title);
  return new TextEncoder().encode(text);
}
