// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import type { AnyExtension, Editor } from '@tiptap/core';
import { EditorContent, useEditor } from '@tiptap/react';
import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactElement,
  type Ref,
} from 'react';
import type * as Y from 'yjs';
import { SharedDocumentAside } from '../collab/document-collab.js';
import type { Block, Comment, DocumentTree, Inline, MediaItem, Note } from '../model.js';
import { saveDocument } from './document-files.js';
import { DocumentSidePanel } from './document-side-panel.js';
import { DocumentToolbar } from './document-toolbar.js';
import type { DocumentAside, EditorDocument } from './editor-document.js';
import { editorToDocumentTree } from './editor-to-tree.js';
import { createRelyDocumentExtensions } from './extensions.js';
import { ENGLISH_DOCUMENT_EDITOR_LABELS, type DocumentEditorLabels } from './labels.js';
import { mediaDataUrl, sniffImageType } from './media.js';
import { DEFAULT_PAGINATION, type PaginationOptions } from './pagination.js';
import { contentReferences, type ContentReferences } from './references.js';
import { fontStack, loadDocumentFonts, usedFamilies } from './fonts.js';

/** Twentieths of a point as CSS pixels. */
const TWIPS_PER_PIXEL = 15;

/**
 * The text of header or footer blocks, a line per paragraph, for the page view to draw; with
 * `page`, the page number and the page count they show are those of that page.
 */
export function blocksText(blocks: Block[], page?: { page: number; pages: number }): string {
  const text = (part: Inline): string => {
    if (part.type === 'hardBreak') return ' ';
    if (part.type !== 'text') return '';
    if (page && part.field === 'page') return String(page.page);
    if (page && part.field === 'pages') return String(page.pages);
    return part.text;
  };
  const inline = (block: Block): string => {
    switch (block.type) {
      case 'paragraph':
      case 'heading':
      case 'listItem':
        return block.content.map(text).join('');
      case 'blockquote':
        return block.blocks.map(inline).join(' ');
      case 'table':
        return block.rows.map((row) => row.cells.map((cell) => cell.blocks.map(inline).join(' ')).join('  ')).join(' ');
      case 'codeBlock':
        return block.text;
      default:
        return '';
    }
  };
  return blocks.map(inline).filter((line) => line.trim().length > 0).join('  ');
}

/** Whether a header or a footer shows the page number or the page count itself. */
function hasPageField(blocks: Block[]): boolean {
  return blocks.some((block) => {
    switch (block.type) {
      case 'paragraph':
      case 'heading':
      case 'listItem':
        return block.content.some((part) => part.type === 'text' && part.field !== undefined);
      case 'blockquote':
        return hasPageField(block.blocks);
      case 'table':
        return block.rows.some((row) => row.cells.some((cell) => hasPageField(cell.blocks)));
      default:
        return false;
    }
  });
}

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

/** A document edited together: what binds the editor to a collaboration session. */
export interface DocumentCollaboration {
  /** The session's shared document; its fragment `content` holds the text. */
  doc: Y.Doc;
  /** From `documentCollaborationExtensions` of `@relybytes/rely-office-react/collab-editor`. */
  extensions: AnyExtension[];
  /** The slot of the session's welcome, so the comments and images it adds keep their own indexes. */
  slot: number;
  /** Put `document` into the shared document when it is empty: the welcome said this client seeds. */
  seed: boolean;
  /** Called once this client put the document in. */
  onSeeded?: () => void;
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
  /** Who writes the comments and replies added here; `labels.you` by default. */
  author?: { name: string; initials?: string };
  /** False hides the panel of comments, notes and headers, for a host that edits them itself. */
  sidePanel?: boolean;
  /** Open in the page view, the text on pages of the document's size with their headers. */
  pageView?: boolean;
  /** Open recording tracked changes. */
  trackChanges?: boolean;
  /**
   * Edit a shared document instead of `document`: the text, comments, notes and images are the
   * session's, and `document` only seeds an empty one. Undo takes back this client's changes.
   */
  collaboration?: DocumentCollaboration;
  className?: string;
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
    author,
    sidePanel = true,
    pageView: initialPageView = false,
    trackChanges: initialTrackChanges = false,
    collaboration,
    className,
  }: DocumentEditorProps,
  ref: Ref<DocumentEditorHandle>,
): ReactElement {
  const words = useMemo(() => ({ ...ENGLISH_DOCUMENT_EDITOR_LABELS, ...labels }), [labels]);
  const sharedDoc = collaboration?.doc;
  const slot = collaboration?.slot ?? 0;
  const shared = useMemo(() => (sharedDoc ? new SharedDocumentAside(sharedDoc, slot) : null), [sharedDoc, slot]);
  const sharedFilled = shared !== null && !shared.isEmpty();
  const media = useRef<MediaItem[]>(sharedFilled ? shared.readMedia() : [...document.media]);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const writer = author ?? { name: words.you };
  const writerName = useRef(writer.name);
  writerName.current = writer.name;
  const collaborationExtensions = collaboration?.extensions;
  const allExtensions = useMemo(
    () =>
      createRelyDocumentExtensions({
        extensions: [...(extensions ?? []), ...(collaborationExtensions ?? [])],
        author: () => writerName.current,
        history: collaborationExtensions === undefined,
      }),
    [extensions, collaborationExtensions],
  );
  const [aside, setAsideState] = useState<DocumentAside>(() =>
    sharedFilled ? shared.readAside(document.aside) : document.aside,
  );
  const asideRef = useRef(aside);
  const [references, setReferences] = useState<ContentReferences>(() => contentReferences(document.content));
  const [focusComment, setFocusComment] = useState<number | null>(null);
  const [pageView, setPageView] = useState(initialPageView);
  const [trackChanges, setTrackChanges] = useState(initialTrackChanges);
  const [findRequest, setFindRequest] = useState(0);
  const [pages, setPages] = useState(1);

  const current = useCallback(
    (editor: Editor): EditorDocument => ({
      content: editor.getJSON(),
      media: media.current,
      aside: asideRef.current,
    }),
    [],
  );

  const editor = useEditor({
    extensions: allExtensions,
    content: collaboration ? undefined : document.content,
    editable,
    immediatelyRender: false,
    editorProps: { attributes: { class: 'rely-document-content' } },
    onUpdate: ({ editor: updated }) => {
      const now = current(updated);
      setReferences(contentReferences(now.content));
      onChangeRef.current?.(now);
    },
  });

  const asideFonts = aside.fonts;
  const asideFontFiles = aside.fontFiles;
  useEffect(() => {
    if (!asideFonts?.length || !asideFontFiles?.length) return;
    let release: (() => void) | null = null;
    let cancelled = false;
    void loadDocumentFonts(asideFonts, asideFontFiles).then((loaded) => {
      if (cancelled) loaded.release();
      else release = loaded.release;
    });
    return () => {
      cancelled = true;
      release?.();
    };
  }, [asideFonts, asideFontFiles]);
  const documentFamilies = useMemo(
    () => [...(asideFonts ?? []).map((font) => font.name), ...usedFamilies(editor?.getJSON() ?? document.content)],
    // The families in the text are read when the list of the file changes or the editor opens.
    [asideFonts, editor, document.content],
  );

  const page = aside.page;
  const margins = {
    top: page.marginsTwips[0] / TWIPS_PER_PIXEL,
    right: page.marginsTwips[1] / TWIPS_PER_PIXEL,
    bottom: page.marginsTwips[2] / TWIPS_PER_PIXEL,
    left: page.marginsTwips[3] / TWIPS_PER_PIXEL,
  };
  const pageHeight = page.heightTwips / TWIPS_PER_PIXEL;
  const headerBlocks = aside.headersFooters.header;
  const footerBlocks = aside.headersFooters.footer;
  const header = useCallback((page: number, count: number) => blocksText(headerBlocks, { page, pages: count }), [headerBlocks]);
  const footer = useCallback((page: number, count: number) => blocksText(footerBlocks, { page, pages: count }), [footerBlocks]);
  const showsPageNumber = useMemo(() => hasPageField([...headerBlocks, ...footerBlocks]), [headerBlocks, footerBlocks]);

  useEffect(() => {
    if (!editor) return;
    const storage = (editor.storage as unknown as Record<string, { options: PaginationOptions; pages: number }>).relyPagination;
    storage.options = {
      ...DEFAULT_PAGINATION,
      enabled: pageView,
      contentHeight: Math.max(48, pageHeight - margins.top - margins.bottom),
      marginTop: margins.top,
      marginBottom: margins.bottom,
      header,
      footer,
      pageLabel: showsPageNumber ? null : words.pageNumber,
    };
    editor.view.dispatch(editor.state.tr.setMeta('relyTrackSkip', true).setMeta('addToHistory', false));
    const update = () => setPages(storage.pages);
    editor.on('transaction', update);
    return () => {
      editor.off('transaction', update);
    };
  }, [editor, pageView, pageHeight, margins.top, margins.bottom, header, footer, showsPageNumber, words.pageNumber]);

  useEffect(() => {
    editor?.commands.setTrackChanges(trackChanges);
  }, [editor, trackChanges]);

  const setAside = useCallback(
    (next: DocumentAside) => {
      const written = shared ? shared.writeAside(asideRef.current, next) : next;
      asideRef.current = written;
      setAsideState(written);
      if (editor) onChangeRef.current?.(current(editor));
    },
    [editor, current, shared],
  );

  useEffect(() => {
    if (!shared) return;
    return shared.observe(() => {
      const next = shared.readAside(asideRef.current);
      asideRef.current = next;
      setAsideState(next);
      media.current = shared.readMedia();
      if (editor) onChangeRef.current?.(current(editor));
    });
  }, [shared, editor, current]);

  const seedRequested = collaboration?.seed ?? false;
  const onSeededRef = useRef(collaboration?.onSeeded);
  onSeededRef.current = collaboration?.onSeeded;
  const seedSource = useRef(document);
  const seededRef = useRef(false);
  useEffect(() => {
    if (!editor || !shared || !seedRequested || seededRef.current) return;
    seededRef.current = true;
    const source = seedSource.current;
    if (shared.isEmpty()) {
      shared.seed(source);
      editor
        .chain()
        .command(({ tr }) => {
          tr.setMeta('addToHistory', false);
          return true;
        })
        .setContent(source.content)
        .run();
      const seeded = shared.readAside(source.aside);
      asideRef.current = seeded;
      setAsideState(seeded);
      media.current = shared.readMedia();
    }
    onSeededRef.current?.();
  }, [editor, shared, seedRequested]);

  const addComment = useCallback(() => {
    if (!editor || editor.state.selection.empty) return;
    const comments = asideRef.current.comments ?? [];
    const index = shared ? shared.allocate('comment', comments.length) : comments.length;
    const next: Comment[] = [...comments];
    while (next.length < index) next.push({ author: '', date: '', blocks: [], resolved: true });
    next[index] = {
      author: writer.name,
      ...(writer.initials ? { initials: writer.initials } : {}),
      date: new Date().toISOString(),
      blocks: [{ type: 'paragraph', content: [] }],
      resolved: false,
    };
    setAside({ ...asideRef.current, comments: next });
    setFocusComment(index);
    editor.chain().focus().setMark('comment', { comment: index }).run();
  }, [editor, setAside, writer.name, writer.initials, shared]);

  const insertFootnote = useCallback(() => {
    if (!editor) return;
    const notes = asideRef.current.notes;
    const index = shared ? shared.allocate('note', notes.length) : notes.length;
    const next: Note[] = [...notes];
    while (next.length < index) next.push({ kind: 'footnote', blocks: [] });
    next[index] = { kind: 'footnote', blocks: [{ type: 'paragraph', content: [] }] };
    setAside({ ...asideRef.current, notes: next });
    editor.chain().focus().insertNoteReference(index).run();
  }, [editor, setAside, shared]);

  const insertImage = useCallback(
    async (file: File) => {
      if (!editor) return;
      const bytes = await onImageFile(file);
      if (bytes === null) return;
      const next = media.current.reduce((highest, item) => Math.max(highest, item.index + 1), 0);
      const index = shared ? shared.allocate('media', next) : next;
      const item: MediaItem = { index, contentType: sniffImageType(bytes), bytes };
      media.current = [...media.current, item];
      shared?.addMedia(item);
      editor
        .chain()
        .focus()
        .insertContent({ type: 'image', attrs: { src: imageSource(item), alt: file.name, media: index } })
        .run();
    },
    [editor, onImageFile, imageSource, shared],
  );

  useImperativeHandle(
    ref,
    () => ({
      getDocument: () => (editor ? current(editor) : { ...document, media: media.current, aside: asideRef.current }),
      getTree: () => editorToDocumentTree(editor ? current(editor) : { ...document, aside: asideRef.current }),
      saveDocx: () => saveDocument(editor ? current(editor) : { ...document, aside: asideRef.current }),
      editor,
    }),
    [editor, current, document],
  );

  const defaultFont = aside.defaultFont;
  const fontStyle = {
    ...(defaultFont?.family ? { '--rely-document-font': fontStack(defaultFont.family) } : {}),
    ...(defaultFont?.sizeHalfPoints ? { '--rely-document-size': `${defaultFont.sizeHalfPoints / 2}pt` } : {}),
  } as CSSProperties;
  const pageStyle: CSSProperties | undefined = pageView
    ? ({
        ...fontStyle,
        '--rely-page-width': `${page.widthTwips / TWIPS_PER_PIXEL}px`,
        '--rely-page-margin-top': `${margins.top}px`,
        '--rely-page-margin-right': `${margins.right}px`,
        '--rely-page-margin-bottom': `${margins.bottom}px`,
        '--rely-page-margin-left': `${margins.left}px`,
        '--rely-page-height': `${pageHeight}px`,
      } as CSSProperties)
    : fontStyle;

  return (
    <div
      className={['rely-document-editor', pageView ? 'rely-page-view' : '', className].filter(Boolean).join(' ')}
      style={pageStyle}
      onKeyDown={(event) => {
        const key = event.key.toLowerCase();
        if ((event.ctrlKey || event.metaKey) && (key === 'f' || key === 'h') && editable && toolbar) {
          event.preventDefault();
          setFindRequest((count) => count + 1);
        }
      }}
    >
      {toolbar && editable && editor ? (
        <DocumentToolbar
          editor={editor}
          labels={words}
          onImage={insertImage}
          onComment={addComment}
          onFootnote={insertFootnote}
          trackChanges={trackChanges}
          onTrackChanges={setTrackChanges}
          pageView={pageView}
          onPageView={setPageView}
          findRequest={findRequest}
          documentFamilies={documentFamilies}
        />
      ) : null}
      <div className="rely-document-body">
        <div className="rely-document-page">
          {pageView ? (
            <div className="rely-page-header rely-page-header-first" aria-hidden="true">
              {header(1, pages)}
            </div>
          ) : null}
          <EditorContent editor={editor} />
          {pageView ? (
            <div className="rely-page-footer rely-page-footer-last" aria-hidden="true">
              {footer(pages, pages)}
              {showsPageNumber ? null : <span className="rely-page-number">{words.pageNumber(pages)}</span>}
            </div>
          ) : null}
        </div>
        {sidePanel ? (
          <DocumentSidePanel
            aside={aside}
            references={references}
            media={media.current}
            imageSource={imageSource}
            editable={editable}
            labels={words}
            author={writer}
            focusComment={focusComment}
            onAsideChange={setAside}
            onDeleteComments={(comments) => editor?.chain().focus().removeComments(comments).run()}
            onDeleteNote={(note) => editor?.chain().focus().removeNoteReferences([note]).run()}
          />
        ) : null}
      </div>
    </div>
  );
});
