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
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactElement,
  type Ref,
} from 'react';
import { mediaDataUrl, sniffImageType } from '../document/media.js';
import type { Frame, Presentation, Slide, SlideRun, SlideShape, TextBody } from '../model.js';
import { writePptx } from '../runtime/documents.js';
import { bodyFromEditedElement } from './edited-text.js';
import { ENGLISH_SLIDE_EDITOR_LABELS, type SlideEditorLabels } from './labels.js';
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
  withoutShape,
  type ResizeHandle,
} from './slide-model.js';
import { SlideView, ShapeView, backgroundCss, frameCss, type MediaUrls } from './slide-view.js';

const THUMBNAIL_WIDTH = 160;
const MAXIMUM_STAGE_WIDTH = 1280;
const NUDGE_EMU = 45_720;
const FONT_SIZES = [10, 12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 44, 54, 60, 72];
const HANDLES: ResizeHandle[] = ['topLeft', 'topRight', 'bottomLeft', 'bottomRight'];
/** The most changes the editor can undo. */
const MAX_UNDO_STEPS = 100;

interface Drag {
  shape: number;
  handle: ResizeHandle | null;
  startX: number;
  startY: number;
  frame: Frame;
  moved: boolean;
}

/** What a host can ask of a mounted slide editor. */
export interface SlideEditorHandle {
  getPresentation: () => Presentation;
  /** The presentation as a pptx; needs `initDocuments` to have loaded the module. */
  savePptx: () => Uint8Array;
  activeSlide: number;
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

/**
 * A slide editor over the slide model: the slides as thumbnails, the current one drawn with its
 * shapes, pictures and tables, where a shape is moved, resized, written into and styled, and the
 * speaker notes under it. What a slide draws from its layout shows locked, as PowerPoint shows it.
 */
export const SlideEditor = forwardRef(function SlideEditor(
  { presentation: initial, onChange, editable = true, labels, onImageFile = readFile, className }: SlideEditorProps,
  ref: Ref<SlideEditorHandle>,
): ReactElement {
  const words = useMemo(() => ({ ...ENGLISH_SLIDE_EDITOR_LABELS, ...labels }), [labels]);
  const [presentation, setPresentation] = useState(initial);
  const [active, setActive] = useState(0);
  const [selected, setSelected] = useState<number | null>(null);
  const [editingText, setEditingText] = useState<number | null>(null);
  const [stageWidth, setStageWidth] = useState(960);
  const stage = useRef<HTMLDivElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const drag = useRef<Drag | null>(null);
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
  const shape = selected === null ? undefined : shapes[selected];
  const scale = stageWidth / presentation.widthEmu;

  /** Change the presentation; `notify` is false while a drag is still going. */
  const update = useCallback((change: (current: Presentation) => Presentation, notify = true) => {
    const next = change(latest.current);
    latest.current = next;
    setPresentation(next);
    if (notify) {
      commit();
      onChangeRef.current?.(next);
    }
  }, [commit]);

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
    setSelected((current) => (current !== null && current < (target.slides[slideIndex]?.shapes?.length ?? 0) ? current : null));
    onChangeRef.current?.(target);
  };

  const updateSlide = useCallback(
    (change: (current: Slide) => Slide, notify = true) =>
      update((current) => ({ ...current, slides: current.slides.map((each, index) => (index === slideIndex ? change(each) : each)) }), notify),
    [update, slideIndex],
  );

  const updateShape = (change: (current: SlideShape) => SlideShape) => {
    if (selected === null) return;
    updateSlide((current) => withShape(current, selected, change));
  };

  const updateText = (change: (body: TextBody) => TextBody) =>
    updateShape((current) => (current.content.type === 'text' ? { ...current, content: { type: 'text', ...change(current.content) } } : current));

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

  useEffect(() => {
    const move = (event: PointerEvent) => {
      const current = drag.current;
      if (!current) return;
      const dx = (event.clientX - current.startX) / scale;
      const dy = (event.clientY - current.startY) / scale;
      if (!current.moved && Math.abs(dx) + Math.abs(dy) < 2 / scale) return;
      current.moved = true;
      const frame = current.handle ? resizedFrame(current.frame, current.handle, dx, dy) : movedFrame(current.frame, dx, dy, latest.current);
      updateSlide((each) => withShape(each, current.shape, (target) => ({ ...target, frame })), false);
    };
    const end = () => {
      if (drag.current?.moved) {
        commit();
        onChangeRef.current?.(latest.current);
      }
      drag.current = null;
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', end);
    return () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', end);
    };
  }, [scale, updateSlide, commit]);

  useImperativeHandle(
    ref,
    () => ({ getPresentation: () => latest.current, savePptx: () => writePptx(latest.current), activeSlide: slideIndex }),
    [slideIndex],
  );

  const placeShape = (created: SlideShape) => {
    updateSlide((current) => ({ ...current, shapes: [...(current.shapes ?? []), created] }));
    setSelected(shapes.length);
    setEditingText(null);
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

  const startDrag = (event: ReactPointerEvent, index: number, handle: ResizeHandle | null) => {
    if (!editable || event.button !== 0) return;
    event.stopPropagation();
    setSelected(index);
    if (editingText === index && handle === null) return;
    setEditingText(null);
    drag.current = { shape: index, handle, startX: event.clientX, startY: event.clientY, frame: shapes[index].frame, moved: false };
  };

  const onStageKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!editable || selected === null || editingText !== null || !shape) return;
    const step = event.shiftKey ? NUDGE_EMU * 10 : NUDGE_EMU;
    const nudges: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    if (event.key in nudges) {
      const [dx, dy] = nudges[event.key];
      updateShape((current) => ({ ...current, frame: movedFrame(current.frame, dx, dy, presentation) }));
    } else if (event.key === 'Delete' || event.key === 'Backspace') {
      updateSlide((current) => withoutShape(current, selected));
      setSelected(null);
    } else if (event.key === 'Enter' && shape.content.type === 'text') {
      setEditingText(selected);
    } else if (event.key === 'Escape') {
      setSelected(null);
    } else {
      return;
    }
    event.preventDefault();
  };

  const run = firstRun(shape);
  const textSelected = shape?.content.type === 'text';
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
            {tool(words.undo, () => travel('undo'), '\u21b6', { disabled: past.current.length === 0 })}
            {tool(words.redo, () => travel('redo'), '\u21b7', { disabled: future.current.length === 0 })}
          </div>
          <div className="rely-tool-group">
            {tool(words.addSlide, () => {
              update((current) => ({ ...current, slides: [...current.slides.slice(0, slideIndex + 1), { shapes: [] }, ...current.slides.slice(slideIndex + 1)] }));
              setActive(slideIndex + 1);
              setSelected(null);
            }, '+')}
            {tool(words.duplicateSlide, () => {
              if (!slide) return;
              update((current) => ({ ...current, slides: [...current.slides.slice(0, slideIndex + 1), duplicatedSlide(slide), ...current.slides.slice(slideIndex + 1)] }));
              setActive(slideIndex + 1);
            }, '⎘', { disabled: !slide })}
            {tool(words.deleteSlide, () => {
              update((current) => ({ ...current, slides: current.slides.filter((_, index) => index !== slideIndex) }));
              setActive(Math.max(0, slideIndex - 1));
              setSelected(null);
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
            {tool(words.rectangle, () => placeShape(newGeometryShape(centred(0.3, 0.25), { type: 'rectangle' })), '▭', { disabled: !slide })}
            {tool(words.ellipse, () => placeShape(newGeometryShape(centred(0.25, 0.3), { type: 'ellipse' })), '◯', { disabled: !slide })}
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
            {tool(words.deleteShape, () => {
              if (selected === null) return;
              updateSlide((current) => withoutShape(current, selected));
              setSelected(null);
            }, '✖', { disabled: !shape })}
            {tool(words.bringForward, () => {
              if (selected === null) return;
              updateSlide((current) => withShapeOrder(current, selected, 1));
              setSelected(Math.min(shapes.length - 1, selected + 1));
            }, '⬆', { disabled: !shape || selected === shapes.length - 1 })}
            {tool(words.sendBackward, () => {
              if (selected === null) return;
              const target = selected - 1;
              if (target < 0 || shapes[target]?.inherited) return;
              updateSlide((current) => withShapeOrder(current, selected, -1));
              setSelected(target);
            }, '⬇', { disabled: !shape || selected === 0 || shapes[(selected ?? 0) - 1]?.inherited === true })}
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
                disabled={!shape}
                value={`#${solidColour(shape?.fill) ?? 'FFFFFF'}`}
                onChange={(event) => updateShape((current) => ({ ...current, fill: { type: 'solid', colour: event.target.value.slice(1).toUpperCase() } }))}
              />
            </label>
            <label className="rely-tool rely-tool-colour" title={words.outline}>
              <span aria-hidden="true">{'▢'}</span>
              <input
                type="color"
                aria-label={words.outline}
                disabled={!shape}
                value={`#${shape?.line?.colour ?? '000000'}`}
                onChange={(event) =>
                  updateShape((current) => ({ ...current, line: { colour: event.target.value.slice(1).toUpperCase(), widthEmu: current.line?.widthEmu ?? 12_700 } }))
                }
              />
            </label>
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
                  setSelected(null);
                  setEditingText(null);
                }}
              >
                <span className="rely-slide-number">{index + 1}</span>
                <SlideView slide={each} widthEmu={presentation.widthEmu} heightEmu={presentation.heightEmu} width={THUMBNAIL_WIDTH} media={media} />
              </button>
            </li>
          ))}
        </ol>

        <div className="rely-slide-main">
          <div ref={stage} className="rely-slide-stage" tabIndex={0} onKeyDown={onStageKeyDown}>
            {slide ? (
              <div
                className="rely-slide"
                role="group"
                aria-label={words.canvas}
                style={{ width: stageWidth, height: presentation.heightEmu * scale, ...backgroundCss(slide.background, media) }}
                onPointerDown={() => {
                  setSelected(null);
                  setEditingText(null);
                }}
              >
                {shapes.map((each, index) => {
                  const isSelected = selected === index;
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
                        if (editable && !each.inherited && each.content.type === 'text') setEditingText(index);
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
                      />
                      {isSelected && editable
                        ? HANDLES.map((handle) => (
                            <span
                              key={handle}
                              className={`rely-slide-handle rely-slide-handle-${handle}`}
                              aria-label={`${words.resize} ${handle}`}
                              onPointerDown={(event) => startDrag(event, index, handle)}
                            />
                          ))
                        : null}
                    </div>
                  );
                })}
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
      </div>
    </div>
  );
});
