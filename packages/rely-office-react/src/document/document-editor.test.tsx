// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { createRef } from 'react';
import type { DocumentTree } from '../model.js';
import { DocumentEditor, type DocumentEditorHandle } from './document-editor.js';
import type { EditorDocument } from './editor-document.js';
import { ITALIAN_DOCUMENT_EDITOR_LABELS } from './labels.js';
import { documentTreeToEditor } from './tree-to-editor.js';

const PNG = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

function tree(): DocumentTree {
  return {
    blocks: [
      { type: 'paragraph', content: [{ type: 'text', text: 'Testo del verbale', marks: [{ type: 'comment', comment: 0 }] }] },
      { type: 'paragraph', content: [{ type: 'text', text: 'aggiunto', marks: [{ type: 'insertion', author: 'Anna' }] }, { type: 'text', text: ' tolto', marks: [{ type: 'deletion', author: 'Anna' }] }] },
    ],
    media: [],
    report: {
      truncated: false, textBoxes: 0, footnotes: 0, endnotes: 0, comments: 1, headersAndFooters: 0,
      fieldsKeptAsText: 0, charts: 0, smartArt: 0, equations: 0, shapes: 0, embeddedObjects: 0,
      unsupportedLinks: 0, oversizedImages: 0, imagesOverLimit: 0, missingImages: 0,
      trackedInsertions: 0, trackedDeletions: 0, hiddenRuns: 0,
    },
    notes: [],
    headersFooters: { header: [], footer: [] },
    page: { widthTwips: 11906, heightTwips: 16838, marginsTwips: [1440, 1440, 1440, 1440] },
    comments: [{ author: 'Marco Rossi', date: '2026-10-01T10:00:00Z', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Da rivedere' }] }], resolved: false }],
  };
}

function open(): EditorDocument {
  return documentTreeToEditor(tree()).document;
}

async function mount(props: Partial<Parameters<typeof DocumentEditor>[0]> = {}) {
  const ref = createRef<DocumentEditorHandle>();
  const onChange = vi.fn();
  render(<DocumentEditor ref={ref} document={open()} onChange={onChange} {...props} />);
  await waitFor(() => expect(ref.current?.editor).toBeTruthy());
  return { ref, onChange, editor: ref.current!.editor! };
}

describe('DocumentEditor', () => {
  test('shows the document, its comments and a toolbar in the language given', async () => {
    await mount({ labels: ITALIAN_DOCUMENT_EDITOR_LABELS });

    expect(screen.getByText('Testo del verbale')).toBeTruthy();
    expect(screen.getByRole('toolbar', { name: 'Formattazione' })).toBeTruthy();
    expect(screen.getByRole('complementary', { name: 'Commenti' }).textContent).toContain('Marco Rossi');
    expect(screen.getByText('Da rivedere')).toBeTruthy();
  });

  test('a toolbar button formats the selection, and the change reaches the host', async () => {
    const { ref, onChange, editor } = await mount();

    act(() => {
      editor.commands.setTextSelection({ from: 1, to: 6 });
    });
    fireEvent.click(screen.getByRole('button', { name: 'Bold' }));

    const first = ref.current!.getTree().blocks[0];
    expect(first.type === 'paragraph' && first.content[0]).toEqual({
      type: 'text',
      text: 'Testo',
      marks: [{ type: 'bold' }, { type: 'comment', comment: 0 }],
    });
    expect(onChange).toHaveBeenCalled();
  });

  test('accepting every change keeps the insertions and drops the deletions', async () => {
    const { ref } = await mount();

    fireEvent.click(screen.getByRole('button', { name: 'Accept all changes' }));

    expect(ref.current!.getTree().blocks[1]).toEqual({
      type: 'paragraph',
      content: [{ type: 'text', text: 'aggiunto' }],
    });
  });

  test('rejecting every change drops the insertions and keeps the deletions', async () => {
    const { ref } = await mount();

    fireEvent.click(screen.getByRole('button', { name: 'Reject all changes' }));

    expect(ref.current!.getTree().blocks[1]).toEqual({
      type: 'paragraph',
      content: [{ type: 'text', text: ' tolto' }],
    });
  });

  test('an image the user chooses joins the media and the text', async () => {
    const { ref, editor } = await mount();
    act(() => {
      editor.commands.setTextSelection(1);
    });

    fireEvent.change(screen.getByTestId('rely-image-input'), {
      target: { files: [new File([PNG], 'logo.png', { type: 'image/png' })] },
    });

    await waitFor(() => expect(ref.current!.getDocument().media).toHaveLength(1));
    const document = ref.current!.getTree();
    expect(document.media[0]).toEqual({ index: 0, contentType: 'image/png', bytes: PNG });
    const first = document.blocks[0];
    expect(first.type === 'paragraph' && first.content[0]).toEqual({ type: 'image', media: 0, alt: 'logo.png' });
  });

  test('an image the host refuses is not inserted', async () => {
    const { ref } = await mount({ onImageFile: async () => null });

    fireEvent.change(screen.getByTestId('rely-image-input'), {
      target: { files: [new File([PNG], 'big.png', { type: 'image/png' })] },
    });

    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(ref.current!.getDocument().media).toHaveLength(0);
  });

  test('a link and an equation are set from their panels', async () => {
    const { ref, editor } = await mount();
    act(() => {
      editor.commands.setTextSelection({ from: 1, to: 6 });
    });

    fireEvent.click(screen.getByRole('button', { name: 'Link' }));
    fireEvent.change(screen.getByLabelText('Address'), { target: { value: 'https://relybytes.com' } });
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    act(() => {
      editor.commands.setTextSelection(editor.state.doc.content.size - 1);
    });
    fireEvent.click(screen.getByRole('button', { name: 'Equation' }));
    fireEvent.change(screen.getByLabelText('Equation in LaTeX'), { target: { value: 'x^2' } });
    fireEvent.submit(screen.getByLabelText('Equation in LaTeX').closest('form')!);

    const blocks = ref.current!.getTree().blocks;
    expect(JSON.stringify(blocks)).toContain('"href":"https://relybytes.com"');
    expect(JSON.stringify(blocks)).toContain('"latex":"x^2"');
  });

  test('read only, the document shows without a toolbar', async () => {
    await mount({ editable: false });

    expect(screen.queryByRole('toolbar')).toBeNull();
    expect(screen.getByText('Testo del verbale')).toBeTruthy();
  });
});
