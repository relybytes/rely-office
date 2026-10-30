// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { createRef } from 'react';
import { SlideEditor, newGeometryShape, newTextShape, type SlideEditorHandle } from '../src/index.js';
import { readPresentation, writePptx, type Presentation, type Slide } from '../src/runtime/index.js';
import { SlideShow, effectCss, transitionCss } from '../src/slides/slide-show.js';
import {
  initiallyHidden,
  newTableShape,
  presetGeometry,
  rotationTowards,
  scaledGeometry,
  showSteps,
  snapFrame,
  withPastedShapes,
  withTableColumn,
  withTableRow,
  withoutShapes,
  withoutTableColumn,
} from '../src/slides/slide-tools.js';
import { loadWasm } from './wasm.js';

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
          newTextShape({ x: 1_000_000, y: 1_000_000, width: 4_000_000, height: 1_000_000 }, 'Titolo'),
          newGeometryShape({ x: 6_000_000, y: 3_000_000, width: 2_000_000, height: 2_000_000 }, { type: 'ellipse' }),
          newGeometryShape({ x: 9_000_000, y: 500_000, width: 1_000_000, height: 1_000_000 }, { type: 'rectangle' }),
        ],
      },
      { shapes: [newTextShape({ x: 0, y: 0, width: 3_000_000, height: 800_000 }, 'Seconda')] },
    ],
  };
}

function mount() {
  const ref = createRef<SlideEditorHandle>();
  render(<SlideEditor ref={ref} presentation={presentation()} />);
  const canvas = screen.getByRole('group', { name: 'Slide' });
  return { ref, canvas, shape: (number: number) => within(canvas).getByRole('button', { name: `Shape ${number}` }) };
}

const current = (ref: { current: SlideEditorHandle | null }) => ref.current!.getPresentation();
const scale = 960 / WIDTH;

beforeAll(loadWasm);

describe('slide tools', () => {
  test('draws presets as paths and stretches them with their frame', () => {
    const triangle = presetGeometry('triangle', 1000, 500);
    expect(triangle).toEqual({
      type: 'path',
      paths: [
        {
          commands: [
            { type: 'moveTo', to: { x: 500, y: 0 } },
            { type: 'lineTo', to: { x: 1000, y: 500 } },
            { type: 'lineTo', to: { x: 0, y: 500 } },
            { type: 'close' },
          ],
          filled: true,
          stroked: true,
          shade: 'normal',
        },
      ],
    });
    const stretched = scaledGeometry(triangle, { x: 0, y: 0, width: 1000, height: 500 }, { x: 0, y: 0, width: 2000, height: 1000 });
    expect(stretched.type === 'path' && stretched.paths[0].commands[1]).toEqual({ type: 'lineTo', to: { x: 2000, y: 1000 } });
    expect(presetGeometry('ellipse', 1, 1)).toEqual({ type: 'ellipse' });
    expect(scaledGeometry({ type: 'rectangle' }, { x: 0, y: 0, width: 1, height: 1 }, { x: 0, y: 0, width: 2, height: 2 })).toEqual({ type: 'rectangle' });
  });

  test('snaps an edge or a centre to the slide and to other shapes', () => {
    const slide = { widthEmu: 1000, heightEmu: 600 };
    const snapped = snapFrame({ x: 395, y: 203, width: 200, height: 100 }, [{ x: 0, y: 200, width: 100, height: 100 }], slide, 10);
    expect(snapped.frame).toEqual({ x: 400, y: 200, width: 200, height: 100 });
    expect(snapped.guides.vertical).toEqual([500]);
    expect(snapped.guides.horizontal).toEqual(expect.arrayContaining([200, 300]));
    expect(snapFrame({ x: 120, y: 120, width: 10, height: 10 }, [], slide, 5).guides).toEqual({ vertical: [], horizontal: [] });
  });

  test('turns towards the pointer, clockwise from up, snapping on request', () => {
    const frame = { x: 0, y: 0, width: 100, height: 100 };
    expect(rotationTowards(frame, { x: 50, y: -50 })).toBe(0);
    expect(rotationTowards(frame, { x: 150, y: 50 })).toBe(90);
    expect(rotationTowards(frame, { x: 50, y: 150 })).toBe(180);
    expect(rotationTowards(frame, { x: 100, y: -10 }, true)).toBe(45);
  });

  test('pastes shapes with their images and removes several, the animations following', () => {
    const base = presentation();
    const image = { index: 0, contentType: 'image/png', bytes: new Uint8Array([1, 2, 3]) };
    const picture = { ...newGeometryShape({ x: 0, y: 0, width: 10, height: 10 }, { type: 'rectangle' }), content: { type: 'picture' as const, media: 0 } };
    const { presentation: pasted, added } = withPastedShapes(base, 0, { shapes: [picture], media: [image] }, 100);
    expect(added).toEqual([3]);
    expect(pasted.media).toEqual([image]);
    expect(pasted.slides[0].shapes![3].frame).toMatchObject({ x: 100, y: 100 });

    const slide: Slide = {
      shapes: base.slides[0].shapes,
      animations: [
        { shape: 0, class: 'entrance', preset: 10, presetSubtype: 0, trigger: 'onClick', delayMs: 0, durationMs: 500 },
        { shape: 2, class: 'entrance', preset: 10, presetSubtype: 0, trigger: 'onClick', delayMs: 0, durationMs: 500 },
      ],
    };
    const left = withoutShapes(slide, [0, 1]);
    expect(left.shapes).toHaveLength(1);
    expect(left.animations).toEqual([{ ...slide.animations![1], shape: 0 }]);
  });

  test('adds and removes the rows and columns of a table', () => {
    const shape = newTableShape({ x: 0, y: 0, width: 3000, height: 900 }, 3, 3);
    expect(shape.content.type).toBe('table');
    if (shape.content.type !== 'table') return;
    const wider = withTableColumn(shape.content, 0);
    expect(wider.columnWidthsEmu).toHaveLength(4);
    expect(wider.rows.every((row) => row.cells.length === 4)).toBe(true);
    expect(withTableRow(shape.content, 2).rows).toHaveLength(4);
    expect(withoutTableColumn(withoutTableColumn(withoutTableColumn(shape.content, 0), 0), 0).columnWidthsEmu).toHaveLength(1);
  });

  test('builds the steps of a show from the triggers of the animations', () => {
    const slide: Slide = {
      animations: [
        { shape: 0, class: 'entrance', preset: 10, presetSubtype: 0, trigger: 'withPrevious', delayMs: 0, durationMs: 400 },
        { shape: 1, class: 'entrance', preset: 2, presetSubtype: 4, trigger: 'onClick', delayMs: 0, durationMs: 500 },
        { shape: 2, class: 'emphasis', preset: 6, presetSubtype: 0, trigger: 'afterPrevious', delayMs: 100, durationMs: 300 },
        { shape: 1, class: 'exit', preset: 10, presetSubtype: 0, trigger: 'withPrevious', delayMs: 0, durationMs: 300 },
      ],
    };
    const steps = showSteps(slide);
    expect(steps).toHaveLength(2);
    expect(steps[0].map((effect) => effect.shape)).toEqual([0]);
    expect(steps[1].map((effect) => [effect.shape, effect.startMs])).toEqual([[1, 0], [2, 600], [1, 600]]);
    expect([...initiallyHidden(slide)]).toEqual([0, 1]);
    expect(transitionCss({ effect: 'fade', advanceOnClick: true, durationMs: 300 })).toMatchObject({ animation: 'rely-transition-fade 300ms ease-out both' });
    expect(transitionCss({ effect: 'push', direction: 'u', advanceOnClick: true })).toMatchObject({ '--rely-transition-from': 'translate(0, 100%)' });
    expect(transitionCss(null)).toEqual({});
    expect(effectCss(steps[1][1])).toEqual({ animation: 'rely-effect-emphasis-grow 300ms ease-out 600ms both' });
  });
});

describe('SlideEditor, several shapes', () => {
  test('Shift adds a shape to the selection and a drag moves them together', () => {
    const { ref, shape } = mount();
    fireEvent.pointerDown(shape(1), { button: 0, clientX: 0, clientY: 0 });
    fireEvent.pointerUp(window);
    fireEvent.pointerDown(shape(2), { button: 0, shiftKey: true });
    fireEvent.pointerUp(window);
    expect(shape(1).getAttribute('aria-pressed')).toBe('true');
    expect(shape(2).getAttribute('aria-pressed')).toBe('true');

    fireEvent.pointerDown(shape(2), { button: 0, clientX: 0, clientY: 0, altKey: true });
    fireEvent.pointerMove(window, { clientX: 30, clientY: 20, altKey: true });
    fireEvent.pointerMove(window, { clientX: 50, clientY: 40, altKey: true });
    fireEvent.pointerUp(window);

    const [first, second] = current(ref).slides[0].shapes!;
    expect(first.frame.x).toBeCloseTo(1_000_000 + 50 / scale, -2);
    expect(second.frame.y).toBeCloseTo(3_000_000 + 40 / scale, -2);
  });

  test('a box dragged over the slide selects the shapes it touches', () => {
    const { canvas, shape } = mount();
    fireEvent.pointerDown(canvas, { button: 0, clientX: 0, clientY: 0 });
    fireEvent.pointerMove(window, { clientX: 1_400_000 * scale, clientY: 1_400_000 * scale });
    fireEvent.pointerUp(window);

    expect(shape(1).getAttribute('aria-pressed')).toBe('true');
    expect(shape(2).getAttribute('aria-pressed')).toBe('false');
  });

  test('a moved shape snaps to the centre of the slide and shows the guide', () => {
    const { ref, shape, canvas } = mount();
    fireEvent.pointerDown(shape(3), { button: 0, clientX: 0, clientY: 0 });
    // The rectangle's centre is 9,500,000; the slide's is 6,096,000.
    const dx = (6_096_000 - 9_500_000) * scale + 3;
    fireEvent.pointerMove(window, { clientX: dx / 2, clientY: 0 });
    fireEvent.pointerMove(window, { clientX: dx, clientY: 0 });
    expect(canvas.querySelector('.rely-slide-guide-vertical')).toBeTruthy();
    fireEvent.pointerUp(window);

    expect(current(ref).slides[0].shapes![2].frame.x).toBe(6_096_000 - 500_000);
    expect(canvas.querySelector('.rely-slide-guide')).toBeNull();
  });

  test('Ctrl+A, Delete and undo work on every selected shape', () => {
    const { ref } = mount();
    const stage = document.querySelector('.rely-slide-stage')!;
    fireEvent.keyDown(stage, { key: 'a', ctrlKey: true });
    fireEvent.keyDown(stage, { key: 'Delete' });
    expect(current(ref).slides[0].shapes).toHaveLength(0);
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(current(ref).slides[0].shapes).toHaveLength(3);
  });

  test('copy and paste brings the shapes onto another slide, and duplicate copies in place', () => {
    const { ref, shape } = mount();
    fireEvent.pointerDown(shape(2), { button: 0 });
    fireEvent.pointerUp(window);
    const stage = document.querySelector('.rely-slide-stage')!;
    const data = new Map<string, string>();
    fireEvent.copy(stage, { clipboardData: { setData: (type: string, value: string) => data.set(type, value), getData: () => '' } });

    fireEvent.click(screen.getByRole('button', { name: 'Slide 2' }));
    fireEvent.paste(stage, { clipboardData: { getData: (type: string) => data.get(type) ?? '', files: [] } });
    const pasted = current(ref).slides[1].shapes!;
    expect(pasted).toHaveLength(2);
    expect(pasted[1].geometry).toEqual({ type: 'ellipse' });
    expect(pasted[1].frame.x).toBe(6_000_000 + 45_720 * 4);

    fireEvent.click(screen.getByRole('button', { name: 'Duplicate' }));
    expect(current(ref).slides[1].shapes).toHaveLength(3);
  });

  test('text from elsewhere is pasted as a text box', () => {
    const { ref } = mount();
    const stage = document.querySelector('.rely-slide-stage')!;
    fireEvent.click(screen.getByRole('button', { name: 'Slide 2' }));
    fireEvent.paste(stage, { clipboardData: { getData: () => 'Testo incollato', files: [] } });
    const added = current(ref).slides[1].shapes!.at(-1)!;
    expect(added.content.type === 'text' && added.content.paragraphs[0].runs[0].text).toBe('Testo incollato');
  });
});

describe('SlideEditor, shapes, rotation and tables', () => {
  test('the shapes menu draws a preset, and resizing stretches its outline', () => {
    const { ref, canvas } = mount();
    fireEvent.click(screen.getByRole('button', { name: 'Shapes' }));
    fireEvent.click(within(screen.getByRole('menu', { name: 'Shapes' })).getByRole('menuitem', { name: 'Star' }));
    const star = current(ref).slides[0].shapes!.at(-1)!;
    expect(star.geometry.type).toBe('path');

    const handle = within(canvas).getByLabelText('Resize bottomRight');
    fireEvent.pointerDown(handle, { button: 0, clientX: 0, clientY: 0 });
    fireEvent.pointerMove(window, { clientX: 40, clientY: 0, altKey: true });
    fireEvent.pointerUp(window);
    const resized = current(ref).slides[0].shapes!.at(-1)!;
    const widest = (shape: typeof star) =>
      shape.geometry.type === 'path' ? Math.max(...shape.geometry.paths[0].commands.map((command) => ('to' in command ? command.to.x : 0))) : 0;
    expect(widest(resized)).toBeGreaterThan(widest(star));
  });

  test('the rotation handle and the rotation box turn the shape', () => {
    const { ref, canvas, shape } = mount();
    fireEvent.pointerDown(shape(2), { button: 0 });
    fireEvent.pointerUp(window);
    fireEvent.change(screen.getByRole('spinbutton', { name: 'Rotate' }), { target: { value: '30' } });
    expect(current(ref).slides[0].shapes![1].rotation).toBe(30);

    fireEvent.pointerDown(within(canvas).getByLabelText('Rotate'), { button: 0 });
    // The ellipse's centre is at 7,000,000 by 4,000,000; a point right of it is 90 degrees.
    fireEvent.pointerMove(window, { clientX: 9_000_000 * scale, clientY: 4_000_000 * scale });
    fireEvent.pointerUp(window);
    expect(current(ref).slides[0].shapes![1].rotation).toBe(90);
  });

  test('a table is inserted, grows a row and a column, and its cell takes a colour', () => {
    const { ref } = mount();
    fireEvent.click(screen.getByRole('button', { name: 'Table' }));
    fireEvent.click(screen.getByRole('button', { name: 'Add row below' }));
    fireEvent.click(screen.getByRole('button', { name: 'Add column right' }));
    fireEvent.change(screen.getByLabelText('Cell colour'), { target: { value: '#ff0000' } });

    const table = current(ref).slides[0].shapes!.at(-1)!.content;
    expect(table.type).toBe('table');
    if (table.type !== 'table') return;
    expect(table.rows).toHaveLength(4);
    expect(table.columnWidthsEmu).toHaveLength(4);
    expect(table.rows[0].cells[0].fill).toBe('FF0000');
  });

  test('a table survives the trip to pptx and back', () => {
    const base = presentation();
    const table = newTableShape({ x: 0, y: 0, width: 3_000_000, height: 900_000 }, 2, 2);
    const reread = readPresentation(writePptx({ ...base, slides: [{ shapes: [table] }] }));
    const content = reread.slides[0].shapes![0].content;
    expect(content.type).toBe('table');
    expect(content.type === 'table' && content.rows.length).toBe(2);
  });
});

describe('transitions, animations and the show', () => {
  test('the effects panel sets the transition and adds an animation to the selection', () => {
    const { ref, shape } = mount();
    fireEvent.pointerDown(shape(1), { button: 0 });
    fireEvent.pointerUp(window);
    fireEvent.click(screen.getByRole('button', { name: 'Transitions and animations' }));
    const panel = screen.getByRole('complementary', { name: 'Transitions and animations' });

    fireEvent.change(within(panel).getAllByRole('combobox')[0], { target: { value: 'push' } });
    fireEvent.change(within(panel).getByRole('combobox', { name: 'Direction' }), { target: { value: 'u' } });
    fireEvent.change(within(panel).getByRole('combobox', { name: 'Animations' }), { target: { value: '1' } });
    fireEvent.click(within(panel).getByRole('button', { name: 'Add to the selection' }));

    const slide = current(ref).slides[0];
    expect(slide.transition).toMatchObject({ effect: 'push', direction: 'u' });
    expect(slide.animations).toEqual([
      { shape: 0, class: 'entrance', preset: 10, presetSubtype: 0, trigger: 'onClick', delayMs: 0, durationMs: 500 },
    ]);
    fireEvent.change(within(panel).getByRole('combobox', { name: 'Start' }), { target: { value: 'afterPrevious' } });
    expect(current(ref).slides[0].animations![0].trigger).toBe('afterPrevious');
    fireEvent.click(within(panel).getByRole('button', { name: 'Remove' }));
    expect(current(ref).slides[0].animations).toEqual([]);
  });

  test('the show plays the effects click by click, skips hidden slides and leaves on Escape', () => {
    const base = presentation();
    base.slides[0].animations = [{ shape: 1, class: 'entrance', preset: 10, presetSubtype: 0, trigger: 'onClick', delayMs: 0, durationMs: 500 }];
    base.slides.splice(1, 0, { hidden: true, shapes: [newTextShape({ x: 0, y: 0, width: 100, height: 100 }, 'Nascosta')] });
    const onClose = vi.fn();
    render(<SlideShow presentation={base} onClose={onClose} fullscreen={false} />);
    const show = screen.getByRole('dialog', { name: 'Slide show' });

    expect(show.querySelector('[data-shape="1"]')).toBeNull();
    fireEvent.keyDown(show, { key: 'ArrowRight' });
    expect(show.querySelector('[data-shape="1"]')).toBeTruthy();
    expect((show.querySelector('[data-shape="1"]') as HTMLElement).style.animation).toContain('rely-effect-entrance-fade');
    fireEvent.click(show);
    expect(within(show).getByText('Seconda')).toBeTruthy();
    expect(within(show).queryByText('Nascosta')).toBeNull();
    fireEvent.keyDown(show, { key: 'ArrowLeft' });
    expect(show.querySelector('[data-shape="1"]')).toBeTruthy();
    fireEvent.keyDown(show, { key: 'End' });
    fireEvent.keyDown(show, { key: ' ' });
    expect(within(show).getByText('End of the show. Click to leave.')).toBeTruthy();
    fireEvent.keyDown(show, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  test('the play button opens the show from the current slide', () => {
    mount();
    act(() => {
      fireEvent.click(screen.getByRole('button', { name: 'Play from this slide' }));
    });
    expect(screen.getByRole('dialog', { name: 'Slide show' })).toBeTruthy();
    fireEvent.keyDown(screen.getByRole('dialog', { name: 'Slide show' }), { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: 'Slide show' })).toBeNull();
  });
});
