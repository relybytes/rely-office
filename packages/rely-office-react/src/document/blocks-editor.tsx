// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import { EditorContent, useEditor } from '@tiptap/react';
import { useMemo, useRef, type ReactElement } from 'react';
import type { Block, MediaItem } from '../model.js';
import { editorContentToBlocks } from './editor-to-tree.js';
import { createRelyDocumentExtensions } from './extensions.js';
import { blocksToEditorContent } from './tree-to-editor.js';

/**
 * A small editor over a list of blocks outside the body: a header, a footer, a note, a comment.
 * It has the schema of the document editor, so a header keeps its table and its logo, and the
 * shortcuts of the marks; it has no toolbar of its own.
 */
export function BlocksEditor({
  blocks,
  media,
  imageSource,
  onChange,
  editable = true,
  label,
  autoFocus = false,
}: Readonly<{
  blocks: Block[];
  media: MediaItem[];
  imageSource?: (item: MediaItem) => string;
  onChange: (blocks: Block[]) => void;
  editable?: boolean;
  label: string;
  autoFocus?: boolean;
}>): ReactElement {
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const extensions = useMemo(() => createRelyDocumentExtensions(), []);
  const editor = useEditor({
    extensions,
    content: blocksToEditorContent(blocks, media, { imageSource }),
    editable,
    autofocus: autoFocus ? 'end' : false,
    immediatelyRender: false,
    editorProps: { attributes: { class: 'rely-blocks-content', role: 'textbox', 'aria-label': label, 'aria-multiline': 'true' } },
    onUpdate: ({ editor: updated }) => onChangeRef.current(editorContentToBlocks(updated.getJSON())),
  });
  return <EditorContent editor={editor} className="rely-blocks-editor" />;
}
