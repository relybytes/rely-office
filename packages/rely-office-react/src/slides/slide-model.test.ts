// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import type { Slide, TextBody } from '../model.js';
import {
  DEFAULT_RUN,
  MINIMUM_SHAPE_EMU,
  duplicatedSlide,
  moved,
  movedFrame,
  newGeometryShape,
  newTextShape,
  resizedFrame,
  textBodyText,
  withAlignment,
  withBodyText,
  withMedia,
  withNotesText,
  withRunStyle,
  withShapeOrder,
  withoutShape,
} from './slide-model.js';

const BODY: TextBody = {
  anchor: 'top',
  insetsEmu: [0, 0, 0, 0],
  wrap: true,
  paragraphs: [
    {
      level: 0,
      align: 'center',
      bullet: null,
      marginLeftEmu: 0,
      indentEmu: 0,
      lineSpacingPercent: 100,
      spaceBefore: 0,
      spaceAfter: 0,
      runs: [
        { ...DEFAULT_RUN, text: 'Titolo', size: 4400, bold: true },
        { ...DEFAULT_RUN, text: '', lineBreak: true },
        { ...DEFAULT_RUN, text: 'sotto' },
      ],
    },
    {
      level: 1,
      align: 'left',
      bullet: { type: 'character', text: '•' },
      marginLeftEmu: 457_200,
      indentEmu: -228_600,
      lineSpacingPercent: 90,
      spaceBefore: 600,
      spaceAfter: 0,
      runs: [{ ...DEFAULT_RUN, text: 'punto', italic: true }],
    },
  ],
};

const SIZE = { widthEmu: 12_192_000, heightEmu: 6_858_000 };

describe('text', () => {
  test('a body reads as a line per paragraph, its line breaks included', () => {
    expect(textBodyText(BODY)).toBe('Titolo\nsotto\npunto');
    expect(textBodyText(null)).toBe('');
  });

  test('new text keeps the look of the paragraph it replaces, and of the last for lines after', () => {
    const body = withBodyText(BODY, 'Nuovo\nuno\ndue');

    expect(body.paragraphs.map((paragraph) => paragraph.align)).toEqual(['center', 'left', 'left']);
    expect(body.paragraphs[0].runs).toEqual([{ ...DEFAULT_RUN, text: 'Nuovo', size: 4400, bold: true }]);
    expect(body.paragraphs[2].runs[0]).toMatchObject({ text: 'due', italic: true });
    expect(body.paragraphs[2].bullet).toEqual({ type: 'character', text: '•' });
  });

  test('a body with no paragraph takes the default look', () => {
    expect(withBodyText({ ...BODY, paragraphs: [] }, 'x').paragraphs[0].runs).toEqual([{ ...DEFAULT_RUN, text: 'x' }]);
  });

  test('a style goes on every run but the line breaks, and an alignment on every paragraph', () => {
    const styled = withAlignment(withRunStyle(BODY, { colour: 'FF0000' }), 'right');

    expect(styled.paragraphs[0].runs.map((run) => run.colour)).toEqual(['FF0000', DEFAULT_RUN.colour, 'FF0000']);
    expect(styled.paragraphs.every((paragraph) => paragraph.align === 'right')).toBe(true);
  });
});

describe('frames', () => {
  const frame = { x: 1_000_000, y: 1_000_000, width: 2_000_000, height: 1_000_000 };

  test('a move keeps at least half the shape on the slide', () => {
    expect(movedFrame(frame, 500_000, -200_000, SIZE)).toEqual({ ...frame, x: 1_500_000, y: 800_000 });
    expect(movedFrame(frame, -9_000_000, 99_000_000, SIZE)).toEqual({ ...frame, x: -1_000_000, y: 6_358_000 });
  });

  test('a corner drag resizes from the opposite corner, down to the minimum', () => {
    expect(resizedFrame(frame, 'bottomRight', 100_000, 50_000)).toEqual({ ...frame, width: 2_100_000, height: 1_050_000 });
    expect(resizedFrame(frame, 'topLeft', 100_000, 50_000)).toEqual({ x: 1_100_000, y: 1_050_000, width: 1_900_000, height: 950_000 });
    expect(resizedFrame(frame, 'topRight', -9_000_000, 9_000_000)).toEqual({
      x: 1_000_000,
      y: 2_000_000 - MINIMUM_SHAPE_EMU,
      width: MINIMUM_SHAPE_EMU,
      height: MINIMUM_SHAPE_EMU,
    });
    expect(resizedFrame(frame, 'bottomLeft', 50_000, 0)).toEqual({ ...frame, x: 1_050_000, width: 1_950_000 });
  });
});

describe('slides', () => {
  const slide: Slide = {
    shapes: [
      { ...newTextShape({ x: 0, y: 0, width: 1, height: 1 }, 'layout'), inherited: true },
      { ...newTextShape({ x: 0, y: 0, width: 1, height: 1 }, 'a'), id: 4 },
      newGeometryShape({ x: 0, y: 0, width: 1, height: 1 }, { type: 'ellipse' }),
    ],
    animations: [
      { shape: 1, class: 'entrance', preset: 10, presetSubtype: 0, trigger: 'onClick', delayMs: 0, durationMs: 500 },
      { shape: 2, class: 'exit', preset: 10, presetSubtype: 0, trigger: 'onClick', delayMs: 0, durationMs: 500 },
    ],
  };

  test('a removed shape takes its animation along, and the next ones follow it down', () => {
    const next = withoutShape(slide, 1);

    expect(next.shapes).toHaveLength(2);
    expect(next.animations).toEqual([{ ...slide.animations![1], shape: 1 }]);
  });

  test('a shape moves in the drawing order with its animation, never behind the layout', () => {
    const forward = withShapeOrder(slide, 1, 1);
    expect(forward.shapes![2].content).toMatchObject({ type: 'text' });
    expect(forward.animations!.map((animation) => animation.shape)).toEqual([2, 1]);
    expect(withShapeOrder(slide, 1, -1)).toBe(slide);
    expect(withShapeOrder(slide, 2, 1)).toBe(slide);
  });

  test('a duplicate is a deep copy whose shapes have no id', () => {
    const copy = duplicatedSlide(slide);

    expect(copy.shapes![1].id).toBeUndefined();
    expect(copy.shapes![1]).not.toBe(slide.shapes![1]);
    expect(copy.animations).toEqual(slide.animations);
  });

  test('a list item moves, and a move off the ends changes nothing', () => {
    expect(moved(['a', 'b', 'c'], 0, 2)).toEqual(['b', 'c', 'a']);
    const items = ['a'];
    expect(moved(items, 0, 1)).toBe(items);
  });

  test('notes are written into a body of their own, and empty notes are not created', () => {
    expect(withNotesText({}, '')).toEqual({});
    expect(textBodyText(withNotesText({}, 'Ricordare\ni costi').notes)).toBe('Ricordare\ni costi');
  });

  test('an image joins the media under the next free index', () => {
    const presentation = { ...SIZE, slides: [], truncated: false, media: [{ index: 3, contentType: 'image/png', bytes: new Uint8Array() }] };

    const { presentation: next, index } = withMedia(presentation, new Uint8Array([1]), 'image/jpeg');

    expect(index).toBe(4);
    expect(next.media[1]).toEqual({ index: 4, contentType: 'image/jpeg', bytes: new Uint8Array([1]) });
  });
});
