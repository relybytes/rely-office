// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type { JSONContent } from '@tiptap/core';
import type { Block, Comment, DocumentTree, HeadersFooters, Inline, Mark } from '../model.js';
import { mapBlockInlines } from './media.js';

/** Every list of blocks a tree holds outside its body: notes, headers and footers, comments. */
function asideBlockLists(tree: DocumentTree): Block[][] {
  const headers = (value: HeadersFooters): Block[][] =>
    Object.values(value).filter((blocks): blocks is Block[] => Array.isArray(blocks));
  return [
    ...tree.notes.map((note) => note.blocks),
    ...headers(tree.headersFooters),
    ...(tree.sections ?? []).flatMap((section) => headers(section.headersFooters)),
    ...(tree.comments ?? []).map((comment) => comment.blocks),
  ];
}

/** Rewrite every block list of a tree, its body and the rest. */
function mapTreeBlocks(tree: DocumentTree, visit: (inline: Inline) => Inline | null): DocumentTree {
  const remap = (blocks: Block[]) => mapBlockInlines(blocks, visit);
  const headers = (value: HeadersFooters): HeadersFooters =>
    Object.fromEntries(
      Object.entries(value).map(([key, blocks]) => [key, Array.isArray(blocks) ? remap(blocks) : blocks]),
    ) as unknown as HeadersFooters;
  return {
    ...tree,
    blocks: remap(tree.blocks),
    notes: tree.notes.map((note) => ({ ...note, blocks: remap(note.blocks) })),
    headersFooters: headers(tree.headersFooters),
    ...(tree.sections === undefined
      ? {}
      : { sections: tree.sections.map((section) => ({ ...section, headersFooters: headers(section.headersFooters) })) }),
    ...(tree.comments === undefined
      ? {}
      : { comments: tree.comments.map((comment) => ({ ...comment, blocks: remap(comment.blocks) })) }),
  };
}

function visitInlines(blocks: Block[], visit: (inline: Inline) => void): void {
  mapBlockInlines(blocks, (inline) => {
    visit(inline);
    return inline;
  });
}

/**
 * Keep the notes something still refers to, numbered in the order the text refers to them, as
 * Word numbers footnotes; a note whose reference was deleted goes with it.
 */
export function compactNotes(tree: DocumentTree): DocumentTree {
  const order: number[] = [];
  const collect = (inline: Inline) => {
    if (inline.type === 'noteReference' && tree.notes[inline.note] && !order.includes(inline.note)) {
      order.push(inline.note);
    }
  };
  visitInlines(tree.blocks, collect);
  asideBlockLists(tree).forEach((blocks) => visitInlines(blocks, collect));
  const renumbered = new Map(order.map((note, index) => [note, index]));
  const mapped = mapTreeBlocks(tree, (inline) => {
    if (inline.type !== 'noteReference') return inline;
    const note = renumbered.get(inline.note);
    return note === undefined ? null : { ...inline, note };
  });
  return { ...mapped, notes: order.map((note) => mapped.notes[note]) };
}

/**
 * Keep the comments the text still marks or anchors, with their replies; a comment whose text
 * was deleted goes with its replies, and the rest are numbered again in their order.
 */
export function compactComments(tree: DocumentTree): DocumentTree {
  const comments = tree.comments;
  if (comments === undefined) return tree;
  const used = new Set<number>();
  const collect = (inline: Inline) => {
    if (inline.type === 'commentAnchor') used.add(inline.comment);
    if (inline.type === 'text') {
      for (const mark of inline.marks ?? []) if (mark.type === 'comment') used.add(mark.comment);
    }
  };
  visitInlines(tree.blocks, collect);
  asideBlockLists({ ...tree, comments: undefined }).forEach((blocks) => visitInlines(blocks, collect));
  const rootOf = (index: number, seen = new Set<number>()): number => {
    const parent = comments[index]?.parent;
    if (parent === undefined || seen.has(index) || comments[parent] === undefined) return index;
    seen.add(index);
    return rootOf(parent, seen);
  };
  const usedRoots = new Set([...used].filter((index) => comments[index] !== undefined).map((index) => rootOf(index)));
  const kept = comments.map((_, index) => index).filter((index) => used.has(index) || usedRoots.has(rootOf(index)));
  const renumbered = new Map(kept.map((index, position) => [index, position]));

  const mapped = mapTreeBlocks({ ...tree, comments: undefined }, (inline) => {
    if (inline.type === 'commentAnchor') {
      const comment = renumbered.get(inline.comment);
      return comment === undefined ? null : { ...inline, comment };
    }
    if (inline.type !== 'text' || !inline.marks?.some((mark) => mark.type === 'comment')) return inline;
    const marks = inline.marks.flatMap((mark): Mark[] => {
      if (mark.type !== 'comment') return [mark];
      const comment = renumbered.get(mark.comment);
      return comment === undefined ? [] : [{ ...mark, comment }];
    });
    const { marks: _marks, ...rest } = inline;
    return marks.length > 0 ? { ...rest, marks } : rest;
  });
  const keptComments: Comment[] = kept.map((index) => {
    const comment = comments[index];
    const parent = comment.parent === undefined ? undefined : renumbered.get(comment.parent);
    const { parent: _parent, ...rest } = comment;
    return parent === undefined ? rest : { ...rest, parent };
  });
  const { comments: _comments, ...withoutComments } = mapped;
  return keptComments.length > 0 ? { ...withoutComments, comments: keptComments } : withoutComments;
}

/** What the editor's content still refers to: the comments it marks or anchors, the notes in order. */
export interface ContentReferences {
  comments: Set<number>;
  /** The notes in the order the text refers to them, each once. */
  notes: number[];
}

/** The comments and notes the editor's content refers to. */
export function contentReferences(content: JSONContent): ContentReferences {
  const comments = new Set<number>();
  const notes: number[] = [];
  const visit = (node: JSONContent): void => {
    if (node.type === 'commentAnchor') comments.add(Number(node.attrs?.comment));
    if (node.type === 'noteReference') {
      const note = Number(node.attrs?.note);
      if (!notes.includes(note)) notes.push(note);
    }
    for (const mark of node.marks ?? []) {
      if (mark.type === 'comment') comments.add(Number(mark.attrs?.comment));
    }
    node.content?.forEach(visit);
  };
  visit(content);
  return { comments, notes };
}

/** The comment a reply answers, followed up to the first of its thread. */
export function threadRoot(comments: Comment[], index: number): number {
  let current = index;
  const seen = new Set<number>();
  while (comments[current]?.parent !== undefined && !seen.has(current)) {
    seen.add(current);
    current = comments[current].parent!;
  }
  return current;
}
