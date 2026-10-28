// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// What the slide editor does beyond moving one shape: preset shapes drawn as paths, the guides
// a moved shape snaps to, rotation, several shapes at once, tables and the steps of a show.

import type {
  Animation,
  Frame,
  PathCommand,
  PathPoint,
  Presentation,
  ShapeGeometry,
  Slide,
  SlideShape,
  SlideTable,
  TextBody,
} from '../model.js';
import { DEFAULT_PARAGRAPH, DEFAULT_RUN } from './slide-model.js';

/** The shapes the editor draws beside rectangles and ellipses. */
export type ShapePreset =
  | 'rectangle'
  | 'roundedRectangle'
  | 'ellipse'
  | 'line'
  | 'triangle'
  | 'rightTriangle'
  | 'diamond'
  | 'parallelogram'
  | 'trapezoid'
  | 'pentagon'
  | 'hexagon'
  | 'octagon'
  | 'star'
  | 'arrowRight'
  | 'arrowLeft'
  | 'arrowUp'
  | 'arrowDown'
  | 'chevron'
  | 'plus'
  | 'heart'
  | 'callout';

export const SHAPE_PRESETS: ShapePreset[] = [
  'rectangle',
  'roundedRectangle',
  'ellipse',
  'line',
  'triangle',
  'rightTriangle',
  'diamond',
  'parallelogram',
  'trapezoid',
  'pentagon',
  'hexagon',
  'octagon',
  'star',
  'arrowRight',
  'arrowLeft',
  'arrowUp',
  'arrowDown',
  'chevron',
  'plus',
  'heart',
  'callout',
];

function polygon(points: [number, number][], width: number, height: number): ShapeGeometry {
  const commands: PathCommand[] = points.map(([x, y], index) => ({
    type: index === 0 ? 'moveTo' : 'lineTo',
    to: { x: Math.round(x * width), y: Math.round(y * height) },
  })) as PathCommand[];
  commands.push({ type: 'close' });
  return { type: 'path', paths: [{ commands, filled: true, stroked: true, shade: 'normal' }] };
}

function regular(sides: number, rotation = -Math.PI / 2): [number, number][] {
  return Array.from({ length: sides }, (_, index) => {
    const angle = rotation + (index * 2 * Math.PI) / sides;
    return [0.5 + 0.5 * Math.cos(angle), 0.5 + 0.5 * Math.sin(angle)] as [number, number];
  });
}

/** The outline of a preset at a frame's size, in EMU from its top-left corner. */
export function presetGeometry(preset: ShapePreset, width: number, height: number): ShapeGeometry {
  switch (preset) {
    case 'rectangle':
      return { type: 'rectangle' };
    case 'roundedRectangle':
      return { type: 'roundedRectangle' };
    case 'ellipse':
      return { type: 'ellipse' };
    case 'line':
      return { type: 'line', flipped: false };
    case 'triangle':
      return polygon([[0.5, 0], [1, 1], [0, 1]], width, height);
    case 'rightTriangle':
      return polygon([[0, 0], [1, 1], [0, 1]], width, height);
    case 'diamond':
      return polygon([[0.5, 0], [1, 0.5], [0.5, 1], [0, 0.5]], width, height);
    case 'parallelogram':
      return polygon([[0.25, 0], [1, 0], [0.75, 1], [0, 1]], width, height);
    case 'trapezoid':
      return polygon([[0.2, 0], [0.8, 0], [1, 1], [0, 1]], width, height);
    case 'pentagon':
      return polygon(regular(5), width, height);
    case 'hexagon':
      return polygon([[0.25, 0], [0.75, 0], [1, 0.5], [0.75, 1], [0.25, 1], [0, 0.5]], width, height);
    case 'octagon':
      return polygon([[0.29, 0], [0.71, 0], [1, 0.29], [1, 0.71], [0.71, 1], [0.29, 1], [0, 0.71], [0, 0.29]], width, height);
    case 'star': {
      const points: [number, number][] = [];
      for (let index = 0; index < 10; index += 1) {
        const angle = -Math.PI / 2 + (index * Math.PI) / 5;
        const radius = index % 2 === 0 ? 0.5 : 0.19;
        points.push([0.5 + radius * Math.cos(angle), 0.5 + radius * Math.sin(angle)]);
      }
      return polygon(points, width, height);
    }
    case 'arrowRight':
      return polygon([[0, 0.25], [0.6, 0.25], [0.6, 0], [1, 0.5], [0.6, 1], [0.6, 0.75], [0, 0.75]], width, height);
    case 'arrowLeft':
      return polygon([[1, 0.25], [0.4, 0.25], [0.4, 0], [0, 0.5], [0.4, 1], [0.4, 0.75], [1, 0.75]], width, height);
    case 'arrowUp':
      return polygon([[0.25, 1], [0.25, 0.4], [0, 0.4], [0.5, 0], [1, 0.4], [0.75, 0.4], [0.75, 1]], width, height);
    case 'arrowDown':
      return polygon([[0.25, 0], [0.25, 0.6], [0, 0.6], [0.5, 1], [1, 0.6], [0.75, 0.6], [0.75, 0]], width, height);
    case 'chevron':
      return polygon([[0, 0], [0.7, 0], [1, 0.5], [0.7, 1], [0, 1], [0.3, 0.5]], width, height);
    case 'plus':
      return polygon(
        [[0.33, 0], [0.67, 0], [0.67, 0.33], [1, 0.33], [1, 0.67], [0.67, 0.67], [0.67, 1], [0.33, 1], [0.33, 0.67], [0, 0.67], [0, 0.33], [0.33, 0.33]],
        width,
        height,
      );
    case 'callout':
      return polygon([[0, 0], [1, 0], [1, 0.75], [0.45, 0.75], [0.25, 1], [0.27, 0.75], [0, 0.75]], width, height);
    case 'heart': {
      const point = (x: number, y: number): PathPoint => ({ x: Math.round(x * width), y: Math.round(y * height) });
      const commands: PathCommand[] = [
        { type: 'moveTo', to: point(0.5, 0.3) },
        { type: 'cubicTo', controls: [point(0.5, 0), point(0, 0)], to: point(0, 0.3) },
        { type: 'cubicTo', controls: [point(0, 0.6), point(0.5, 0.8)], to: point(0.5, 1) },
        { type: 'cubicTo', controls: [point(0.5, 0.8), point(1, 0.6)], to: point(1, 0.3) },
        { type: 'cubicTo', controls: [point(1, 0), point(0.5, 0)], to: point(0.5, 0.3) },
        { type: 'close' },
      ];
      return { type: 'path', paths: [{ commands, filled: true, stroked: true, shade: 'normal' }] };
    }
  }
}

/** A geometry drawn at a new size: the points of a path stretched with its frame. */
export function scaledGeometry(geometry: ShapeGeometry, from: Frame, to: Frame): ShapeGeometry {
  if (geometry.type !== 'path' || (from.width === to.width && from.height === to.height)) return geometry;
  const sx = from.width > 0 ? to.width / from.width : 1;
  const sy = from.height > 0 ? to.height / from.height : 1;
  const point = (at: PathPoint): PathPoint => ({ x: Math.round(at.x * sx), y: Math.round(at.y * sy) });
  return {
    type: 'path',
    paths: geometry.paths.map((path) => ({
      ...path,
      commands: path.commands.map((command): PathCommand => {
        switch (command.type) {
          case 'moveTo':
          case 'lineTo':
            return { type: command.type, to: point(command.to) };
          case 'cubicTo':
            return { type: 'cubicTo', controls: [point(command.controls[0]), point(command.controls[1])], to: point(command.to) };
          case 'close':
            return command;
        }
      }),
    })),
  };
}

/** A shape at a new frame, a path of it stretched to fit. */
export function withFrame(shape: SlideShape, frame: Frame): SlideShape {
  return { ...shape, frame, geometry: scaledGeometry(shape.geometry, shape.frame, frame) };
}

/** Where a moved frame snaps and the guide lines it snaps to, in EMU. */
export interface Snapped {
  frame: Frame;
  guides: { vertical: number[]; horizontal: number[] };
}

/**
 * A moved frame snapped to the edges and centres of the other frames and of the slide, when one
 * of its own edges or its centre comes within `threshold` of one; the guides it snapped to.
 */
export function snapFrame(frame: Frame, others: Frame[], slide: { widthEmu: number; heightEmu: number }, threshold: number): Snapped {
  const verticals = [0, slide.widthEmu / 2, slide.widthEmu, ...others.flatMap((other) => [other.x, other.x + other.width / 2, other.x + other.width])];
  const horizontals = [0, slide.heightEmu / 2, slide.heightEmu, ...others.flatMap((other) => [other.y, other.y + other.height / 2, other.y + other.height])];
  const axis = (start: number, size: number, lines: number[]) => {
    let best: { shift: number; line: number } | null = null;
    for (const edge of [start, start + size / 2, start + size]) {
      for (const line of lines) {
        const shift = line - edge;
        if (Math.abs(shift) <= threshold && (best === null || Math.abs(shift) < Math.abs(best.shift))) best = { shift, line };
      }
    }
    if (best === null) return { start, guides: [] as number[] };
    const moved = start + best.shift;
    const guides = [...new Set(lines.filter((line) => [moved, moved + size / 2, moved + size].some((edge) => Math.abs(edge - line) < 1)))];
    return { start: moved, guides };
  };
  const x = axis(frame.x, frame.width, verticals);
  const y = axis(frame.y, frame.height, horizontals);
  return {
    frame: { ...frame, x: Math.round(x.start), y: Math.round(y.start) },
    guides: { vertical: x.guides, horizontal: y.guides },
  };
}

/**
 * The rotation a shape takes when its handle above it is dragged to a point: the angle of the
 * point around the centre, clockwise from straight up, snapped to 15 degrees when asked.
 */
export function rotationTowards(frame: Frame, point: PathPoint, snap = false): number {
  const centreX = frame.x + frame.width / 2;
  const centreY = frame.y + frame.height / 2;
  const degrees = (Math.atan2(point.x - centreX, -(point.y - centreY)) * 180) / Math.PI;
  const turned = ((degrees % 360) + 360) % 360;
  const rounded = snap ? Math.round(turned / 15) * 15 : Math.round(turned);
  return rounded % 360;
}

/** The smallest frame that holds every frame. */
export function framesBounds(frames: Frame[]): Frame | null {
  if (frames.length === 0) return null;
  const left = Math.min(...frames.map((frame) => frame.x));
  const top = Math.min(...frames.map((frame) => frame.y));
  const right = Math.max(...frames.map((frame) => frame.x + frame.width));
  const bottom = Math.max(...frames.map((frame) => frame.y + frame.height));
  return { x: left, y: top, width: right - left, height: bottom - top };
}

/** Whether two frames overlap, for selecting the shapes a dragged box touches. */
export function framesOverlap(first: Frame, second: Frame): boolean {
  return first.x < second.x + second.width && second.x < first.x + first.width && first.y < second.y + second.height && second.y < first.y + first.height;
}

/** Shapes copied to the clipboard of the editor, with the images they show. */
export interface CopiedShapes {
  shapes: SlideShape[];
  media: Presentation['media'];
}

/** The marker that tells shapes this editor copied apart from other text on the clipboard. */
export const CLIPBOARD_MARKER = 'rely-office/slide-shapes';

/** Copies of shapes to paste, without their ids, moved by an offset so they show apart. */
export function pastedShapes(shapes: SlideShape[], offset: number, slide: { widthEmu: number; heightEmu: number }): SlideShape[] {
  return shapes.map(({ id: _id, inherited: _inherited, ...shape }) => {
    const x = Math.min(shape.frame.x + offset, slide.widthEmu - shape.frame.width / 2);
    const y = Math.min(shape.frame.y + offset, slide.heightEmu - shape.frame.height / 2);
    return structuredClone({ ...shape, frame: { ...shape.frame, x: Math.round(x), y: Math.round(y) } });
  });
}

/**
 * A slide with shapes added and the presentation's images they need: an image from another
 * presentation joins this one's media under a new index.
 */
export function withPastedShapes(presentation: Presentation, slideIndex: number, copied: CopiedShapes, offset: number): { presentation: Presentation; added: number[] } {
  const media = [...presentation.media];
  const remap = new Map<number, number>();
  const known = (index: number) => {
    if (remap.has(index)) return remap.get(index)!;
    const source = copied.media.find((item) => item.index === index);
    const same = presentation.media.find((item) => item.index === index);
    if (!source || (same && same.bytes.length === source.bytes.length && same.contentType === source.contentType)) {
      remap.set(index, index);
      return index;
    }
    const next = media.reduce((highest, item) => Math.max(highest, item.index + 1), 0);
    media.push({ ...source, index: next });
    remap.set(index, next);
    return next;
  };
  const shapes = pastedShapes(copied.shapes, offset, presentation).map((shape) => {
    const content = shape.content.type === 'picture' ? { ...shape.content, media: known(shape.content.media) } : shape.content;
    const fill = shape.fill?.type === 'picture' ? { ...shape.fill, media: known(shape.fill.media) } : shape.fill;
    return { ...shape, content, fill };
  });
  const slide = presentation.slides[slideIndex];
  const first = slide.shapes?.length ?? 0;
  return {
    presentation: {
      ...presentation,
      media,
      slides: presentation.slides.map((each, index) => (index === slideIndex ? { ...each, shapes: [...(each.shapes ?? []), ...shapes] } : each)),
    },
    added: shapes.map((_, index) => first + index),
  };
}

/** A slide without several shapes; the animations follow the shapes that stay. */
export function withoutShapes(slide: Slide, indexes: number[]): Slide {
  const removed = new Set(indexes);
  const kept = (slide.shapes ?? []).map((_, index) => index).filter((index) => !removed.has(index));
  const renumber = new Map(kept.map((index, position) => [index, position]));
  return {
    ...slide,
    shapes: kept.map((index) => slide.shapes![index]),
    animations: (slide.animations ?? [])
      .filter((animation) => renumber.has(animation.shape))
      .map((animation) => ({ ...animation, shape: renumber.get(animation.shape)! })),
  };
}

function emptyBody(text = ''): TextBody {
  return {
    anchor: 'top',
    insetsEmu: [91_440, 45_720, 91_440, 45_720],
    wrap: true,
    paragraphs: [{ ...DEFAULT_PARAGRAPH, runs: [{ ...DEFAULT_RUN, size: 1400, text }] }],
  };
}

/** A new table of rows and columns at a frame, its first row a heading. */
export function newTableShape(frame: Frame, rows: number, columns: number): SlideShape {
  const width = Math.round(frame.width / columns);
  const height = Math.round(frame.height / rows);
  return {
    frame,
    rotation: 0,
    geometry: { type: 'rectangle' },
    fill: null,
    line: null,
    content: {
      type: 'table',
      columnWidthsEmu: Array.from({ length: columns }, () => width),
      rows: Array.from({ length: rows }, (_, row) => ({
        heightEmu: height,
        cells: Array.from({ length: columns }, () => ({
          body: emptyBody(),
          fill: row === 0 ? '4472C4' : row % 2 === 0 ? 'E9EBF5' : 'CFD5EA',
          colSpan: 1,
          rowSpan: 1,
          covered: false,
        })),
      })),
    },
  };
}

/** A table with a row added after one, its cells as empty as the row's were filled. */
export function withTableRow(table: SlideTable, after: number): SlideTable {
  const template = table.rows[Math.min(Math.max(after, 0), table.rows.length - 1)];
  const row = {
    heightEmu: template.heightEmu,
    cells: template.cells.map((cell) => ({ ...cell, body: { ...cell.body, paragraphs: emptyBody().paragraphs }, colSpan: 1, rowSpan: 1, covered: false })),
  };
  return { ...table, rows: [...table.rows.slice(0, after + 1), row, ...table.rows.slice(after + 1)] };
}

/** A table with a column added after one. */
export function withTableColumn(table: SlideTable, after: number): SlideTable {
  const width = table.columnWidthsEmu[Math.min(Math.max(after, 0), table.columnWidthsEmu.length - 1)];
  return {
    ...table,
    columnWidthsEmu: [...table.columnWidthsEmu.slice(0, after + 1), width, ...table.columnWidthsEmu.slice(after + 1)],
    rows: table.rows.map((row) => {
      const template = row.cells[Math.min(Math.max(after, 0), row.cells.length - 1)];
      const cell = { ...template, body: { ...template.body, paragraphs: emptyBody().paragraphs }, colSpan: 1, rowSpan: 1, covered: false };
      return { ...row, cells: [...row.cells.slice(0, after + 1), cell, ...row.cells.slice(after + 1)] };
    }),
  };
}

/** A table without a row; the last one stays. */
export function withoutTableRow(table: SlideTable, row: number): SlideTable {
  if (table.rows.length <= 1) return table;
  return { ...table, rows: table.rows.filter((_, index) => index !== row) };
}

/** A table without a column; the last one stays. */
export function withoutTableColumn(table: SlideTable, column: number): SlideTable {
  if (table.columnWidthsEmu.length <= 1) return table;
  return {
    ...table,
    columnWidthsEmu: table.columnWidthsEmu.filter((_, index) => index !== column),
    rows: table.rows.map((row) => ({ ...row, cells: row.cells.filter((_, index) => index !== column) })),
  };
}

/** A table with one cell changed. */
export function withTableCell(
  table: SlideTable,
  row: number,
  column: number,
  change: (cell: SlideTable['rows'][number]['cells'][number]) => SlideTable['rows'][number]['cells'][number],
): SlideTable {
  return {
    ...table,
    rows: table.rows.map((each, rowIndex) =>
      rowIndex === row ? { ...each, cells: each.cells.map((cell, cellIndex) => (cellIndex === column ? change(cell) : cell)) } : each,
    ),
  };
}

/** The transitions the editor offers, by their PresentationML names. */
export const TRANSITION_EFFECTS = ['none', 'fade', 'push', 'wipe', 'split', 'cover', 'pull', 'zoom', 'cut', 'dissolve', 'reveal'];

/** The animations the editor offers: class, preset number and name. */
export const ANIMATION_PRESETS: { class: Animation['class']; preset: number; name: string }[] = [
  { class: 'entrance', preset: 1, name: 'appear' },
  { class: 'entrance', preset: 10, name: 'fade' },
  { class: 'entrance', preset: 2, name: 'fly' },
  { class: 'entrance', preset: 22, name: 'wipe' },
  { class: 'entrance', preset: 53, name: 'zoom' },
  { class: 'emphasis', preset: 6, name: 'grow' },
  { class: 'emphasis', preset: 8, name: 'spin' },
  { class: 'exit', preset: 1, name: 'disappear' },
  { class: 'exit', preset: 10, name: 'fade' },
  { class: 'exit', preset: 2, name: 'fly' },
];

/** The name of an animation's preset, or its number when the editor has no name for it. */
export function animationName(animation: Pick<Animation, 'class' | 'preset'>): string {
  return ANIMATION_PRESETS.find((each) => each.class === animation.class && each.preset === animation.preset)?.name ?? String(animation.preset);
}

/** A slide with an animation moved one step earlier (-1) or later (+1) in its order. */
export function withAnimationOrder(slide: Slide, index: number, step: 1 | -1): Slide {
  const animations = [...(slide.animations ?? [])];
  const target = index + step;
  if (target < 0 || target >= animations.length) return slide;
  [animations[index], animations[target]] = [animations[target], animations[index]];
  return { ...slide, animations };
}

/** One effect of a step of the show: which shape, how, and when after the click. */
export interface ShowEffect {
  shape: number;
  animation: Animation;
  startMs: number;
}

/**
 * The steps of a slide in the show: the first runs as the slide comes in, every click after
 * runs the next. An effect `withPrevious` starts with the one before it, `afterPrevious` when
 * it ends; each starts after its own delay.
 */
export function showSteps(slide: Slide): ShowEffect[][] {
  const steps: ShowEffect[][] = [[]];
  let lastStart = 0;
  let lastEnd = 0;
  for (const animation of slide.animations ?? []) {
    if (animation.trigger === 'onClick') {
      steps.push([]);
      lastStart = 0;
      lastEnd = 0;
    }
    const base = animation.trigger === 'afterPrevious' ? lastEnd : animation.trigger === 'withPrevious' ? lastStart : 0;
    const startMs = base + Math.max(0, animation.delayMs);
    steps[steps.length - 1].push({ shape: animation.shape, animation, startMs });
    lastStart = startMs;
    lastEnd = Math.max(lastEnd, startMs + Math.max(0, animation.durationMs));
  }
  return steps;
}

/**
 * Which shapes show before any effect of a slide runs: every one but those whose first effect
 * is an entrance, which come in with it.
 */
export function initiallyHidden(slide: Slide): Set<number> {
  const seen = new Set<number>();
  const hidden = new Set<number>();
  for (const animation of slide.animations ?? []) {
    if (seen.has(animation.shape)) continue;
    seen.add(animation.shape);
    if (animation.class === 'entrance') hidden.add(animation.shape);
  }
  return hidden;
}

/** The slides a show plays, the hidden ones left out. */
export function shownSlides(presentation: Presentation): number[] {
  return presentation.slides.map((_, index) => index).filter((index) => !presentation.slides[index].hidden);
}
