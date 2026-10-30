// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import { act, render, screen, waitFor } from '@testing-library/react';
import { createRef } from 'react';
import * as Y from 'yjs';
import { Awareness } from 'y-protocols/awareness';
import { documentCollaborationExtensions } from '../src/collab/document-extensions.js';
import { SharedPresentation } from '../src/collab/slides-collab.js';
import type { CollabSession } from '../src/collab/session.js';
import {
  DocumentEditor,
  SheetGrid,
  SlideEditor,
  documentTreeToEditor,
  newTextShape,
  textBodyText,
  type DocumentEditorHandle,
  type SheetGridCollaboration,
  type SlideEditorHandle,
} from '../src/index.js';
import type { DocumentTree, Presentation } from '../src/model.js';
import { createSheet } from '../src/runtime/index.js';
import { loadWasm } from './wasm.js';

beforeAll(loadWasm);

/** Two Yjs documents that hear each other at once. */
function linked(): [Y.Doc, Y.Doc] {
  const left = new Y.Doc();
  const right = new Y.Doc();
  left.on('update', (update: Uint8Array, origin: unknown) => {
    if (origin !== 'remote') Y.applyUpdate(right, update, 'remote');
  });
  right.on('update', (update: Uint8Array, origin: unknown) => {
    if (origin !== 'remote') Y.applyUpdate(left, update, 'remote');
  });
  return [left, right];
}

/** The part of a session the document extensions read. */
function fakeSession(doc: Y.Doc): CollabSession {
  return { doc, awareness: new Awareness(doc) } as unknown as CollabSession;
}

function tree(): DocumentTree {
  return {
    blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Verbale condiviso' }] }],
    media: [],
    report: {
      truncated: false, textBoxes: 0, footnotes: 0, endnotes: 0, comments: 0, headersAndFooters: 0,
      fieldsKeptAsText: 0, charts: 0, smartArt: 0, equations: 0, shapes: 0, embeddedObjects: 0,
      unsupportedLinks: 0, oversizedImages: 0, imagesOverLimit: 0, missingImages: 0,
      trackedInsertions: 0, trackedDeletions: 0, hiddenRuns: 0,
    },
    notes: [],
    headersFooters: { header: [], footer: [] },
    page: { widthTwips: 11906, heightTwips: 16838, marginsTwips: [1440, 1440, 1440, 1440] },
    comments: [],
  };
}

describe('a document edited together', () => {
  it('seeds the shared document once and shows each change on both sides', async () => {
    const [left, right] = linked();
    const document = documentTreeToEditor(tree()).document;
    const empty = documentTreeToEditor({ ...tree(), blocks: [] }).document;
    const first = createRef<DocumentEditorHandle>();
    const second = createRef<DocumentEditorHandle>();
    const seeded = vi.fn();
    render(
      <>
        <DocumentEditor
          ref={first}
          document={document}
          collaboration={{ doc: left, extensions: documentCollaborationExtensions(fakeSession(left), { name: 'Ada', color: '#2563eb' }), slot: 0, seed: true, onSeeded: seeded }}
        />
        <DocumentEditor
          ref={second}
          document={empty}
          collaboration={{ doc: right, extensions: documentCollaborationExtensions(fakeSession(right), { name: 'Bea', color: '#15803d' }), slot: 1, seed: false }}
        />
      </>,
    );
    await waitFor(() => expect(seeded).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(second.current?.editor?.getText()).toBe('Verbale condiviso'));
    act(() => {
      second.current!.editor!.chain().focus('end').insertContent(' e approvato').run();
    });
    await waitFor(() => expect(first.current!.editor!.getText()).toBe('Verbale condiviso e approvato'));
    expect(first.current!.getTree().blocks).toEqual(second.current!.getTree().blocks);
    act(() => {
      first.current!.editor!.commands.undo();
    });
    expect(second.current!.editor!.getText()).toBe('Verbale condiviso e approvato');
  });
});

describe('a grid edited together', () => {
  it('redraws on the others changes and shows where they are', async () => {
    const sheet = await createSheet({ locale: 'it' });
    const listeners = new Set<() => void>();
    const presence = new Set<() => void>();
    const selections: { sheet: number; window: { top: number; left: number; bottom: number; right: number } }[] = [];
    let others = [{ client: 7, name: 'Bea', color: '#15803d', sheet: 0, window: { top: 2, left: 1, bottom: 2, right: 1 } }];
    const collaboration: SheetGridCollaboration = {
      subscribe: (listener) => {
        listeners.add(listener);
        return () => listeners.delete(listener);
      },
      setSelection: (selection) => {
        if (selection) selections.push(selection);
      },
      remoteSelections: () => others,
      onPresence: (listener) => {
        presence.add(listener);
        return () => presence.delete(listener);
      },
    };
    const onChange = vi.fn();
    render(<SheetGrid sheet={sheet} collaboration={collaboration} onChange={onChange} />);
    expect(screen.getAllByTitle('Bea').length).toBeGreaterThan(0);
    expect(selections.at(-1)).toEqual({ sheet: 0, window: { top: 0, left: 0, bottom: 0, right: 0 } });
    act(() => {
      sheet.setInput(0, 0, 0, 'dagli altri');
      for (const listener of listeners) listener();
    });
    expect(screen.getAllByText('dagli altri').length).toBeGreaterThan(0);
    expect(onChange).toHaveBeenCalled();
    act(() => {
      others = [];
      for (const listener of presence) listener();
    });
    expect(screen.queryAllByTitle('Bea')).toEqual([]);
    sheet.free();
  });
});

describe('slides edited together', () => {
  const WIDTH = 12_192_000;
  const HEIGHT = 6_858_000;
  const deck = (): Presentation => ({
    widthEmu: WIDTH,
    heightEmu: HEIGHT,
    truncated: false,
    media: [],
    slides: [{ shapes: [newTextShape({ x: 0, y: 0, width: 3_000_000, height: 800_000 }, 'Titolo')] }],
  });

  it('seeds once, takes in the others changes and undoes its own', async () => {
    const [left, right] = linked();
    const first = new SharedPresentation(left, 0);
    const second = new SharedPresentation(right, 1);
    const editor = createRef<SlideEditorHandle>();
    const seeded = vi.fn();
    render(<SlideEditor ref={editor} presentation={deck()} collaboration={{ shared: first, seed: true, onSeeded: seeded }} />);
    await waitFor(() => expect(seeded).toHaveBeenCalledTimes(1));
    expect(second.read().slides.length).toBe(1);

    act(() => {
      const theirs = second.read();
      second.write({ ...theirs, slides: [...theirs.slides, { shapes: [newTextShape({ x: 0, y: 0, width: 100, height: 100 }, 'Nuova')] }] });
    });
    await waitFor(() => expect(editor.current!.getPresentation().slides.length).toBe(2));
    const lastShape = editor.current!.getPresentation().slides[1]!.shapes![0]!;
    expect(lastShape.content.type === 'text' ? textBodyText(lastShape.content) : '').toBe('Nuova');
    expect(first.canUndo()).toBe(false);
  });
});
