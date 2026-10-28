// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent as ReactClipboardEvent,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactElement,
  type Ref,
} from 'react';
import { mediaDataUrl, sniffImageType } from '../document/media.js';
import type { Frame, Presentation, Slide, SlideRun, SlideShape, SlideTable, TextBody } from '../model.js';
import { writePptx } from '../runtime/documents.js';
import { bodyFromEditedElement } from './edited-text.js';
import { ENGLISH_SLIDE_EDITOR_LABELS, type SlideEditorLabels } from './labels.js';
import { SlideEffectsPanel } from './slide-effects-panel.js';
import {
  duplicatedSlide,
  moved,
  movedFrame,
  newGeometryShape,
  newPictureShape,
  newTextShape,
  resizedFrame,
  solidColour,
  textBodyText,
  withAlignment,
  withMedia,
  withNotesText,
  withRunStyle,
  withShape,
  withShapeOrder,
  type ResizeHandle,
} from './slide-model.js';
import { SlideShow } from './slide-show.js';
import {
  SHAPE_PRESETS,
  framesBounds,
  framesOverlap,
  newTableShape,
  presetGeometry,
  rotationTowards,
  snapFrame,
  withFrame,
  withPastedShapes,
  withTableCell,
  withTableColumn,
  withTableRow,
  withoutShapes,
  withoutTableColumn,
  withoutTableRow,
  type CopiedShapes,
  type ShapePreset,
  type Snapped,
} from './slide-tools.js';
import { SlideView, ShapeView, backgroundCss, frameCss, type MediaUrls, type TableCellPlace } from './slide-view.js';

const THUMBNAIL_WIDTH = 160;
const MAXIMUM_STAGE_WIDTH = 1280;
const NUDGE_EMU = 45_720;
const PASTE_OFFSET_EMU = NUDGE_EMU * 4;
/** How near, in screen pixels, an edge comes to a guide before it snaps to it. */
const SNAP_PIXELS = 6;
const FONT_SIZES = [10, 12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 44, 54, 60, 72];
const HANDLES: ResizeHandle[] = ['topLeft', 'topRight', 'bottomLeft', 'bottomRight'];
/** The most changes the editor can undo. */
const MAX_UNDO_STEPS = 100;

/** Shapes copied in any slide editor of the page, ready to paste into another one. */
let copiedShapes: { copied: CopiedShapes; text: string } | null = null;

type Drag =
  | { kind: 'move'; shapes: number[]; frames: Frame[]; startX: number; startY: number; moved: boolean }
  | { kind: 'resize'; shape: number; original: SlideShape; handle: ResizeHandle; startX: number; startY: number; moved: boolean }
  | { kind: 'rotate'; shape: number; frame: Frame; moved: boolean }
  | { kind: 'marquee'; originX: number; originY: number; startX: number; startY: number; additive: boolean; base: number[]; moved: boolean };

interface Marquee {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** What a host can ask of a mounted slide editor. */
export interface SlideEditorHandle {
  getPresentation: () => Presentation;
  /** The presentation as a pptx; needs `initDocuments` to have loaded the module. */
  savePptx: () => Uint8Array;
  activeSlide: number;
  /** Play the show from a slide, the current one when none is named. */
  play: (slide?: number) => void;
}

export interface SlideEditorProps {
  /** The presentation to open; the editor keeps its own state after that, so key it to open another. */
  presentation: Presentation;
  /** Called after every change with the presentation as it is now. */
  onChange?: (presentation: Presentation) => void;
  /** False shows the slides without letting them change. */
  editable?: boolean;
  /** The words of the editor, in the host's language; English by default. */
  labels?: Partial<SlideEditorLabels>;
  /** The bytes of an image the user chose, or null to refuse it; by default the file's bytes. */
  onImageFile?: (file: File) => Promise<Uint8Array | null>;
  className?: string;
}

async function readFile(file: File): Promise<Uint8Array | null> {
  if (typeof file.arrayBuffer === 'function') return new Uint8Array(await file.arrayBuffer());
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer));
    reader.onerror = () => reject(reader.error);
    reader.readAsArrayBuffer(file);
  });
}

function firstRun(shape: SlideShape | undefined): SlideRun | undefined {
  return shape?.content.type === 'text' ? shape.content.paragraphs[0]?.runs.find((run) => !run.lineBreak) : undefined;
}

function PresetIcon({ preset }: Readonly<{ preset: ShapePreset }>): ReactElement {
  const shape = newGeometryShape({ x: 0, y: 0, width: 100, height: 100 }, presetGeometry(preset, 100, 100));
  return (
    <span className="rely-slide-preset-icon" aria-hidden="true">
      <ShapeView shape={{ ...shape, line: { colour: '1F2328', widthEmu: 6 }, fill: { type: 'solid', colour: 'DBEAFE' } }} scale={0.24} media={new Map()} />
    </span>
  );
}

/**
 * A slide editor over the slide model: the slides as thumbnails, the current one drawn with its
 * shapes, pictures and tables, where shapes are selected one or several at a time, moved along
 * guides, resized, rotated, copied, written into and styled; the transitions and animations of
 * the show, the show itself, and the speaker notes under the slide. What a slide draws from its
 * layout shows locked, as PowerPoint shows it.
 */
export const SlideEditor = forwardRef(function SlideEditor(
  { presentation: initial, onChange, editable = true, labels, onImageFile = readFile, className }: SlideEditorProps,
  ref: Ref<SlideEditorHandle>,
): ReactElement {
  const words = useMemo(() => ({ ...ENGLISH_SLIDE_EDITOR_LABELS, ...labels }), [labels]);
  const [presentation, setPresentation] = useState(initial);
  const [active, setActive] = useState(0);
  const [selected, setSelected] = useState<number[]>([]);
  const [editingText, setEditingText] = useState<number | null>(null);
  const [activeCell, setActiveCell] = useState<TableCellPlace | null>(null);
  const [stageWidth, setStageWidth] = useState(960);
  const [guides, setGuides] = useState<Snapped['guides'] | null>(null);
  const [marquee, setMarquee] = useState<Marquee | null>(null);
  const [showing, setShowing] = useState<number | null>(null);
  const [effectsOpen, setEffectsOpen] = useState(false);
  const [shapesOpen, setShapesOpen] = useState(false);
  const stage = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLDivElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const drag = useRef<Drag | null>(null);
  const pastes = useRef(0);
  const latest = useRef(presentation);
  latest.current = presentation;
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  // The presentation as the last finished change left it, and the ones before and after it.
  const committed = useRef(presentation);
  const past = useRef<Presentation[]>([]);
  const future = useRef<Presentation[]>([]);
  const [, setHistoryVersion] = useState(0);

  /** Record the change that led to the presentation as it is now. */
  const commit = useCallback(() => {
    past.current = [...past.current.slice(-(MAX_UNDO_STEPS - 1)), committed.current];
    future.current = [];
    committed.current = latest.current;
    setHistoryVersion((version) => version + 1);
  }, []);

  const media: MediaUrls = useMemo(
    () => new Map(presentation.media.map((item) => [item.index, mediaDataUrl(item)])),
    [presentation.media],
  );
  const slideIndex = Math.min(active, Math.max(0, presentation.slides.length - 1));
  const slide: Slide | undefined = presentation.slides[slideIndex];
  const shapes = slide?.shapes ?? [];
  const primary = selected.length > 0 ? selected[selected.length - 1] : null;
  const shape = primary === null ? undefined : shapes[primary];
  const scale = stageWidth / presentation.widthEmu;

  /** Change the presentation; `notify` is false while a drag is still going. */
  const update = useCallback(
    (change: (current: Presentation) => Presentation, notify = true) => {
      const next = change(latest.current);
      latest.current = next;
      setPresentation(next);
      if (notify) {
        commit();
        onChangeRef.current?.(next);
      }
    },
    [commit],
  );

  /** Go one change back (`undo`) or forward (`redo`). */
  const travel = (direction: 'undo' | 'redo') => {
    const from = direction === 'undo' ? past : future;
    const to = direction === 'undo' ? future : past;
    const target = from.current[from.current.length - 1];
    if (!editable || target === undefined) return;
    from.current = from.current.slice(0, -1);
    to.current = [...to.current, latest.current];
    latest.current = target;
    committed.current = target;
    setPresentation(target);
    setHistoryVersion((version) => version + 1);
    setEditingText(null);
    const count = target.slides[slideIndex]?.shapes?.length ?? 0;
    setSelected((current) => current.filter((index) => index < count));
    onChangeRef.current?.(target);
  };

  const updateSlide = useCallback(
    (change: (current: Slide) => Slide, notify = true) =>
      update((current) => ({ ...current, slides: current.slides.map((each, index) => (index === slideIndex ? change(each) : each)) }), notify),
    [update, slideIndex],
  );

  /** Change every selected shape. */
  const updateShapes = (change: (current: SlideShape) => SlideShape) => {
    if (selected.length === 0) return;
    updateSlide((current) => selected.reduce((each, index) => withShape(each, index, change), current));
  };

  const updateText = (change: (body: TextBody) => TextBody) =>
    updateShapes((current) => (current.content.type === 'text' ? { ...current, content: { type: 'text', ...change(current.content) } } : current));

  const updateTable = (change: (table: SlideTable) => SlideTable) => {
    if (primary === null) return;
    updateSlide((current) =>
      withShape(current, primary, (target) => (target.content.type === 'table' ? { ...target, content: { type: 'table', ...change(target.content) } } : target)),
    );
  };

  useLayoutEffect(() => {
    const element = stage.current;
    if (!element) return;
    // The slide fits the stage both ways, as PowerPoint fits it to the window.
    const measure = () => {
      const style = getComputedStyle(element);
      const width = element.clientWidth - parseFloat(style.paddingLeft || '0') - parseFloat(style.paddingRight || '0');
      const height = element.clientHeight - parseFloat(style.paddingTop || '0') - parseFloat(style.paddingBottom || '0');
      const byHeight = height > 0 ? (height * latest.current.widthEmu) / latest.current.heightEmu : Number.POSITIVE_INFINITY;
      if (width > 0) setStageWidth(Math.min(MAXIMUM_STAGE_WIDTH, width, byHeight));
    };
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  /** A point of the page as EMU of the slide. */
  const slidePoint = useCallback(
    (clientX: number, clientY: number) => {
      const box = canvas.current?.getBoundingClientRect();
      return { x: (clientX - (box?.left ?? 0)) / scale, y: (clientY - (box?.top ?? 0)) / scale };
    },
    [scale],
  );

  useEffect(() => {
    const move = (event: PointerEvent) => {
      const current = drag.current;
      if (!current) return;
      if (current.kind === 'rotate') {
        current.moved = true;
        const rotation = rotationTowards(current.frame, slidePoint(event.clientX, event.clientY), event.shiftKey);
        updateSlide((each) => withShape(each, current.shape, (target) => ({ ...target, rotation })), false);
        return;
      }
      if (current.kind === 'marquee') {
        const point = slidePoint(event.clientX, event.clientY);
        const box = {
          x: Math.min(point.x, current.originX),
          y: Math.min(point.y, current.originY),
          width: Math.abs(point.x - current.originX),
          height: Math.abs(point.y - current.originY),
        };
        if (!current.moved && Math.abs(event.clientX - current.startX) + Math.abs(event.clientY - current.startY) < 3) return;
        current.moved = true;
        setMarquee(box);
        const touched = (latest.current.slides[slideIndex]?.shapes ?? [])
          .map((each, index) => (!each.inherited && framesOverlap(box, each.frame) ? index : -1))
          .filter((index) => index >= 0);
        setSelected(current.additive ? [...new Set([...current.base, ...touched])] : touched);
        return;
      }
      const dx = (event.clientX - current.startX) / scale;
      const dy = (event.clientY - current.startY) / scale;
      if (!current.moved && Math.abs(dx) + Math.abs(dy) < 2 / scale) return;
      current.moved = true;
      if (current.kind === 'resize') {
        const frame = resizedFrame(current.original.frame, current.handle, dx, dy);
        updateSlide((each) => withShape(each, current.shape, () => withFrame(current.original, frame)), false);
        return;
      }
      const bounds = framesBounds(current.frames)!;
      let shiftX = dx;
      let shiftY = dy;
      if (!event.altKey) {
        const others = (latest.current.slides[slideIndex]?.shapes ?? [])
          .filter((_, index) => !current.shapes.includes(index))
          .map((each) => each.frame);
        const snapped = snapFrame({ ...bounds, x: bounds.x + dx, y: bounds.y + dy }, others, latest.current, SNAP_PIXELS / scale);
        shiftX = snapped.frame.x - bounds.x;
        shiftY = snapped.frame.y - bounds.y;
        setGuides(snapped.guides.vertical.length + snapped.guides.horizontal.length > 0 ? snapped.guides : null);
      }
      updateSlide(
        (each) =>
          current.shapes.reduce(
            (result, index, position) =>
              withShape(result, index, (target) => ({ ...target, frame: movedFrame(current.frames[position], shiftX, shiftY, latest.current) })),
            each,
          ),
        false,
      );
    };
    const end = () => {
      const current = drag.current;
      drag.current = null;
      setGuides(null);
      setMarquee(null);
      if (!current) return;
      if (current.kind === 'marquee') {
        if (!current.moved && !current.additive) setSelected([]);
        return;
      }
      if (current.moved) {
        commit();
        onChangeRef.current?.(latest.current);
      }
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', end);
    return () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', end);
    };
  }, [scale, updateSlide, commit, slidePoint, slideIndex]);

  useImperativeHandle(
    ref,
    () => ({
      getPresentation: () => latest.current,
      savePptx: () => writePptx(latest.current),
      activeSlide: slideIndex,
      play: (slideNumber?: number) => setShowing(slideNumber ?? slideIndex),
    }),
    [slideIndex],
  );

  const select = (indexes: number[]) => {
    setSelected(indexes);
    setEditingText(null);
    setActiveCell(null);
  };

  const placeShape = (created: SlideShape) => {
    updateSlide((current) => ({ ...current, shapes: [...(current.shapes ?? []), created] }));
    select([shapes.length]);
    setShapesOpen(false);
  };

  const centred = (widthShare: number, heightShare: number): Frame => {
    const width = Math.round(presentation.widthEmu * widthShare);
    const height = Math.round(presentation.heightEmu * heightShare);
    return { x: Math.round((presentation.widthEmu - width) / 2), y: Math.round((presentation.heightEmu - height) / 2), width, height };
  };

  const insertImage = async (file: File) => {
    const bytes = await onImageFile(file);
    if (bytes === null) return;
    const contentType = sniffImageType(bytes);
    let index = -1;
    update((current) => {
      const added = withMedia(current, bytes, contentType);
      index = added.index;
      return added.presentation;
    }, false);
    placeShape(newPictureShape(centred(0.4, 0.4), index));
  };

  const insertPreset = (preset: ShapePreset) => {
    const frame = preset === 'line' ? centred(0.3, 0.001) : centred(0.25, 0.3);
    placeShape(newGeometryShape(frame, presetGeometry(preset, frame.width, frame.height)));
  };

  const copySelection = (): string | null => {
    const chosen = selected.map((index) => shapes[index]).filter((each): each is SlideShape => Boolean(each) && !each.inherited);
    if (chosen.length === 0) return null;
    const usedMedia = new Set(
      chosen.flatMap((each) => [
        ...(each.content.type === 'picture' ? [each.content.media] : []),
        ...(each.fill?.type === 'picture' ? [each.fill.media] : []),
      ]),
    );
    const text = chosen
      .map((each) => (each.content.type === 'text' ? textBodyText(each.content) : ''))
      .filter(Boolean)
      .join('\n');
    copiedShapes = {
      copied: { shapes: structuredClone(chosen), media: presentation.media.filter((item) => usedMedia.has(item.index)) },
      text,
    };
    pastes.current = 0;
    return text;
  };

  const deleteSelection = () => {
    const removable = selected.filter((index) => !shapes[index]?.inherited);
    if (removable.length === 0) return;
    updateSlide((current) => withoutShapes(current, removable));
    select([]);
  };

  const pasteShapes = (copied: CopiedShapes) => {
    pastes.current += 1;
    let added: number[] = [];
    update((current) => {
      const result = withPastedShapes(current, slideIndex, copied, PASTE_OFFSET_EMU * pastes.current);
      added = result.added;
      return result.presentation;
    });
    select(added);
  };

  const duplicateSelection = () => {
    const chosen = selected.map((index) => shapes[index]).filter((each): each is SlideShape => Boolean(each) && !each.inherited);
    if (chosen.length === 0) return;
    let added: number[] = [];
    update((current) => {
      const result = withPastedShapes(current, slideIndex, { shapes: chosen, media: [] }, PASTE_OFFSET_EMU);
      added = result.added;
      return result.presentation;
    });
    select(added);
  };

  const pasteFromEvent = (event: ReactClipboardEvent<HTMLDivElement>) => {
    if (!editable || !slide || editingText !== null) return;
    const file = [...(event.clipboardData.files ?? [])].find((each) => each.type.startsWith('image/'));
    if (file) {
      event.preventDefault();
      void insertImage(file);
      return;
    }
    const text = event.clipboardData.getData('text/plain');
    if (copiedShapes && (text === '' || text === copiedShapes.text)) {
      event.preventDefault();
      pasteShapes(copiedShapes.copied);
    } else if (text.trim() !== '') {
      event.preventDefault();
      placeShape(newTextShape(centred(0.5, 0.2), text));
    }
  };

  const startDrag = (event: ReactPointerEvent, index: number, handle: ResizeHandle | 'rotate' | null) => {
    if (!editable || event.button !== 0) return;
    event.stopPropagation();
    const additive = event.shiftKey || event.ctrlKey || event.metaKey;
    if (handle === null && additive) {
      const next = selected.includes(index) ? selected.filter((each) => each !== index) : [...selected, index];
      setSelected(next);
      setEditingText(null);
      return;
    }
    if (editingText === index && handle === null) return;
    const chosen = handle === null && selected.includes(index) ? [...selected.filter((each) => each !== index), index] : [index];
    setSelected(chosen);
    setEditingText(null);
    const target = shapes[index];
    if (handle === 'rotate') drag.current = { kind: 'rotate', shape: index, frame: target.frame, moved: false };
    else if (handle) drag.current = { kind: 'resize', shape: index, original: target, handle, startX: event.clientX, startY: event.clientY, moved: false };
    else {
      const movable = chosen.filter((each) => !shapes[each]?.inherited);
      drag.current = { kind: 'move', shapes: movable, frames: movable.map((each) => shapes[each].frame), startX: event.clientX, startY: event.clientY, moved: false };
    }
  };

  const onStageKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!editable || editingText !== null) return;
    const command = event.ctrlKey || event.metaKey;
    const key = event.key.toLowerCase();
    if (command && key === 'a') {
      select(shapes.map((_, index) => index).filter((index) => !shapes[index].inherited));
    } else if (command && key === 'd') {
      duplicateSelection();
    } else if (selected.length > 0) {
      const step = event.shiftKey ? NUDGE_EMU * 10 : NUDGE_EMU;
      const nudges: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
      if (event.key in nudges) {
        const [dx, dy] = nudges[event.key];
        updateShapes((current) => (current.inherited ? current : { ...current, frame: movedFrame(current.frame, dx, dy, presentation) }));
      } else if (event.key === 'Delete' || event.key === 'Backspace') {
        deleteSelection();
      } else if (event.key === 'Enter' && primary !== null && (shape?.content.type === 'text' || shape?.content.type === 'table')) {
        setEditingText(primary);
        if (shape.content.type === 'table') setActiveCell((current) => current ?? { row: 0, column: 0 });
      } else if (event.key === 'Escape') {
        select([]);
      } else {
        return;
      }
    } else {
      return;
    }
    event.preventDefault();
  };

  const run = firstRun(shape);
  const textSelected = selected.some((index) => shapes[index]?.content.type === 'text');
  const tableSelected = shape?.content.type === 'table';
  const cell = activeCell ?? { row: 0, column: 0 };
  const tableCell = tableSelected && shape.content.type === 'table' ? shape.content.rows[cell.row]?.cells[cell.column] : undefined;
  const tool = (label: string, onClick: () => void, glyph: string, options: { disabled?: boolean; pressed?: boolean } = {}) => (
    <button
      type="button"
      className="rely-tool"
      aria-label={label}
      title={label}
      disabled={options.disabled}
      aria-pressed={options.pressed}
      onMouseDown={(event) => event.preventDefault()}
      onClick={onClick}
    >
      {glyph}
    </button>
  );

  return (
    <div
      className={['rely-slide-editor', className].filter(Boolean).join(' ')}
      onKeyDown={(event) => {
        // Text being written and the notes keep the browser's own undo.
        const target = event.target as HTMLElement;
        if (editingText !== null || target.isContentEditable || target.tagName === 'TEXTAREA' || target.tagName === 'INPUT') return;
        if (event.key === 'F5') {
          event.preventDefault();
          setShowing(event.shiftKey ? slideIndex : 0);
          return;
        }
        const key = event.key.toLowerCase();
        if (!(event.ctrlKey || event.metaKey)) return;
        if (key === 'z' && !event.shiftKey) travel('undo');
        else if (key === 'y' || (key === 'z' && event.shiftKey)) travel('redo');
        else return;
        event.preventDefault();
      }}
    >
      {editable ? (
        <div className="rely-toolbar" role="toolbar" aria-label={words.toolbar}>
          <div className="rely-tool-group">
            {tool(words.undo, () => travel('undo'), '↶', { disabled: past.current.length === 0 })}
            {tool(words.redo, () => travel('redo'), '↷', { disabled: future.current.length === 0 })}
          </div>
          <div className="rely-tool-group">
            {tool(words.addSlide, () => {
              update((current) => ({ ...current, slides: [...current.slides.slice(0, slideIndex + 1), { shapes: [] }, ...current.slides.slice(slideIndex + 1)] }));
              setActive(slideIndex + 1);
              select([]);
            }, '+')}
            {tool(words.duplicateSlide, () => {
              if (!slide) return;
              update((current) => ({ ...current, slides: [...current.slides.slice(0, slideIndex + 1), duplicatedSlide(slide), ...current.slides.slice(slideIndex + 1)] }));
              setActive(slideIndex + 1);
            }, '⎘', { disabled: !slide })}
            {tool(words.deleteSlide, () => {
              update((current) => ({ ...current, slides: current.slides.filter((_, index) => index !== slideIndex) }));
              setActive(Math.max(0, slideIndex - 1));
              select([]);
            }, '−', { disabled: presentation.slides.length <= 1 })}
            {tool(words.moveSlideUp, () => {
              update((current) => ({ ...current, slides: moved(current.slides, slideIndex, slideIndex - 1) }));
              setActive(slideIndex - 1);
            }, '↑', { disabled: slideIndex === 0 })}
            {tool(words.moveSlideDown, () => {
              update((current) => ({ ...current, slides: moved(current.slides, slideIndex, slideIndex + 1) }));
              setActive(slideIndex + 1);
            }, '↓', { disabled: slideIndex >= presentation.slides.length - 1 })}
            {tool(words.hideSlide, () => updateSlide((current) => ({ ...current, hidden: !current.hidden })), '◌', {
              disabled: !slide,
              pressed: slide?.hidden ?? false,
            })}
          </div>
          <div className="rely-tool-group">
            {tool(words.textBox, () => placeShape(newTextShape(centred(0.5, 0.15), words.newText)), 'T', { disabled: !slide })}
            {tool(words.rectangle, () => insertPreset('rectangle'), '▭', { disabled: !slide })}
            {tool(words.ellipse, () => insertPreset('ellipse'), '◯', { disabled: !slide })}
            <span className="rely-slide-menu-anchor">
              <button
                type="button"
                className="rely-tool"
                aria-label={words.shapes}
                title={words.shapes}
                aria-expanded={shapesOpen}
                disabled={!slide}
                onClick={() => setShapesOpen((open) => !open)}
              >
                {'△▾'}
              </button>
              {shapesOpen ? (
                <div className="rely-slide-shape-menu" role="menu" aria-label={words.shapes}>
                  {SHAPE_PRESETS.map((preset) => (
                    <button
                      key={preset}
                      type="button"
                      role="menuitem"
                      aria-label={words.shapeName(preset)}
                      title={words.shapeName(preset)}
                      onClick={() => insertPreset(preset)}
                    >
                      <PresetIcon preset={preset} />
                    </button>
                  ))}
                </div>
              ) : null}
            </span>
            {tool(words.table, () => placeShape(newTableShape(centred(0.6, 0.4), 3, 3)), '⊞', { disabled: !slide })}
            {tool(words.image, () => fileInput.current?.click(), '▣', { disabled: !slide })}
            <input
              ref={fileInput}
              type="file"
              accept="image/png,image/jpeg,image/gif,image/webp"
              hidden
              data-testid="rely-slide-image-input"
              onChange={(event) => {
                const file = event.target.files?.[0];
                event.target.value = '';
                if (file) void insertImage(file);
              }}
            />
          </div>
          <div className="rely-tool-group">
            {tool(words.copy, () => void copySelection(), '⧉', { disabled: selected.length === 0 })}
            {tool(words.cut, () => {
              if (copySelection() !== null) deleteSelection();
            }, '✂', { disabled: selected.length === 0 })}
            {tool(words.paste, () => copiedShapes && pasteShapes(copiedShapes.copied), '⎀', { disabled: !slide })}
            {tool(words.duplicate, duplicateSelection, '⿻', { disabled: selected.length === 0 })}
            {tool(words.deleteShape, deleteSelection, '✖', { disabled: selected.length === 0 })}
            {tool(words.bringForward, () => {
              if (primary === null) return;
              updateSlide((current) => withShapeOrder(current, primary, 1));
              setSelected([Math.min(shapes.length - 1, primary + 1)]);
            }, '⬆', { disabled: selected.length !== 1 || primary === shapes.length - 1 })}
            {tool(words.sendBackward, () => {
              if (primary === null) return;
              const target = primary - 1;
              if (target < 0 || shapes[target]?.inherited) return;
              updateSlide((current) => withShapeOrder(current, primary, -1));
              setSelected([target]);
            }, '⬇', { disabled: selected.length !== 1 || primary === 0 || shapes[(primary ?? 0) - 1]?.inherited === true })}
          </div>
          <div className="rely-tool-group">
            {tool(words.bold, () => updateText((body) => withRunStyle(body, { bold: !run?.bold })), 'B', { disabled: !textSelected, pressed: run?.bold ?? false })}
            {tool(words.italic, () => updateText((body) => withRunStyle(body, { italic: !run?.italic })), 'I', { disabled: !textSelected, pressed: run?.italic ?? false })}
            {tool(words.underline, () => updateText((body) => withRunStyle(body, { underline: !run?.underline })), 'U', { disabled: !textSelected, pressed: run?.underline ?? false })}
            <select
              className="rely-tool-select"
              aria-label={words.fontSize}
              disabled={!textSelected}
              value={run ? Math.round(run.size / 100) : 18}
              onChange={(event) => updateText((body) => withRunStyle(body, { size: Number(event.target.value) * 100 }))}
            >
              {[...new Set([...FONT_SIZES, run ? Math.round(run.size / 100) : 18])].sort((a, b) => a - b).map((size) => (
                <option key={size} value={size}>
                  {size}
                </option>
              ))}
            </select>
            <label className="rely-tool rely-tool-colour" title={words.textColour}>
              <span aria-hidden="true">A</span>
              <input
                type="color"
                aria-label={words.textColour}
                disabled={!textSelected}
                value={`#${run?.colour ?? '000000'}`}
                onChange={(event) => updateText((body) => withRunStyle(body, { colour: event.target.value.slice(1).toUpperCase() }))}
              />
            </label>
            {tool(words.alignLeft, () => updateText((body) => withAlignment(body, 'left')), '≡', { disabled: !textSelected })}
            {tool(words.alignCenter, () => updateText((body) => withAlignment(body, 'center')), '≣', { disabled: !textSelected })}
            {tool(words.alignRight, () => updateText((body) => withAlignment(body, 'right')), '≢', { disabled: !textSelected })}
          </div>
          <div className="rely-tool-group">
            <label className="rely-tool rely-tool-colour" title={words.fill}>
              <span aria-hidden="true">{'▧'}</span>
              <input
                type="color"
                aria-label={words.fill}
                disabled={selected.length === 0}
                value={`#${solidColour(shape?.fill) ?? 'FFFFFF'}`}
                onChange={(event) => updateShapes((current) => ({ ...current, fill: { type: 'solid', colour: event.target.value.slice(1).toUpperCase() } }))}
              />
            </label>
            <label className="rely-tool rely-tool-colour" title={words.outline}>
              <span aria-hidden="true">{'▢'}</span>
              <input
                type="color"
                aria-label={words.outline}
                disabled={selected.length === 0}
                value={`#${shape?.line?.colour ?? '000000'}`}
                onChange={(event) =>
                  updateShapes((current) => ({ ...current, line: { colour: event.target.value.slice(1).toUpperCase(), widthEmu: current.line?.widthEmu ?? 12_700 } }))
                }
              />
            </label>
            <label className="rely-tool rely-slide-rotation" title={words.rotate}>
              <span aria-hidden="true">{'⟳'}</span>
              <input
                type="number"
                aria-label={words.rotate}
                min={0}
                max={359}
                disabled={selected.length !== 1}
                value={shape?.rotation ?? 0}
                onChange={(event) => {
                  const value = ((Math.round(Number(event.target.value) || 0) % 360) + 360) % 360;
                  updateShapes((current) => ({ ...current, rotation: value }));
                }}
              />
            </label>
          </div>
          {tableSelected ? (
            <div className="rely-tool-group">
              {tool(words.addTableRow, () => updateTable((table) => withTableRow(table, cell.row)), '+━')}
              {tool(words.addTableColumn, () => updateTable((table) => withTableColumn(table, cell.column)), '+┃')}
              {tool(words.deleteTableRow, () => {
                updateTable((table) => withoutTableRow(table, cell.row));
                setActiveCell(null);
              }, '−━')}
              {tool(words.deleteTableColumn, () => {
                updateTable((table) => withoutTableColumn(table, cell.column));
                setActiveCell(null);
              }, '−┃')}
              <label className="rely-tool rely-tool-colour" title={words.cellFill}>
                <span aria-hidden="true">{'▦'}</span>
                <input
                  type="color"
                  aria-label={words.cellFill}
                  value={`#${tableCell?.fill ?? 'FFFFFF'}`}
                  onChange={(event) =>
                    updateTable((table) => withTableCell(table, cell.row, cell.column, (each) => ({ ...each, fill: event.target.value.slice(1).toUpperCase() })))
                  }
                />
              </label>
            </div>
          ) : null}
          <div className="rely-tool-group">
            {tool(words.effects, () => setEffectsOpen((open) => !open), '✧', { disabled: !slide, pressed: effectsOpen })}
            {tool(words.play, () => setShowing(slideIndex), '▶', { disabled: !slide })}
            {tool(words.playFromStart, () => setShowing(0), '⏮', { disabled: !slide })}
          </div>
        </div>
      ) : null}

      <div className="rely-slide-workspace">
        <ol className="rely-slide-list" aria-label={words.slides}>
          {presentation.slides.map((each, index) => (
            <li key={index}>
              <button
                type="button"
                className={['rely-slide-thumbnail', each.hidden ? 'rely-slide-hidden' : ''].filter(Boolean).join(' ')}
                aria-label={words.slide(index + 1)}
                aria-current={index === slideIndex ? 'true' : undefined}
                onClick={() => {
                  setActive(index);
                  select([]);
                }}
              >
                <span className="rely-slide-number">{index + 1}</span>
                <SlideView slide={each} widthEmu={presentation.widthEmu} heightEmu={presentation.heightEmu} width={THUMBNAIL_WIDTH} media={media} />
              </button>
            </li>
          ))}
        </ol>

        <div className="rely-slide-main">
          <div
            ref={stage}
            className="rely-slide-stage"
            tabIndex={0}
            onKeyDown={onStageKeyDown}
            onCopy={(event) => {
              if (editingText !== null) return;
              const text = copySelection();
              if (text === null) return;
              event.clipboardData.setData('text/plain', text);
              event.preventDefault();
            }}
            onCut={(event) => {
              if (editingText !== null || !editable) return;
              const text = copySelection();
              if (text === null) return;
              event.clipboardData.setData('text/plain', text);
              event.preventDefault();
              deleteSelection();
            }}
            onPaste={pasteFromEvent}
          >
            {slide ? (
              <div
                ref={canvas}
                className="rely-slide"
                role="group"
                aria-label={words.canvas}
                style={{ width: stageWidth, height: presentation.heightEmu * scale, ...backgroundCss(slide.background, media) }}
                onPointerDown={(event) => {
                  setEditingText(null);
                  if (!editable || event.button !== 0) {
                    select([]);
                    return;
                  }
                  const point = slidePoint(event.clientX, event.clientY);
                  const additive = event.shiftKey || event.ctrlKey || event.metaKey;
                  drag.current = { kind: 'marquee', originX: point.x, originY: point.y, startX: event.clientX, startY: event.clientY, additive, base: selected, moved: false };
                }}
              >
                {shapes.map((each, index) => {
                  const isSelected = selected.includes(index);
                  const single = isSelected && selected.length === 1;
                  return (
                    <div
                      key={index}
                      role="button"
                      aria-label={each.inherited ? words.lockedShape : words.shape(index + 1)}
                      aria-pressed={isSelected}
                      aria-disabled={each.inherited || !editable || undefined}
                      className={['rely-slide-shape', each.inherited ? 'rely-slide-locked' : '', isSelected ? 'rely-slide-selected' : ''].filter(Boolean).join(' ')}
                      style={frameCss(each, scale)}
                      onPointerDown={each.inherited ? undefined : (event) => startDrag(event, index, null)}
                      onDoubleClick={() => {
                        if (!editable || each.inherited) return;
                        if (each.content.type === 'text') setEditingText(index);
                        if (each.content.type === 'table') {
                          setEditingText(index);
                          setActiveCell((current) => current ?? { row: 0, column: 0 });
                        }
                      }}
                    >
                      <ShapeView
                        shape={each}
                        scale={scale}
                        media={media}
                        editingText={editingText === index}
                        onCommitText={(element) => {
                          updateSlide((current) =>
                            withShape(current, index, (target) =>
                              target.content.type === 'text'
                                ? { ...target, content: { type: 'text', ...bodyFromEditedElement(element, target.content) } }
                                : target,
                            ),
                          );
                          setEditingText(null);
                        }}
                        activeCell={isSelected ? activeCell : null}
                        onCellDown={
                          each.content.type === 'table' && editable
                            ? (place) => {
                                if (editingText === index && (activeCell?.row !== place.row || activeCell.column !== place.column)) setEditingText(null);
                                setActiveCell(place);
                              }
                            : undefined
                        }
                        onCommitCell={(place, element) => {
                          updateSlide((current) =>
                            withShape(current, index, (target) =>
                              target.content.type === 'table'
                                ? {
                                    ...target,
                                    content: {
                                      type: 'table',
                                      ...withTableCell(target.content, place.row, place.column, (each) => ({ ...each, body: bodyFromEditedElement(element, each.body) })),
                                    },
                                  }
                                : target,
                            ),
                          );
                        }}
                      />
                      {single && editable && !each.inherited ? (
                        <>
                          {HANDLES.map((handle) => (
                            <span
                              key={handle}
                              className={`rely-slide-handle rely-slide-handle-${handle}`}
                              aria-label={`${words.resize} ${handle}`}
                              onPointerDown={(event) => startDrag(event, index, handle)}
                            />
                          ))}
                          <span className="rely-slide-rotate" aria-label={words.rotate} title={words.rotate} onPointerDown={(event) => startDrag(event, index, 'rotate')} />
                        </>
                      ) : null}
                    </div>
                  );
                })}
                {guides?.vertical.map((x) => <span key={`v${x}`} className="rely-slide-guide rely-slide-guide-vertical" style={{ left: x * scale }} />)}
                {guides?.horizontal.map((y) => <span key={`h${y}`} className="rely-slide-guide rely-slide-guide-horizontal" style={{ top: y * scale }} />)}
                {marquee ? (
                  <span
                    className="rely-slide-marquee"
                    style={{ left: marquee.x * scale, top: marquee.y * scale, width: marquee.width * scale, height: marquee.height * scale }}
                  />
                ) : null}
              </div>
            ) : null}
          </div>
          {slide ? (
            <label className="rely-slide-notes">
              <span>{words.notes}</span>
              <textarea
                aria-label={words.notes}
                readOnly={!editable}
                key={slideIndex}
                defaultValue={textBodyText(slide.notes)}
                onBlur={(event) => {
                  const text = event.currentTarget.value;
                  if (text !== textBodyText(latest.current.slides[slideIndex]?.notes)) updateSlide((current) => withNotesText(current, text));
                }}
              />
            </label>
          ) : null}
        </div>
        {effectsOpen && slide && editable ? (
          <SlideEffectsPanel slide={slide} selected={selected} words={words} onChange={(next) => updateSlide(() => next)} onClose={() => setEffectsOpen(false)} />
        ) : null}
      </div>
      {showing !== null ? <SlideShow presentation={presentation} start={showing} labels={words} onClose={() => setShowing(null)} /> : null}
    </div>
  );
});
