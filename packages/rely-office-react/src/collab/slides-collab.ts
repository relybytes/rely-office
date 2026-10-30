// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import * as Y from 'yjs';
import type { Animation, MediaItem, Presentation, Slide, SlideShape } from '../model.js';
import { slotIndex } from './document-collab.js';

/** What of a slide is replaced whole, the last writer's winning. */
const SLIDE_FIELDS = ['background', 'notes', 'hidden', 'transition'] as const;

interface StoredAnimation extends Omit<Animation, 'shape'> {
  /** The key of the shape, not its index, so a shape added by another client does not shift it. */
  shapeKey: string;
}

interface KeyedSlide {
  key: string;
  slide: Slide;
  shapeKeys: string[];
}

/**
 * The keys of a list as it changed: an item that is the same object keeps its key, a changed one
 * takes the key of a gone one, the one at its place first, and the rest are new.
 */
export function matchKeys<T>(
  before: readonly T[],
  beforeKeys: readonly string[],
  after: readonly T[],
  newKey: () => string,
): { keys: string[]; changed: Set<string> } {
  const byItem = new Map<T, number[]>();
  before.forEach((item, index) => byItem.set(item, [...(byItem.get(item) ?? []), index]));
  const used = new Set<number>();
  const keys: (string | null)[] = after.map((item) => {
    const candidates = byItem.get(item);
    const index = candidates?.find((candidate) => !used.has(candidate));
    if (index === undefined) return null;
    used.add(index);
    return beforeKeys[index]!;
  });
  const changed = new Set<string>();
  for (let index = 0; index < after.length; index += 1) {
    if (keys[index] !== null || index >= before.length || used.has(index) || beforeKeys[index] === undefined) continue;
    used.add(index);
    keys[index] = beforeKeys[index]!;
    changed.add(keys[index]!);
  }
  const gone = before.map((_, index) => index).filter((index) => !used.has(index) && beforeKeys[index] !== undefined);
  for (let index = 0; index < after.length; index += 1) {
    if (keys[index] !== null) continue;
    const reused = gone.shift();
    keys[index] = reused === undefined ? newKey() : beforeKeys[reused]!;
    changed.add(keys[index]!);
  }
  return { keys: keys as string[], changed };
}

/** The positions of the longest run of `values` that only grows. */
function longestIncreasing(values: readonly number[]): Set<number> {
  const tails: number[] = [];
  const previous = new Array<number>(values.length).fill(-1);
  for (let index = 0; index < values.length; index += 1) {
    let low = 0;
    let high = tails.length;
    while (low < high) {
      const middle = (low + high) >> 1;
      if (values[tails[middle]!]! < values[index]!) low = middle + 1;
      else high = middle;
    }
    if (low > 0) previous[index] = tails[low - 1]!;
    tails[low] = index;
  }
  const kept = new Set<number>();
  for (let index = tails[tails.length - 1] ?? -1; index >= 0; index = previous[index]!) kept.add(index);
  return kept;
}

/**
 * Change a shared list of keys from `before` to `after`: what went is deleted, what came or moved
 * goes after the key before it. Keys another client added that this one has not seen stay where
 * they are.
 */
export function applyKeyChanges(array: Y.Array<string>, before: readonly string[], after: readonly string[]): void {
  const kept = new Set(after);
  for (const key of before) {
    if (kept.has(key)) continue;
    const at = array.toArray().indexOf(key);
    if (at >= 0) array.delete(at, 1);
  }
  const oldPosition = new Map(before.map((key, index) => [key, index]));
  const surviving = after.filter((key) => oldPosition.has(key));
  const steady = longestIncreasing(surviving.map((key) => oldPosition.get(key)!));
  const stay = new Set(surviving.filter((_, index) => steady.has(index)));
  after.forEach((key, index) => {
    if (stay.has(key)) return;
    const existing = array.toArray().indexOf(key);
    if (existing >= 0) array.delete(existing, 1);
    const predecessor = index > 0 ? array.toArray().indexOf(after[index - 1]!) : -1;
    array.insert(predecessor + 1, [key]);
  });
}

/** The keys of a Yjs array once each, in order: two clients reordering together can repeat one. */
function uniqueKeys(array: Y.Array<string>): string[] {
  const seen = new Set<string>();
  return array.toArray().filter((key) => (seen.has(key) ? false : (seen.add(key), true)));
}

/**
 * A presentation edited together: the slides and their shapes by key, so two clients editing
 * different shapes, or adding slides at once, keep both changes; the slide size whole; the
 * images by index. Undo takes back this client's changes only.
 */
export class SharedPresentation {
  readonly doc: Y.Doc;
  readonly slot: number;
  private readonly root: Y.Map<unknown>;
  private readonly order: Y.Array<string>;
  private readonly slides: Y.Map<Y.Map<unknown>>;
  private readonly media: Y.Map<unknown>;
  private readonly origin = {};
  private readonly undoManager: Y.UndoManager;
  private counter = 0;
  private last: { presentation: Presentation; slides: KeyedSlide[] } | null = null;
  private readonly shapeCache = new Map<string, { json: unknown; shape: SlideShape }>();

  constructor(doc: Y.Doc, slot: number) {
    this.doc = doc;
    this.slot = slot;
    this.root = doc.getMap('presentation');
    this.order = doc.getArray<string>('slideOrder');
    this.slides = doc.getMap<Y.Map<unknown>>('slides');
    this.media = doc.getMap('media');
    this.undoManager = new Y.UndoManager([this.root, this.order, this.slides, this.media], {
      trackedOrigins: new Set([this.origin]),
      captureTimeout: 300,
    });
  }

  /** Whether nothing was put into the shared document yet. */
  isEmpty(): boolean {
    return this.root.size === 0;
  }

  /** Put a whole presentation in: what a seeding client does once. */
  seed(presentation: Presentation): void {
    this.doc.transact(() => {
      this.writeAll(presentation);
    }, 'seed');
    this.last = null;
    this.read();
  }

  /** The presentation as the shared document holds it now. */
  read(): Presentation {
    const keyed: KeyedSlide[] = [];
    for (const key of uniqueKeys(this.order)) {
      const shared = this.slides.get(key);
      if (!shared) continue;
      const order = shared.get('shapeOrder') as Y.Array<string> | undefined;
      const shapes = shared.get('shapes') as Y.Map<unknown> | undefined;
      const shapeKeys = order ? uniqueKeys(order).filter((shapeKey) => shapes?.has(shapeKey)) : [];
      const slide: Slide = { shapes: shapeKeys.map((shapeKey) => this.shapeOf(shapeKey, shapes!.get(shapeKey))) };
      for (const field of SLIDE_FIELDS) {
        const value = shared.get(field);
        if (value !== undefined) (slide as Record<string, unknown>)[field] = value;
      }
      const animations = shared.get('animations') as StoredAnimation[] | undefined;
      if (animations) {
        slide.animations = animations.flatMap(({ shapeKey, ...animation }) => {
          const index = shapeKeys.indexOf(shapeKey);
          return index < 0 ? [] : [{ ...animation, shape: index }];
        });
      }
      keyed.push({ key, slide, shapeKeys });
    }
    const media: MediaItem[] = [...this.media.entries()]
      .map(([index, value]) => {
        const item = value as { contentType: string; bytes: Uint8Array };
        return { index: Number(index), contentType: item.contentType, bytes: item.bytes };
      })
      .sort((first, second) => first.index - second.index);
    const presentation: Presentation = {
      widthEmu: (this.root.get('widthEmu') as number | undefined) ?? 12_192_000,
      heightEmu: (this.root.get('heightEmu') as number | undefined) ?? 6_858_000,
      truncated: (this.root.get('truncated') as boolean | undefined) ?? false,
      slides: keyed.map((entry) => entry.slide),
      media,
    };
    this.last = { presentation, slides: keyed };
    return presentation;
  }

  /**
   * Write what changed since the last read or write. An image this client added under an index
   * another client took moves to one of its own, and the presentation returned says so.
   */
  write(next: Presentation): Presentation {
    if (!this.last) this.read();
    const previous = this.last!;
    let written = next;
    this.doc.transact(() => {
      written = this.writeMedia(next, previous.presentation);
      for (const field of ['widthEmu', 'heightEmu', 'truncated'] as const) {
        if (this.root.get(field) !== written[field]) this.root.set(field, written[field]);
      }
      const { keys } = matchKeys(
        previous.presentation.slides,
        previous.slides.map((entry) => entry.key),
        written.slides,
        () => this.newKey(),
      );
      const before = new Map(previous.slides.map((entry) => [entry.key, entry]));
      const keyedSlides: KeyedSlide[] = [];
      written.slides.forEach((slide, index) => {
        const key = keys[index]!;
        const old = before.get(key);
        if (old && old.slide === slide) {
          keyedSlides.push(old);
          return;
        }
        keyedSlides.push(this.writeSlide(key, slide, old));
      });
      applyKeyChanges(this.order, previous.slides.map((entry) => entry.key), keys);
      for (const key of before.keys()) {
        if (!keys.includes(key)) this.slides.delete(key);
      }
      this.last = { presentation: written, slides: keyedSlides };
    }, this.origin);
    return written;
  }

  canUndo(): boolean {
    return this.undoManager.undoStack.length > 0;
  }

  canRedo(): boolean {
    return this.undoManager.redoStack.length > 0;
  }

  /** Take back this client's last change; false when there is none. */
  undo(): boolean {
    return this.undoManager.undo() !== null;
  }

  /** Put back what the last undo took away; false when there is none. */
  redo(): boolean {
    return this.undoManager.redo() !== null;
  }

  /** End the current undo step, so the next change is a step of its own. */
  stopCapturing(): void {
    this.undoManager.stopCapturing();
  }

  /** Called when the presentation changed by another client, or by undo and redo. */
  observe(listener: () => void): () => void {
    const handler = (_events: unknown, transaction: Y.Transaction) => {
      if (transaction.origin !== this.origin) listener();
    };
    const shared = [this.root, this.order, this.slides, this.media];
    for (const type of shared) type.observeDeep(handler);
    return () => {
      for (const type of shared) type.unobserveDeep(handler);
    };
  }

  /** Stop tracking undo; the shared document stays the session's. */
  destroy(): void {
    this.undoManager.destroy();
  }

  private newKey(): string {
    this.counter += 1;
    return `${this.doc.clientID.toString(36)}.${this.counter.toString(36)}`;
  }

  private shapeOf(key: string, json: unknown): SlideShape {
    const cached = this.shapeCache.get(key);
    if (cached && cached.json === json) return cached.shape;
    const shape = json as SlideShape;
    this.shapeCache.set(key, { json, shape });
    return shape;
  }

  private writeAll(presentation: Presentation): void {
    this.root.set('widthEmu', presentation.widthEmu);
    this.root.set('heightEmu', presentation.heightEmu);
    this.root.set('truncated', presentation.truncated);
    for (const item of presentation.media) {
      this.media.set(String(item.index), { contentType: item.contentType, bytes: item.bytes });
    }
    const keys = presentation.slides.map(() => this.newKey());
    presentation.slides.forEach((slide, index) => this.writeSlide(keys[index]!, slide, undefined));
    this.order.insert(this.order.length, keys);
  }

  private writeSlide(key: string, slide: Slide, old: KeyedSlide | undefined): KeyedSlide {
    let shared = this.slides.get(key);
    if (!shared) {
      shared = new Y.Map<unknown>();
      shared.set('shapeOrder', new Y.Array<string>());
      shared.set('shapes', new Y.Map<unknown>());
      this.slides.set(key, shared);
    }
    for (const field of SLIDE_FIELDS) {
      const value = slide[field];
      if (JSON.stringify(value) === JSON.stringify(old?.slide[field])) continue;
      if (value === undefined) shared.delete(field);
      else shared.set(field, value);
    }
    const shapes = slide.shapes ?? [];
    const { keys: shapeKeys } = matchKeys(old?.slide.shapes ?? [], old?.shapeKeys ?? [], shapes, () => this.newKey());
    const store = shared.get('shapes') as Y.Map<unknown>;
    const previous = new Map((old?.shapeKeys ?? []).map((shapeKey, index) => [shapeKey, old!.slide.shapes![index]]));
    shapes.forEach((shape, index) => {
      const shapeKey = shapeKeys[index]!;
      if (previous.get(shapeKey) === shape && store.has(shapeKey)) return;
      store.set(shapeKey, shape);
      this.shapeCache.set(shapeKey, { json: store.get(shapeKey), shape });
    });
    applyKeyChanges(shared.get('shapeOrder') as Y.Array<string>, old?.shapeKeys ?? [], shapeKeys);
    for (const shapeKey of previous.keys()) {
      if (!shapeKeys.includes(shapeKey)) store.delete(shapeKey);
    }
    const animations: StoredAnimation[] | undefined = slide.animations?.flatMap(({ shape, ...animation }) => {
      const shapeKey = shapeKeys[shape];
      return shapeKey === undefined ? [] : [{ ...animation, shapeKey }];
    });
    if (JSON.stringify(animations) !== JSON.stringify(shared.get('animations'))) {
      if (animations === undefined) shared.delete('animations');
      else shared.set('animations', animations);
    }
    return { key, slide, shapeKeys };
  }

  private writeMedia(next: Presentation, previous: Presentation): Presentation {
    const known = new Set(previous.media.map((item) => item.index));
    const moved = new Map<number, number>();
    const media: MediaItem[] = [];
    for (const item of next.media) {
      if (known.has(item.index)) {
        media.push(item);
        continue;
      }
      let index = item.index;
      if (this.media.has(String(index))) {
        let highest = index;
        for (const key of this.media.keys()) highest = Math.max(highest, Number(key));
        index = slotIndex(highest + 1, this.slot);
        moved.set(item.index, index);
      }
      this.media.set(String(index), { contentType: item.contentType, bytes: item.bytes });
      media.push({ ...item, index });
    }
    for (const index of known) {
      if (!next.media.some((item) => item.index === index)) this.media.delete(String(index));
    }
    if (moved.size === 0) return next;
    const remap = (index: number) => moved.get(index) ?? index;
    const slides = next.slides.map((slide) => ({
      ...slide,
      shapes: slide.shapes?.map((shape) => ({
        ...shape,
        content: shape.content.type === 'picture' ? { ...shape.content, media: remap(shape.content.media) } : shape.content,
        fill: shape.fill?.type === 'picture' ? { ...shape.fill, media: remap(shape.fill.media) } : shape.fill,
      })),
    }));
    return { ...next, media, slides };
  }
}
