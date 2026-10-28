// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { createRef } from 'react';
import {
  ITALIAN_SLIDE_EDITOR_LABELS,
  SlideEditor,
  newGeometryShape,
  newTextShape,
  textBodyText,
  type SlideEditorHandle,
} from '../src/index.js';
import { readPresentation, writePptx, type Presentation } from '../src/runtime/index.js';
import { loadWasm } from './wasm.js';

const PNG = Uint8Array.from([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
  0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
  0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
  0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
  0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
]);

const WIDTH = 12_192_000;
const HEIGHT = 6_858_000;

function presentation(): Presentation {
  return {
    widthEmu: WIDTH,
    heightEmu: HEIGHT,
    truncated: false,
    media: [],
    slides: [
      {
        shapes: [
          { ...newTextShape({ x: 0, y: 0, width: WIDTH, height: 600_000 }, 'Logo del layout'), inherited: true },
          newTextShape({ x: 1_000_000, y: 1_000_000, width: 4_000_000, height: 1_000_000 }, 'Ordine del giorno'),
          newGeometryShape({ x: 6_000_000, y: 3_000_000, width: 2_000_000, height: 2_000_000 }, { type: 'ellipse' }),
        ],
      },
      { shapes: [newTextShape({ x: 0, y: 0, width: 3_000_000, height: 800_000 }, 'Seconda')], hidden: true },
    ],
  };
}

function mount(props: Partial<Parameters<typeof SlideEditor>[0]> = {}) {
  const ref = createRef<SlideEditorHandle>();
  const onChange = vi.fn();
  render(<SlideEditor ref={ref} presentation={presentation()} onChange={onChange} {...props} />);
  const canvas = screen.getByRole('group');
  return { ref, onChange, canvas, shape: (name: string) => within(canvas).getByRole('button', { name }) };
}

const current = (ref: { current: SlideEditorHandle | null }) => ref.current!.getPresentation();

beforeAll(loadWasm);

describe('SlideEditor', () => {
  test('shows the slides as thumbnails and the first with its text, the layout locked', () => {
    const { canvas } = mount();

    expect(screen.getAllByRole('button', { name: /^Slide \d$/ })).toHaveLength(2);
    expect(within(canvas).getByText('Ordine del giorno')).toBeTruthy();
    expect(within(canvas).getByRole('button', { name: 'From the layout' }).getAttribute('aria-disabled')).toBe('true');
  });

  test('a selected shape moves with the arrows and goes with Delete, its change reported', () => {
    const { ref, onChange, shape } = mount();
    fireEvent.pointerDown(shape('Shape 2'), { button: 0, clientX: 10, clientY: 10 });
    fireEvent.pointerUp(window);
    const stage = shape('Shape 2').closest('.rely-slide-stage')!;

    fireEvent.keyDown(stage, { key: 'ArrowRight', shiftKey: true });
    expect(current(ref).slides[0].shapes![1].frame.x).toBe(1_000_000 + 457_200);
    fireEvent.keyDown(stage, { key: 'Delete' });

    expect(current(ref).slides[0].shapes).toHaveLength(2);
    expect(onChange).toHaveBeenCalledTimes(2);
  });

  test('a drag moves a shape and a corner drag resizes it, reported once each', () => {
    const { ref, onChange, shape } = mount();
    const scale = 960 / WIDTH;

    fireEvent.pointerDown(shape('Shape 3'), { button: 0, clientX: 100, clientY: 100 });
    fireEvent.pointerMove(window, { clientX: 150, clientY: 120 });
    fireEvent.pointerMove(window, { clientX: 200, clientY: 140 });
    fireEvent.pointerUp(window);
    const frame = current(ref).slides[0].shapes![2].frame;
    expect(frame.x).toBeCloseTo(6_000_000 + 100 / scale, -2);
    expect(frame.y).toBeCloseTo(3_000_000 + 40 / scale, -2);
    expect(onChange).toHaveBeenCalledTimes(1);

    const handle = shape('Shape 3').querySelector('.rely-slide-handle-bottomRight')!;
    fireEvent.pointerDown(handle, { button: 0, clientX: 0, clientY: 0 });
    fireEvent.pointerMove(window, { clientX: 48, clientY: 24 });
    fireEvent.pointerUp(window);
    expect(current(ref).slides[0].shapes![2].frame.width).toBeCloseTo(2_000_000 + 48 / scale, -2);
    expect(onChange).toHaveBeenCalledTimes(2);
  });

  test('text written into a shape is kept with its look', () => {
    const { ref, shape } = mount();
    fireEvent.doubleClick(shape('Shape 2'));
    const editable = shape('Shape 2').querySelector('[contenteditable="true"]') as HTMLElement;

    editable.querySelector('[data-run="0"]')!.textContent = 'Agenda';
    fireEvent.blur(editable);

    const content = current(ref).slides[0].shapes![1].content;
    expect(content.type === 'text' && textBodyText(content)).toBe('Agenda');
    expect(shape('Shape 2').querySelector('[contenteditable="true"]')).toBeNull();
  });

  test('the toolbar styles the text of the selected shape and fills it', () => {
    const { ref, shape } = mount();
    fireEvent.pointerDown(shape('Shape 2'), { button: 0 });
    fireEvent.pointerUp(window);

    fireEvent.click(screen.getByRole('button', { name: 'Bold' }));
    fireEvent.change(screen.getByLabelText('Font size'), { target: { value: '32' } });
    fireEvent.click(screen.getByRole('button', { name: 'Centre' }));
    fireEvent.change(screen.getByLabelText('Fill colour'), { target: { value: '#ffcc00' } });

    const edited = current(ref).slides[0].shapes![1];
    expect(edited.content.type === 'text' && edited.content.paragraphs[0]).toMatchObject({
      align: 'center',
      runs: [{ bold: true, size: 3200 }],
    });
    expect(edited.fill).toEqual({ type: 'solid', colour: 'FFCC00' });
    expect(screen.getByRole('button', { name: 'Bold' }).getAttribute('aria-pressed')).toBe('true');
  });

  test('shapes are added and moved in the drawing order, never behind the layout', () => {
    const { ref } = mount();

    fireEvent.click(screen.getByRole('button', { name: 'Text box' }));
    fireEvent.click(screen.getByRole('button', { name: 'Rectangle' }));
    expect(current(ref).slides[0].shapes!.map((each) => each.geometry.type)).toEqual(['rectangle', 'rectangle', 'ellipse', 'rectangle', 'rectangle']);
    fireEvent.click(screen.getByRole('button', { name: 'Send backward' }));
    expect(current(ref).slides[0].shapes![3].content.type).toBe('none');
    expect(screen.getByRole('button', { name: 'Bring forward' })).toBeTruthy();
  });

  test('an image the user chooses joins the media as a picture', async () => {
    const { ref } = mount();

    fireEvent.change(screen.getByTestId('rely-slide-image-input'), {
      target: { files: [new File([PNG], 'foto.png', { type: 'image/png' })] },
    });

    await waitFor(() => expect(current(ref).media).toHaveLength(1));
    expect(current(ref).slides[0].shapes!.at(-1)!.content).toEqual({ type: 'picture', media: 0 });
  });

  test('slides are added, duplicated, moved, hidden and deleted, in the language given', () => {
    const { ref } = mount({ labels: ITALIAN_SLIDE_EDITOR_LABELS });

    fireEvent.click(screen.getByRole('button', { name: 'Duplica diapositiva' }));
    expect(current(ref).slides).toHaveLength(3);
    expect(ref.current!.activeSlide).toBe(1);
    fireEvent.click(screen.getByRole('button', { name: 'Sposta giù' }));
    expect(ref.current!.activeSlide).toBe(2);
    fireEvent.click(screen.getByRole('button', { name: 'Nascondi nella presentazione' }));
    expect(current(ref).slides[2].hidden).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Elimina diapositiva' }));
    fireEvent.click(screen.getByRole('button', { name: 'Nuova diapositiva' }));
    expect(current(ref).slides.map((slide) => slide.shapes?.length)).toEqual([3, 1, 0]);
  });

  test('the speaker notes of a slide are written when the field is left', () => {
    const { ref } = mount();

    const notes = screen.getByLabelText('Speaker notes');
    fireEvent.change(notes, { target: { value: 'Citare i numeri' } });
    fireEvent.blur(notes);
    fireEvent.click(screen.getByRole('button', { name: 'Slide 2' }));

    expect(textBodyText(current(ref).slides[0].notes)).toBe('Citare i numeri');
    expect((screen.getByLabelText('Speaker notes') as HTMLTextAreaElement).value).toBe('');
  });

  test('changes are undone and redone one at a time, a drag as one change', () => {
    const { ref, shape } = mount();
    expect(screen.getByRole('button', { name: 'Undo' }).hasAttribute('disabled')).toBe(true);

    fireEvent.pointerDown(shape('Shape 3'), { button: 0, clientX: 0, clientY: 0 });
    fireEvent.pointerMove(window, { clientX: 30, clientY: 0 });
    fireEvent.pointerMove(window, { clientX: 60, clientY: 0 });
    fireEvent.pointerUp(window);
    const stage = shape('Shape 3').closest('.rely-slide-stage')!;
    fireEvent.keyDown(stage, { key: 'Delete' });
    expect(current(ref).slides[0].shapes).toHaveLength(2);

    fireEvent.keyDown(stage, { key: 'z', ctrlKey: true });
    expect(current(ref).slides[0].shapes).toHaveLength(3);
    expect(current(ref).slides[0].shapes![2].frame.x).toBeGreaterThan(6_000_000);
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(current(ref).slides[0].shapes![2].frame.x).toBe(6_000_000);
    expect(screen.getByRole('button', { name: 'Undo' }).hasAttribute('disabled')).toBe(true);

    fireEvent.keyDown(stage, { key: 'y', ctrlKey: true });
    expect(current(ref).slides[0].shapes![2].frame.x).toBeGreaterThan(6_000_000);
    fireEvent.click(screen.getByRole('button', { name: 'Redo' }));
    expect(current(ref).slides[0].shapes).toHaveLength(2);
    expect(screen.getByRole('button', { name: 'Redo' }).hasAttribute('disabled')).toBe(true);
  });

  test('a new change after an undo drops what could be redone', () => {
    const { ref } = mount();
    fireEvent.click(screen.getByRole('button', { name: 'Text box' }));
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));

    fireEvent.click(screen.getByRole('button', { name: 'Ellipse' }));

    expect(screen.getByRole('button', { name: 'Redo' }).hasAttribute('disabled')).toBe(true);
    expect(current(ref).slides[0].shapes!.at(-1)!.geometry).toEqual({ type: 'ellipse' });
  });

  test('read only, there is no toolbar and a shape cannot be moved', () => {
    const { ref, shape } = mount({ editable: false });

    fireEvent.pointerDown(shape('Shape 2'), { button: 0, clientX: 0, clientY: 0 });
    fireEvent.pointerMove(window, { clientX: 300, clientY: 300 });
    fireEvent.pointerUp(window);

    expect(screen.queryByRole('toolbar')).toBeNull();
    expect(current(ref).slides[0].shapes![1].frame.x).toBe(1_000_000);
  });

  test('a pptx edited in the editor saves to a pptx that reads back with the edit', () => {
    const pptx = writePptx({ ...presentation(), slides: presentation().slides.map((slide) => ({ ...slide, shapes: slide.shapes!.filter((each) => !each.inherited) })), media: [PNG] });
    const opened = readPresentation(pptx);
    const ref = createRef<SlideEditorHandle>();
    render(<SlideEditor ref={ref} presentation={opened} />);
    const canvas = screen.getAllByRole('group', { name: 'Slide' }).at(-1)!;
    const own = opened.slides[0].shapes!.findIndex((each) => !each.inherited && each.content.type === 'text');

    act(() => {
      fireEvent.pointerDown(within(canvas).getByRole('button', { name: `Shape ${own + 1}` }), { button: 0 });
      fireEvent.pointerUp(window);
    });
    fireEvent.click(screen.getAllByRole('button', { name: 'Italic' }).at(-1)!);

    const reread = readPresentation(ref.current!.savePptx());
    const text = reread.slides[0].shapes!.find((each) => each.content.type === 'text' && !each.inherited)!;
    expect(text.content.type === 'text' && text.content.paragraphs[0].runs[0]).toMatchObject({ text: 'Ordine del giorno', italic: true });
    expect(reread.slides[1].hidden).toBe(true);
  });
});
