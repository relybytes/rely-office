// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type { DocumentBytes, Limits, TreeOptions } from '../model.js';
import { readDocumentTree, writeDocx } from '../runtime/documents.js';
import type { EditorDocument, EditorMappingReport, TreeToEditorOptions } from './editor-document.js';
import { editorToDocumentTree } from './editor-to-tree.js';
import { documentTreeToEditor } from './tree-to-editor.js';

/** How a file is opened for the editor. */
export interface OpenDocumentOptions extends TreeToEditorOptions {
  limits?: Limits;
  /** `keep` shows tracked changes as marks the editor can accept or reject. */
  trackedChanges?: TreeOptions['trackedChanges'];
}

/**
 * A docx, doc or odt opened for the editor, once `initDocuments` has loaded the module. A doc and
 * an odt are saved back as docx.
 */
export function openDocument(
  bytes: DocumentBytes,
  options: OpenDocumentOptions = {},
): { document: EditorDocument; report: EditorMappingReport } {
  const tree = readDocumentTree(
    bytes,
    options.limits,
    options.trackedChanges === undefined ? undefined : { trackedChanges: options.trackedChanges },
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
  });
}
