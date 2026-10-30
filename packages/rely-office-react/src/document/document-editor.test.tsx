// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { Editor } from '@tiptap/core';
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

/** Type into a small editor of the side panel, as a person would with the caret at its end. */
async function typeInto(box: HTMLElement, text: string): Promise<void> {
  // TipTap keeps its editor on the element it edits.
  const editor = await waitFor(() => {
    const found = (box as unknown as { editor?: Editor }).editor;
    expect(found).toBeTruthy();
    return found!;
  });
  act(() => {
    editor.chain().focus('end').insertContent(text).run();
  });
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
    expect(screen.getByRole('list', { name: 'Commenti' }).textContent).toContain('Marco Rossi');
    await waitFor(() => expect(screen.getByRole('textbox', { name: 'Commento Marco Rossi' }).textContent).toBe('Da rivedere'));
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

  test('a floating image keeps its anchor, and its wrapping and side are chosen from the toolbar', async () => {
    const anchor = {
      horizontal: { base: 'column' as const, offsetEmu: 0, align: 'end' as const },
      vertical: { base: 'paragraph' as const, offsetEmu: 0 },
      widthEmu: 952_500,
      heightEmu: 476_250,
      wrap: { type: 'around' as const },
      distanceEmu: [0, 0, 114_300, 114_300] as [number, number, number, number],
    };
    const floating: DocumentTree = {
      ...tree(),
      blocks: [
        {
          type: 'paragraph',
          content: [{ type: 'image', media: 0, width: 100, height: 50, anchor }, { type: 'text', text: 'Testo accanto' }],
        },
      ],
      media: [{ index: 0, contentType: 'image/png', bytes: PNG }],
      comments: [],
    };
    const ref = createRef<DocumentEditorHandle>();
    render(<DocumentEditor ref={ref} document={documentTreeToEditor(floating).document} />);
    await waitFor(() => expect(ref.current?.editor).toBeTruthy());
    const editor = ref.current!.editor!;
    const image = () => document.querySelector('.rely-image') as HTMLElement;
    expect(image().dataset.float).toBe('right');
    expect(image().dataset.wrap).toBe('around');
    const kept = ref.current!.getTree().blocks[0];
    expect(kept.type === 'paragraph' && kept.content[0]).toMatchObject({ type: 'image', anchor });

    act(() => {
      editor.commands.setNodeSelection(1);
    });
    fireEvent.change(screen.getByRole('combobox', { name: 'Position' }), { target: { value: 'start' } });
    expect(image().dataset.float).toBe('left');
    fireEvent.change(screen.getByRole('combobox', { name: 'Text wrapping' }), { target: { value: 'inline' } });
    expect(image().dataset.float).toBe('inline');
    const inline = ref.current!.getTree().blocks[0];
    expect(inline.type === 'paragraph' && inline.content[0]).not.toHaveProperty('anchor');
    expect((screen.getByRole('combobox', { name: 'Position' }) as HTMLSelectElement).disabled).toBe(true);

    fireEvent.change(screen.getByRole('combobox', { name: 'Text wrapping' }), { target: { value: 'topAndBottom' } });
    const made = ref.current!.getTree().blocks[0];
    expect(made.type === 'paragraph' && made.content[0]).toMatchObject({
      type: 'image',
      anchor: {
        wrap: { type: 'topAndBottom' },
        horizontal: { base: 'column', align: 'start' },
        widthEmu: 952_500,
        heightEmu: 476_250,
      },
    });
    expect(image().dataset.wrap).toBe('topAndBottom');
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

  test('a comment is added on the selection, written, answered and resolved', async () => {
    const { ref, editor } = await mount({ author: { name: 'Lucia Neri', initials: 'LN' } });
    act(() => {
      editor.commands.setTextSelection({ from: 20, to: 28 });
    });

    fireEvent.click(screen.getByRole('button', { name: 'Add comment' }));
    const box = await screen.findByRole('textbox', { name: 'Comment Lucia Neri' });
    await typeInto(box, 'Controllare le cifre');
    const thread = box.closest('li')!;
    fireEvent.click(within(thread as HTMLElement).getByRole('button', { name: 'Reply' }));
    fireEvent.click(within(thread as HTMLElement).getByRole('checkbox', { name: 'Resolved' }));

    const tree = ref.current!.getTree();
    expect(tree.comments).toHaveLength(3);
    expect(tree.comments![1]).toMatchObject({ author: 'Lucia Neri', initials: 'LN', resolved: true });
    expect(tree.comments![1].blocks).toEqual([{ type: 'paragraph', content: [{ type: 'text', text: 'Controllare le cifre' }] }]);
    expect(tree.comments![2]).toMatchObject({ author: 'Lucia Neri', parent: 1 });
    const second = tree.blocks[1];
    expect(second.type === 'paragraph' && second.content[0]).toMatchObject({
      text: 'aggiunto',
      marks: [{ type: 'comment', comment: 1 }, { type: 'insertion' }],
    });
  });

  test('a deleted comment leaves the text, and the saved tree, with its replies', async () => {
    const { ref } = await mount();

    fireEvent.click(screen.getByRole('button', { name: 'Delete comment' }));

    const tree = ref.current!.getTree();
    expect(tree.comments).toBeUndefined();
    expect(tree.blocks[0]).toEqual({ type: 'paragraph', content: [{ type: 'text', text: 'Testo del verbale' }] });
    expect(screen.queryByRole('textbox', { name: 'Comment Marco Rossi' })).toBeNull();
  });

  test('a footnote is inserted, numbered, written and deleted', async () => {
    const { ref, editor } = await mount();
    act(() => {
      editor.commands.setTextSelection(6);
    });

    fireEvent.click(screen.getByRole('button', { name: 'Insert footnote' }));
    const note = await screen.findByRole('textbox', { name: 'Note 1' });
    await typeInto(note, 'Fonte: bilancio 2025');

    let tree = ref.current!.getTree();
    expect(tree.notes).toEqual([{ kind: 'footnote', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Fonte: bilancio 2025' }] }] }]);
    expect(JSON.stringify(tree.blocks[0])).toContain('"noteReference","note":0');
    expect(document.querySelector('.rely-note-reference')!.textContent).toBe('1');

    fireEvent.click(screen.getByRole('button', { name: 'Delete note' }));
    tree = ref.current!.getTree();
    expect(tree.notes).toEqual([]);
    expect(JSON.stringify(tree.blocks)).not.toContain('noteReference');
  });

  test('the header and the footer are edited beside the text', async () => {
    const { ref, onChange } = await mount();

    fireEvent.click(screen.getByText('Headers and footers'));
    await typeInto(await screen.findByRole('textbox', { name: 'Header' }), 'Riservato');

    expect(ref.current!.getTree().headersFooters.header).toEqual([
      { type: 'paragraph', content: [{ type: 'text', text: 'Riservato' }] },
    ]);
    expect(onChange.mock.calls.at(-1)![0].aside.headersFooters.header).toHaveLength(1);
  });

  test('without the side panel, nothing beside the text is shown', async () => {
    await mount({ sidePanel: false });

    expect(screen.queryByText('Headers and footers')).toBeNull();
  });

  test('read only, the document shows without a toolbar', async () => {
    await mount({ editable: false });

    expect(screen.queryByRole('toolbar')).toBeNull();
    expect(screen.getByText('Testo del verbale')).toBeTruthy();
  });
});
