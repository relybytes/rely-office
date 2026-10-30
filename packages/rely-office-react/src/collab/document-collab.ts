// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import * as Y from 'yjs';
import type { DocumentAside, EditorDocument } from '../document/editor-document.js';
import type { Comment, MediaItem, Note } from '../model.js';
import { MAX_SLOTS } from './protocol.js';

/** The Yjs field the editor's text lives in. */
export const DOCUMENT_CONTENT_FIELD = 'content';

const PLACEHOLDER_COMMENT: Comment = { author: '', date: '', blocks: [], resolved: true };
const PLACEHOLDER_NOTE: Note = { kind: 'footnote', blocks: [] };

/** The parts of the aside that are replaced whole, the last writer's winning. */
const WHOLE_KEYS = [
  'report',
  'headersFooters',
  'page',
  'sections',
  'objects',
  'fonts',
  'fontFiles',
  'defaultFont',
  'macroProject',
] as const;

/**
 * The first index at or past `length` that belongs to `slot`: two clients adding a comment, a
 * note or an image at the same moment never pick the same one.
 */
export function slotIndex(length: number, slot: number): number {
  const wanted = ((slot % MAX_SLOTS) + MAX_SLOTS) % MAX_SLOTS;
  const remainder = length % MAX_SLOTS;
  return length + ((wanted - remainder + MAX_SLOTS) % MAX_SLOTS);
}

function denseArray<T>(map: Y.Map<unknown>, placeholder: T): T[] {
  let highest = -1;
  for (const key of map.keys()) highest = Math.max(highest, Number(key));
  const items: T[] = [];
  for (let index = 0; index <= highest; index += 1) {
    const value = map.get(String(index));
    items.push(value === undefined ? placeholder : (value as T));
  }
  return items;
}

/**
 * What a document editor shares besides its text: comments and notes by index, so two clients
 * adding one together keep both, images by index, and the page, headers and sections whole.
 * The editor's text itself is the Yjs fragment `content`, bound by TipTap's collaboration.
 */
export class SharedDocumentAside {
  readonly doc: Y.Doc;
  readonly slot: number;
  private readonly comments: Y.Map<unknown>;
  private readonly notes: Y.Map<unknown>;
  private readonly media: Y.Map<unknown>;
  private readonly whole: Y.Map<unknown>;
  private readonly origin = {};

  constructor(doc: Y.Doc, slot: number) {
    this.doc = doc;
    this.slot = slot;
    this.comments = doc.getMap('comments');
    this.notes = doc.getMap('notes');
    this.media = doc.getMap('media');
    this.whole = doc.getMap('aside');
  }

  /** Whether nothing was put into the shared document yet. */
  isEmpty(): boolean {
    return this.whole.size === 0 && this.doc.getXmlFragment(DOCUMENT_CONTENT_FIELD).length === 0;
  }

  /** The aside as the editor holds it, with every index that is not used filled by a placeholder. */
  readAside(fallback: DocumentAside): DocumentAside {
    const aside: DocumentAside = { ...fallback };
    for (const key of WHOLE_KEYS) {
      const value = this.whole.get(key);
      if (value !== undefined) (aside as unknown as Record<string, unknown>)[key] = value;
    }
    aside.notes = denseArray<Note>(this.notes, PLACEHOLDER_NOTE);
    if (this.comments.size > 0 || fallback.comments !== undefined) {
      aside.comments = denseArray<Comment>(this.comments, PLACEHOLDER_COMMENT);
    }
    return aside;
  }

  /** The images, by index. */
  readMedia(): MediaItem[] {
    const items: MediaItem[] = [];
    for (const [key, value] of this.media.entries()) {
      const item = value as { contentType: string; bytes: Uint8Array };
      items.push({ index: Number(key), contentType: item.contentType, bytes: item.bytes });
    }
    return items.sort((first, second) => first.index - second.index);
  }

  /** Put a whole document in: what a seeding client does once, before the text. */
  seed(document: EditorDocument): void {
    this.doc.transact(() => {
      for (const key of WHOLE_KEYS) {
        const value = document.aside[key];
        if (value !== undefined) this.whole.set(key, value);
      }
      document.aside.notes.forEach((note, index) => this.notes.set(String(index), note));
      (document.aside.comments ?? []).forEach((comment, index) => this.comments.set(String(index), comment));
      for (const item of document.media) this.media.set(String(item.index), { contentType: item.contentType, bytes: item.bytes });
    }, this.origin);
  }

  /**
   * Write what changed between two asides. Comments appended past the end, as a reply is, move
   * to an index of this client's slot; their position in the returned aside says where they went.
   */
  writeAside(before: DocumentAside, after: DocumentAside): DocumentAside {
    const result: DocumentAside = { ...after };
    this.doc.transact(() => {
      for (const key of WHOLE_KEYS) {
        if (JSON.stringify(before[key]) !== JSON.stringify(after[key])) {
          if (after[key] === undefined) this.whole.delete(key);
          else this.whole.set(key, after[key]);
        }
      }
      result.notes = this.writeList(this.notes, before.notes, after.notes, PLACEHOLDER_NOTE);
      if (after.comments !== undefined) {
        result.comments = this.writeList(this.comments, before.comments ?? [], after.comments, PLACEHOLDER_COMMENT);
      }
    }, this.origin);
    return result;
  }

  /** The index a new comment, note or image of this client takes. */
  allocate(kind: 'comment' | 'note' | 'media', length: number): number {
    const map = kind === 'comment' ? this.comments : kind === 'note' ? this.notes : this.media;
    let highest = length - 1;
    for (const key of map.keys()) highest = Math.max(highest, Number(key));
    return slotIndex(highest + 1, this.slot);
  }

  /** Share a new image. */
  addMedia(item: MediaItem): void {
    this.media.set(String(item.index), { contentType: item.contentType, bytes: item.bytes });
  }

  /** Called when another client changed the aside or the images. */
  observe(listener: () => void): () => void {
    const handler = (_events: unknown, transaction: Y.Transaction) => {
      if (transaction.origin !== this.origin) listener();
    };
    const maps = [this.comments, this.notes, this.media, this.whole];
    for (const map of maps) map.observeDeep(handler);
    return () => {
      for (const map of maps) map.unobserveDeep(handler);
    };
  }

  private writeList<T>(map: Y.Map<unknown>, before: T[], after: T[], placeholder: T): T[] {
    const result = [...after];
    const appended: [number, T][] = [];
    for (let index = 0; index < after.length; index += 1) {
      const item = after[index]!;
      if (index >= before.length) {
        appended.push([index, item]);
        continue;
      }
      if (JSON.stringify(item) !== JSON.stringify(before[index])) map.set(String(index), item);
    }
    for (const [index, item] of appended) {
      if (map.has(String(index)) || index % MAX_SLOTS !== this.slot % MAX_SLOTS) {
        const target = this.allocate(map === this.notes ? 'note' : 'comment', result.length);
        while (result.length < target) result.push(placeholder);
        result[target] = item;
        result[index] = placeholder;
        map.set(String(target), item);
      } else {
        map.set(String(index), item);
      }
    }
    return result;
  }
}
