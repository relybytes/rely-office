// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type { AnyExtension, Editor } from '@tiptap/core';
import { EditorContent, useEditor } from '@tiptap/react';
import {
  forwardRef,
  useCallback,
  useImperativeHandle,
  useMemo,
  useRef,
  type ReactElement,
  type Ref,
} from 'react';
import type { Block, DocumentTree, MediaItem } from '../model.js';
import { saveDocument } from './document-files.js';
import { DocumentToolbar } from './document-toolbar.js';
import type { EditorDocument } from './editor-document.js';
import { editorToDocumentTree } from './editor-to-tree.js';
import { createRelyDocumentExtensions } from './extensions.js';
import { ENGLISH_DOCUMENT_EDITOR_LABELS, type DocumentEditorLabels } from './labels.js';
import { mediaDataUrl, sniffImageType } from './media.js';

/** What a host can ask of a mounted document editor. */
export interface DocumentEditorHandle {
  /** The document as it is now, the images inserted since it opened included. */
  getDocument: () => EditorDocument;
  /** The document as a tree, for `writeDocx` or for the host's own storage. */
  getTree: () => DocumentTree;
  /** The document as a docx; needs `initDocuments` to have loaded the module. */
  saveDocx: () => Uint8Array;
  /** The TipTap editor, for commands the toolbar does not offer. */
  editor: Editor | null;
}

export interface DocumentEditorProps {
  /** The document to open; the editor keeps its own state after that, so key it to open another. */
  document: EditorDocument;
  /** Called after every change with the document as it is now. */
  onChange?: (document: EditorDocument) => void;
  /** False shows the document without letting it change. */
  editable?: boolean;
  /** False hides the toolbar, for a host that draws its own over `handle.editor`. */
  toolbar?: boolean;
  /** The words of the toolbar, in the host's language; English by default. */
  labels?: Partial<DocumentEditorLabels>;
  /**
   * The bytes of an image the user chose, or null to refuse it; by default the file's bytes.
   * A host checks the size or uploads the file here.
   */
  onImageFile?: (file: File) => Promise<Uint8Array | null>;
  /** The `src` an inserted image is shown with; by default a `data:` URL of its bytes. */
  imageSource?: (item: MediaItem) => string;
  /** The host's extensions, added after the ones the tree needs. */
  extensions?: AnyExtension[];
  className?: string;
}

function blocksToText(blocks: Block[]): string {
  return blocks
    .map((block) => {
      switch (block.type) {
        case 'heading':
        case 'paragraph':
        case 'listItem':
          return block.content.map((inline) => (inline.type === 'text' ? inline.text : '')).join('');
        case 'blockquote':
          return blocksToText(block.blocks);
        case 'codeBlock':
          return block.text;
        case 'table':
          return block.rows.map((row) => row.cells.map((cell) => blocksToText(cell.blocks)).join(' ')).join('\n');
        default:
          return '';
      }
    })
    .join('\n');
}

/** The bytes of a chosen file; FileReader where `Blob.arrayBuffer` is missing (Safari before 14). */
function readImageFile(file: File): Promise<Uint8Array | null> {
  if (typeof file.arrayBuffer === 'function') {
    return file.arrayBuffer().then((buffer) => new Uint8Array(buffer));
  }
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer));
    reader.onerror = () => reject(reader.error);
    reader.readAsArrayBuffer(file);
  });
}

/**
 * A rich-text editor over the document tree: every block, mark and inline of a docx, doc or odt
 * has a place in it, and what it does not show as text (notes, comments, page, headers,
 * sections, objects) is written back as it came in.
 */
export const DocumentEditor = forwardRef(function DocumentEditor(
  {
    document,
    onChange,
    editable = true,
    toolbar = true,
    labels,
    onImageFile = readImageFile,
    imageSource = mediaDataUrl,
    extensions,
    className,
  }: DocumentEditorProps,
  ref: Ref<DocumentEditorHandle>,
): ReactElement {
  const words = useMemo(() => ({ ...ENGLISH_DOCUMENT_EDITOR_LABELS, ...labels }), [labels]);
  const media = useRef<MediaItem[]>([...document.media]);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const allExtensions = useMemo(() => createRelyDocumentExtensions({ extensions }), [extensions]);

  const current = useCallback(
    (editor: Editor): EditorDocument => ({
      content: editor.getJSON(),
      media: media.current,
      aside: document.aside,
    }),
    [document.aside],
  );

  const editor = useEditor({
    extensions: allExtensions,
    content: document.content,
    editable,
    immediatelyRender: false,
    editorProps: { attributes: { class: 'rely-document-content' } },
    onUpdate: ({ editor: updated }) => onChangeRef.current?.(current(updated)),
  });

  const insertImage = useCallback(
    async (file: File) => {
      if (!editor) return;
      const bytes = await onImageFile(file);
      if (bytes === null) return;
      const index = media.current.reduce((highest, item) => Math.max(highest, item.index + 1), 0);
      const item: MediaItem = { index, contentType: sniffImageType(bytes), bytes };
      media.current = [...media.current, item];
      editor
        .chain()
        .focus()
        .insertContent({ type: 'image', attrs: { src: imageSource(item), alt: file.name, media: index } })
        .run();
    },
    [editor, onImageFile, imageSource],
  );

  useImperativeHandle(
    ref,
    () => ({
      getDocument: () =>
        editor ? current(editor) : { ...document, media: media.current },
      getTree: () => editorToDocumentTree(editor ? current(editor) : document),
      saveDocx: () => saveDocument(editor ? current(editor) : document),
      editor,
    }),
    [editor, current, document],
  );

  const comments = document.aside.comments ?? [];
  return (
    <div className={['rely-document-editor', className].filter(Boolean).join(' ')}>
      {toolbar && editable && editor ? (
        <DocumentToolbar editor={editor} labels={words} onImage={insertImage} />
      ) : null}
      <div className="rely-document-body">
        <EditorContent editor={editor} className="rely-document-page" />
        {comments.length > 0 ? (
          <aside className="rely-document-comments" aria-label={words.comments}>
            <h2>{words.comments}</h2>
            <ol>
              {comments.map((comment, index) => (
                <li key={index} data-comment={index} className={comment.resolved ? 'rely-resolved' : undefined}>
                  <strong>{comment.author}</strong>
                  {comment.date ? <time dateTime={comment.date}> {comment.date.slice(0, 10)}</time> : null}
                  {comment.resolved ? <span> ({words.resolved})</span> : null}
                  <p>{blocksToText(comment.blocks)}</p>
                </li>
              ))}
            </ol>
          </aside>
        ) : null}
      </div>
    </div>
  );
});
