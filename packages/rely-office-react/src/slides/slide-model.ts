// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import type {
  Frame,
  MediaItem,
  Paint,
  Presentation,
  ShapeGeometry,
  Slide,
  SlideParagraph,
  SlideRun,
  SlideShape,
  TextBody,
} from '../model.js';

/** EMU in an inch, as Office measures slides. */
export const EMU_PER_INCH = 914_400;
/** EMU in a point. */
export const EMU_PER_POINT = 12_700;
/** The smallest side a shape keeps while it is resized. */
export const MINIMUM_SHAPE_EMU = 91_440;

/** A run the editor writes when there is nothing to copy the look from. */
export const DEFAULT_RUN: SlideRun = {
  text: '',
  size: 1800,
  bold: false,
  italic: false,
  underline: false,
  strike: false,
  colour: '000000',
  family: 'Calibri',
  lineBreak: false,
};

export const DEFAULT_PARAGRAPH: Omit<SlideParagraph, 'runs'> = {
  level: 0,
  align: 'left',
  bullet: null,
  marginLeftEmu: 0,
  indentEmu: 0,
  lineSpacingPercent: 100,
  spaceBefore: 0,
  spaceAfter: 0,
};

/** The text of a body, a line per paragraph, as a plain text box shows it. */
export function textBodyText(body: TextBody | null | undefined): string {
  return (body?.paragraphs ?? [])
    .map((paragraph) => paragraph.runs.map((run) => (run.lineBreak ? '\n' : run.text)).join(''))
    .join('\n');
}

/**
 * A body with its text replaced by `text`, a paragraph per line: each line keeps the look of the
 * paragraph it replaces, or of the last one, and the look of that paragraph's first run.
 */
export function withBodyText(body: TextBody, text: string): TextBody {
  const lines = text.split('\n');
  const templates = body.paragraphs.length > 0 ? body.paragraphs : [{ ...DEFAULT_PARAGRAPH, runs: [] }];
  return {
    ...body,
    paragraphs: lines.map((line, index) => {
      const template = templates[Math.min(index, templates.length - 1)];
      const run = template.runs.find((candidate) => !candidate.lineBreak) ?? DEFAULT_RUN;
      return { ...template, runs: [{ ...run, text: line, lineBreak: false }] };
    }),
  };
}

/** A body with every run given a change of look, as a toolbar applies it to a selected shape. */
export function withRunStyle(body: TextBody, patch: Partial<Omit<SlideRun, 'text' | 'lineBreak'>>): TextBody {
  return {
    ...body,
    paragraphs: body.paragraphs.map((paragraph) => ({
      ...paragraph,
      runs: paragraph.runs.map((run) => (run.lineBreak ? run : { ...run, ...patch })),
    })),
  };
}

/** A body with every paragraph aligned the same way. */
export function withAlignment(body: TextBody, align: SlideParagraph['align']): TextBody {
  return { ...body, paragraphs: body.paragraphs.map((paragraph) => ({ ...paragraph, align })) };
}

/** A new text box at a frame, its text in the editor's default look. */
export function newTextShape(frame: Frame, text = ''): SlideShape {
  return {
    frame,
    rotation: 0,
    geometry: { type: 'rectangle' },
    fill: null,
    line: null,
    content: {
      type: 'text',
      anchor: 'top',
      insetsEmu: [91_440, 45_720, 91_440, 45_720],
      wrap: true,
      paragraphs: [{ ...DEFAULT_PARAGRAPH, runs: [{ ...DEFAULT_RUN, text }] }],
    },
  };
}

/** A new drawn shape at a frame, filled and outlined in the editor's default colours. */
export function newGeometryShape(frame: Frame, geometry: ShapeGeometry): SlideShape {
  return {
    frame,
    rotation: 0,
    geometry,
    fill: { type: 'solid', colour: '4472C4' },
    line: { colour: '2F528F', widthEmu: 12_700 },
    content: { type: 'none' },
  };
}

/** A new picture of the presentation's media at a frame. */
export function newPictureShape(frame: Frame, media: number): SlideShape {
  return { frame, rotation: 0, geometry: { type: 'rectangle' }, fill: null, line: null, content: { type: 'picture', media } };
}

/** A frame moved by a distance, kept on the slide. */
export function movedFrame(frame: Frame, dx: number, dy: number, presentation: Pick<Presentation, 'widthEmu' | 'heightEmu'>): Frame {
  return {
    ...frame,
    x: Math.round(Math.min(Math.max(frame.x + dx, -frame.width / 2), presentation.widthEmu - frame.width / 2)),
    y: Math.round(Math.min(Math.max(frame.y + dy, -frame.height / 2), presentation.heightEmu - frame.height / 2)),
  };
}

/** Which corner of a frame a resize drags. */
export type ResizeHandle = 'topLeft' | 'topRight' | 'bottomLeft' | 'bottomRight';

/** A frame resized by dragging a corner, never smaller than `MINIMUM_SHAPE_EMU`. */
export function resizedFrame(frame: Frame, handle: ResizeHandle, dx: number, dy: number): Frame {
  const left = handle === 'topLeft' || handle === 'bottomLeft';
  const top = handle === 'topLeft' || handle === 'topRight';
  const width = Math.max(MINIMUM_SHAPE_EMU, frame.width + (left ? -dx : dx));
  const height = Math.max(MINIMUM_SHAPE_EMU, frame.height + (top ? -dy : dy));
  return {
    x: Math.round(left ? frame.x + frame.width - width : frame.x),
    y: Math.round(top ? frame.y + frame.height - height : frame.y),
    width: Math.round(width),
    height: Math.round(height),
  };
}

/** A slide with one shape changed. */
export function withShape(slide: Slide, index: number, change: (shape: SlideShape) => SlideShape): Slide {
  return { ...slide, shapes: (slide.shapes ?? []).map((shape, at) => (at === index ? change(shape) : shape)) };
}

/** A slide without a shape; its animations follow the shapes that move down. */
export function withoutShape(slide: Slide, index: number): Slide {
  return {
    ...slide,
    shapes: (slide.shapes ?? []).filter((_, at) => at !== index),
    animations: (slide.animations ?? [])
      .filter((animation) => animation.shape !== index)
      .map((animation) => (animation.shape > index ? { ...animation, shape: animation.shape - 1 } : animation)),
  };
}

/** A slide with a shape moved one step forward (+1) or back (-1) in the drawing order. */
export function withShapeOrder(slide: Slide, index: number, step: 1 | -1): Slide {
  const shapes = [...(slide.shapes ?? [])];
  const target = index + step;
  if (target < 0 || target >= shapes.length || shapes[target].inherited) return slide;
  [shapes[index], shapes[target]] = [shapes[target], shapes[index]];
  const swap = (shape: number) => (shape === index ? target : shape === target ? index : shape);
  return { ...slide, shapes, animations: (slide.animations ?? []).map((animation) => ({ ...animation, shape: swap(animation.shape) })) };
}

/** A copy of a slide, its own shapes and its notes, for "duplicate". */
export function duplicatedSlide(slide: Slide): Slide {
  return structuredClone({
    ...slide,
    shapes: (slide.shapes ?? []).map(({ id: _id, ...shape }) => shape),
  });
}

/** The list with an item moved from one place to another. */
export function moved<T>(items: T[], from: number, to: number): T[] {
  if (to < 0 || to >= items.length || from === to) return items;
  const next = [...items];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

/** The speaker notes of a slide with their text replaced. */
export function withNotesText(slide: Slide, text: string): Slide {
  if (text.length === 0 && !slide.notes) return slide;
  const body: TextBody = slide.notes ?? {
    paragraphs: [],
    anchor: 'top',
    insetsEmu: [91_440, 45_720, 91_440, 45_720],
    wrap: true,
  };
  return { ...slide, notes: withBodyText(body, text) };
}

/** A presentation with an image added to its media; answers the index shapes name it by. */
export function withMedia(presentation: Presentation, bytes: Uint8Array, contentType: string): { presentation: Presentation; index: number } {
  const index = presentation.media.reduce((highest, item) => Math.max(highest, item.index + 1), 0);
  const item: MediaItem = { index, contentType, bytes };
  return { presentation: { ...presentation, media: [...presentation.media, item] }, index };
}

/** The solid colour of a paint, `RRGGBB`, or null. */
export function solidColour(paint: Paint | null | undefined): string | null {
  return paint?.type === 'solid' ? paint.colour : null;
}
