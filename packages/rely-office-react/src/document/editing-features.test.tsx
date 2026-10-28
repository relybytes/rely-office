// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { getSchema } from '@tiptap/core';
import { Fragment } from '@tiptap/pm/model';
import { createRef } from 'react';
import type { Block, DocumentTree } from '../model.js';
import { DocumentEditor, blocksText, type DocumentEditorHandle } from './document-editor.js';
import { editorToDocumentTree } from './editor-to-tree.js';
import { createRelyDocumentExtensions } from './extensions.js';
import { cleanFormat, formatHtml, lineHeightCss } from './formatting.js';
import { resizedImage } from './image-view.js';
import { pageBreaks } from './pagination.js';
import { findMatches } from './search.js';
import { markDeleted } from './track-changes.js';
import { documentTreeToEditor } from './tree-to-editor.js';

const PNG = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

function tree(blocks: Block[]): DocumentTree {
  return {
    blocks,
    media: [{ index: 0, contentType: 'image/png', bytes: PNG }],
    report: {
      truncated: false, textBoxes: 0, footnotes: 0, endnotes: 0, comments: 0, headersAndFooters: 0,
      fieldsKeptAsText: 0, charts: 0, smartArt: 0, equations: 0, shapes: 0, embeddedObjects: 0,
      unsupportedLinks: 0, oversizedImages: 0, imagesOverLimit: 0, missingImages: 0,
      trackedInsertions: 0, trackedDeletions: 0, hiddenRuns: 0,
    },
    notes: [],
    headersFooters: { header: [{ type: 'paragraph', content: [{ type: 'text', text: 'Riservato' }] }], footer: [] },
    page: { widthTwips: 11906, heightTwips: 16838, marginsTwips: [1440, 1440, 1440, 1440] },
  };
}

const TEXT: Block[] = [
  { type: 'paragraph', content: [{ type: 'text', text: 'Il gatto e il cane' }] },
  { type: 'paragraph', content: [{ type: 'text', text: 'Un altro Gatto' }] },
];

async function mount(blocks: Block[] = TEXT, props: Partial<Parameters<typeof DocumentEditor>[0]> = {}) {
  const ref = createRef<DocumentEditorHandle>();
  render(<DocumentEditor ref={ref} document={documentTreeToEditor(tree(blocks)).document} author={{ name: 'Anna' }} {...props} />);
  await waitFor(() => expect(ref.current?.editor).toBeTruthy());
  return { ref, editor: ref.current!.editor! };
}

describe('paragraph formats', () => {
  it('keeps only what a format sets and draws it as CSS', () => {
    expect(cleanFormat({ style: null, pageBreakBefore: false, spaceAfterTwips: 120 })).toEqual({ spaceAfterTwips: 120 });
    expect(cleanFormat({ style: null })).toBeNull();
    expect(lineHeightCss({ rule: 'auto', value: 480 })).toBe('2.4');
    expect(lineHeightCss({ rule: 'exact', value: 300 })).toBe('15pt');
    expect(formatHtml({ style: 'title', indentFirstLineTwips: 720, pageBreakBefore: true })).toMatchObject({
      style: 'text-indent: 36pt',
      class: 'rely-style-title rely-page-break-before',
    });
    expect(formatHtml(null)).toEqual({});
  });

  it('goes into the editor and back with a paragraph, a list item and table widths', () => {
    const blocks: Block[] = [
      { type: 'paragraph', content: [{ type: 'text', text: 'a' }], format: { style: 'caption', lineSpacing: { rule: 'auto', value: 360 } } },
      { type: 'heading', level: 1, content: [{ type: 'text', text: 'T' }], format: { style: 'title' } },
      { type: 'listItem', ordered: false, level: 0, listId: 1, content: [{ type: 'text', text: 'b' }], format: { spaceAfterTwips: 0 } },
      {
        type: 'table',
        columnWidthsTwips: [1500, 3000],
        rows: [{ header: false, cells: [{ colSpan: 1, rowSpan: 1, blocks: [] }, { colSpan: 1, rowSpan: 1, blocks: [] }] }],
      },
    ];
    const back = editorToDocumentTree(documentTreeToEditor(tree(blocks)).document).blocks;
    expect(back[0]).toMatchObject({ format: blocks[0].type === 'paragraph' ? blocks[0].format : undefined });
    expect(back[1]).toMatchObject({ type: 'heading', format: { style: 'title' } });
    expect(back[2]).toMatchObject({ type: 'listItem', format: { spaceAfterTwips: 0 } });
    expect(back[3]).toMatchObject({ type: 'table', columnWidthsTwips: [1500, 3000] });
  });
});

describe('search', () => {
  it('finds a text in every paragraph, with or without its case', async () => {
    const { editor } = await mount();
    expect(findMatches(editor.state.doc, 'gatto', false)).toHaveLength(2);
    expect(findMatches(editor.state.doc, 'gatto', true)).toEqual([{ from: 4, to: 9 }]);
    expect(findMatches(editor.state.doc, '', false)).toEqual([]);
  });

  it('replaces the places one at a time and all at once from the find panel', async () => {
    const { ref } = await mount();
    fireEvent.click(screen.getByRole('button', { name: 'Find and replace' }));
    fireEvent.change(screen.getByRole('searchbox', { name: 'Find' }), { target: { value: 'gatto' } });
    expect(screen.getByText('1 of 2')).toBeTruthy();

    fireEvent.change(screen.getByRole('textbox', { name: 'Replace with' }), { target: { value: 'topo' } });
    fireEvent.click(screen.getByRole('button', { name: 'Replace' }));
    expect(ref.current!.getTree().blocks[0]).toMatchObject({ content: [{ text: 'Il topo e il cane' }] });

    fireEvent.click(screen.getByRole('button', { name: 'Replace all' }));
    expect(ref.current!.getTree().blocks[1]).toMatchObject({ content: [{ text: 'Un altro topo' }] });
    expect(screen.getByText('No results')).toBeTruthy();
  });
});

describe('tracked changes recorded while typing', () => {
  it('marks a deleted stretch, dropping what was only inserted', () => {
    const schema = getSchema(createRelyDocumentExtensions());
    const deletion = schema.marks.deletion.create({ author: 'Anna', date: null });
    const fragment = Fragment.fromArray([
      schema.text('kept'),
      schema.text('new', [schema.marks.insertion.create({ author: 'Anna', date: null })]),
    ]);
    const marked = markDeleted(fragment, deletion, schema);
    expect(marked.childCount).toBe(1);
    expect(marked.firstChild!.marks.map((mark) => mark.type.name)).toEqual(['deletion']);
  });

  it('records typing as an insertion and a deletion as deleted text by the author', async () => {
    const { ref, editor } = await mount(TEXT, { trackChanges: true });
    expect(screen.getByRole('button', { name: 'Track changes' }).getAttribute('aria-pressed')).toBe('true');

    act(() => {
      editor.chain().focus().setTextSelection(3).insertContent('X').run();
    });
    act(() => {
      editor.chain().setTextSelection({ from: 10, to: 15 }).deleteSelection().run();
    });

    const first = ref.current!.getTree().blocks[0];
    const texts = first.type === 'paragraph' ? first.content : [];
    expect(texts).toEqual([
      { type: 'text', text: 'Il' },
      { type: 'text', text: 'X', marks: [{ type: 'insertion', author: 'Anna', date: expect.any(String) }] },
      { type: 'text', text: ' gatto' },
      { type: 'text', text: ' e il', marks: [{ type: 'deletion', author: 'Anna', date: expect.any(String) }] },
      { type: 'text', text: ' cane' },
    ]);

    act(() => {
      editor.chain().setTextSelection(4).acceptChangesInSelection().run();
    });
    const accepted = ref.current!.getTree().blocks[0];
    expect(accepted.type === 'paragraph' && accepted.content[0]).toEqual({ type: 'text', text: 'IlX gatto' });
  });

  it('records nothing once tracking is switched off', async () => {
    const { ref, editor } = await mount(TEXT, { trackChanges: true });
    fireEvent.click(screen.getByRole('button', { name: 'Track changes' }));
    act(() => {
      editor.chain().focus().setTextSelection(1).insertContent('Y').run();
    });
    expect(ref.current!.getTree().blocks[0]).toEqual({ type: 'paragraph', content: [{ type: 'text', text: 'YIl gatto e il cane' }] });
  });
});

describe('toolbar formats', () => {
  it('sets the font, the size, the line spacing and a named style', async () => {
    const { ref, editor } = await mount();
    act(() => {
      editor.commands.setTextSelection({ from: 1, to: 3 });
    });
    fireEvent.change(screen.getByRole('combobox', { name: 'Font' }), { target: { value: 'Georgia' } });
    fireEvent.change(screen.getByRole('combobox', { name: 'Font size' }), { target: { value: '14' } });
    fireEvent.change(screen.getByRole('combobox', { name: 'Line spacing' }), { target: { value: '360' } });

    const first = ref.current!.getTree().blocks[0];
    expect(first).toMatchObject({ format: { lineSpacing: { rule: 'auto', value: 360 } } });
    expect(first.type === 'paragraph' && first.content[0]).toEqual({
      type: 'text',
      text: 'Il',
      marks: [{ type: 'textStyle', fontFamily: 'Georgia', fontSize: '14pt' }],
    });

    fireEvent.change(screen.getByRole('combobox', { name: 'Text style' }), { target: { value: 'title' } });
    expect(ref.current!.getTree().blocks[0]).toMatchObject({ type: 'paragraph', format: { style: 'title' } });
    fireEvent.change(screen.getByRole('combobox', { name: 'Text style' }), { target: { value: '2' } });
    expect(ref.current!.getTree().blocks[0]).toMatchObject({ type: 'heading', level: 2 });
    expect(ref.current!.getTree().blocks[0]).not.toHaveProperty('format.style');
  });

  it('breaks the page at the cursor', async () => {
    const { ref, editor } = await mount();
    act(() => {
      editor.commands.setTextSelection(9);
    });
    fireEvent.click(screen.getByRole('button', { name: 'Page break' }));
    const blocks = ref.current!.getTree().blocks;
    expect(blocks).toHaveLength(3);
    expect(blocks[1]).toMatchObject({ content: [{ text: ' e il cane' }], format: { pageBreakBefore: true } });
  });
});

describe('page view', () => {
  it('starts a page where the text no longer fits or a paragraph breaks it', async () => {
    const { editor } = await mount([
      { type: 'paragraph', content: [{ type: 'text', text: 'a' }] },
      { type: 'paragraph', content: [{ type: 'text', text: 'b' }] },
      { type: 'paragraph', content: [{ type: 'text', text: 'c' }], format: { pageBreakBefore: true } },
      { type: 'paragraph', content: [{ type: 'text', text: 'd' }] },
    ]);
    const offsets: number[] = [];
    editor.state.doc.forEach((_, offset) => offsets.push(offset));
    expect(pageBreaks(editor.state.doc, () => 60, 100)).toEqual([offsets[1], offsets[2], offsets[3]]);
    expect(pageBreaks(editor.state.doc, () => 10, 100)).toEqual([offsets[2]]);
  });

  it('draws the header of the document on the page', async () => {
    await mount(TEXT, { pageView: true });
    expect(document.querySelector('.rely-page-view')).toBeTruthy();
    expect(document.querySelector('.rely-page-header-first')!.textContent).toBe('Riservato');
    expect(screen.getByRole('button', { name: 'Page view' }).getAttribute('aria-pressed')).toBe('true');
    expect(blocksText(TEXT)).toBe('Il gatto e il cane  Un altro Gatto');
  });
});

describe('images', () => {
  it('keep their proportions when dragged by the corner', () => {
    expect(resizedImage({ width: 200, height: 100 }, 100, 1000)).toEqual({ width: 300, height: 150 });
    expect(resizedImage({ width: 200, height: 100 }, -500, 1000)).toEqual({ width: 16, height: 8 });
    expect(resizedImage({ width: 200, height: 100 }, 900, 400)).toEqual({ width: 400, height: 200 });
  });

  it('take the size dragged to into the tree', async () => {
    const { ref } = await mount([{ type: 'paragraph', content: [{ type: 'image', media: 0, width: 100, height: 50 }] }]);
    const handle = await waitFor(() => {
      const found = document.querySelector('.rely-image-handle');
      expect(found).toBeTruthy();
      return found!;
    });

    fireEvent.mouseDown(handle, { clientX: 0 });
    fireEvent.mouseMove(window, { clientX: 60 });
    fireEvent.mouseUp(window);

    expect(ref.current!.getTree().blocks[0]).toMatchObject({ content: [{ type: 'image', width: 160, height: 80 }] });
  });
});
