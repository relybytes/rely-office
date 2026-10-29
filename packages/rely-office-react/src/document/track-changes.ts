// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { Extension } from '@tiptap/core';
import { Fragment, Slice, type Mark, type Node as ProseMirrorNode, type Schema } from '@tiptap/pm/model';
import { Plugin, PluginKey, TextSelection, type EditorState, type Transaction } from '@tiptap/pm/state';
import { AddMarkStep, RemoveMarkStep, ReplaceStep } from '@tiptap/pm/transform';

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    relyTrackRecording: {
      /** Record what is typed and deleted from now on as tracked changes, or stop. */
      setTrackChanges: (enabled: boolean) => ReturnType;
      /** Accept or reject the tracked changes the selection touches. */
      acceptChangesInSelection: () => ReturnType;
      rejectChangesInSelection: () => ReturnType;
    };
  }
}

export interface TrackRecordingOptions {
  /** Who the recorded changes are by. */
  author: () => string;
}

export interface TrackRecordingStorage {
  enabled: boolean;
}

/** Set on a transaction that must not be recorded, as accepting or rejecting a change is not. */
export const TRACK_SKIP = 'relyTrackSkip';

const trackKey = new PluginKey('relyTrackRecording');

/** Put back the marks a tracked formatting change replaced, over `from` to `to`. */
export function restoreFormatting(tr: Transaction, node: ProseMirrorNode, from: number, to: number): void {
  const change = node.marks.find((mark) => mark.type.name === 'formatChange');
  if (!change) return;
  for (const mark of node.marks) {
    if (FORMATTING_MARKS.has(mark.type.name)) tr.removeMark(from, to, mark.type);
  }
  const previous = Array.isArray(change.attrs.previous) ? change.attrs.previous : [];
  for (const json of previous as { type: string; attrs?: Record<string, unknown> }[]) {
    const type = tr.doc.type.schema.marks[json.type];
    if (type && FORMATTING_MARKS.has(json.type)) tr.addMark(from, to, type.create(json.attrs ?? {}));
  }
  tr.removeMark(from, to, change.type);
}

/** The marks that are formatting, which a tracked formatting change records. */
export const FORMATTING_MARKS: ReadonlySet<string> = new Set([
  'bold',
  'italic',
  'underline',
  'strike',
  'code',
  'superscript',
  'subscript',
  'textStyle',
  'highlight',
]);

/** The formatting marks of a text node as JSON, in a stable order, to compare and to keep. */
function formattingOf(node: ProseMirrorNode): { type: string; attrs?: Record<string, unknown> }[] {
  const change = node.marks.find((mark) => mark.type.name === 'formatChange');
  if (change && Array.isArray(change.attrs.previous)) {
    return change.attrs.previous as { type: string; attrs?: Record<string, unknown> }[];
  }
  return currentFormatting(node);
}

function currentFormatting(node: ProseMirrorNode): { type: string; attrs?: Record<string, unknown> }[] {
  return node.marks
    .filter((mark) => FORMATTING_MARKS.has(mark.type.name))
    .map((mark) => mark.toJSON() as { type: string; attrs?: Record<string, unknown> })
    .sort((a, b) => a.type.localeCompare(b.type));
}

/**
 * A deleted stretch as it goes back into the text, marked deleted: what was only a tracked
 * insertion goes for good, as Word drops an insertion its own author deletes.
 */
export function markDeleted(fragment: Fragment, deletion: Mark, schema: Schema): Fragment {
  const insertion = schema.marks.insertion;
  const nodes: ProseMirrorNode[] = [];
  fragment.forEach((node) => {
    if (node.isText) {
      if (insertion && insertion.isInSet(node.marks)) return;
      if (schema.marks.deletion?.isInSet(node.marks)) nodes.push(node);
      else nodes.push(node.mark(deletion.addToSet(node.marks)));
    } else if (node.isInline) {
      nodes.push(node);
    } else {
      nodes.push(node.copy(markDeleted(node.content, deletion, schema)));
    }
  });
  return Fragment.fromArray(nodes);
}

function hasText(fragment: Fragment): boolean {
  let found = false;
  fragment.descendants((node) => {
    if (node.isText || (node.isInline && node.isLeaf)) found = true;
    return !found;
  });
  return found;
}

/**
 * Changes recorded as they are made: typed text is marked inserted, deleted text stays in the
 * document marked deleted, each by the author and the time, until someone accepts or rejects it.
 */
export const RelyTrackRecording = Extension.create<TrackRecordingOptions, TrackRecordingStorage>({
  name: 'relyTrackRecording',
  addOptions() {
    return { author: () => '' };
  },
  addStorage() {
    return { enabled: false };
  },
  addProseMirrorPlugins() {
    const extension = this;
    return [
      new Plugin({
        key: trackKey,
        appendTransaction(transactions, oldState, newState) {
          if (!extension.storage.enabled) return null;
          const { insertion, deletion, formatChange } = newState.schema.marks;
          if (!insertion || !deletion) return null;
          const recorded = transactions.filter(
            (transaction) =>
              transaction.docChanged &&
              !transaction.getMeta(TRACK_SKIP) &&
              !transaction.getMeta('history$') &&
              !transaction.getMeta('appendedTransaction'),
          );
          if (recorded.length === 0) return null;
          const attrs = { author: extension.options.author(), date: new Date().toISOString() };
          const inserted = insertion.create(attrs);
          const deleted = deletion.create(attrs);
          const tr = newState.tr;
          let changed = false;
          let caret: number | null = null;
          let base = oldState.doc;
          for (const transaction of recorded) {
            transaction.steps.forEach((step, index) => {
              if (step instanceof AddMarkStep || step instanceof RemoveMarkStep) {
                const { from, to, mark } = step as (AddMarkStep | RemoveMarkStep) & { from: number; to: number; mark: Mark };
                if (!FORMATTING_MARKS.has(mark.type.name) || !formatChange) return;
                const before = transaction.docs[index] ?? base;
                const later = transaction.mapping.slice(index + 1);
                before.nodesBetween(from, to, (node, position) => {
                  if (!node.isText || insertion.isInSet(node.marks) || deletion.isInSet(node.marks)) return true;
                  const previous = formattingOf(node);
                  const start = tr.mapping.map(later.map(Math.max(from, position), 1), 1);
                  const end = tr.mapping.map(later.map(Math.min(to, position + node.nodeSize), -1), -1);
                  if (end <= start) return false;
                  const now = tr.doc.nodeAt(start);
                  if (now && JSON.stringify(currentFormatting(now)) === JSON.stringify(previous)) {
                    tr.removeMark(start, end, formatChange);
                  } else {
                    tr.addMark(start, end, formatChange.create({ ...attrs, previous }));
                  }
                  changed = true;
                  return false;
                });
                return;
              }
              if (!(step instanceof ReplaceStep)) return;
              const before = transaction.docs[index] ?? base;
              const { from, to, slice } = step as ReplaceStep & { from: number; to: number; slice: Slice };
              const later = transaction.mapping.slice(index + 1);
              const map = (position: number, assoc: number) => tr.mapping.map(later.map(position, assoc), assoc);
              if (slice.size > 0) {
                const start = map(from, -1);
                const end = map(from + slice.size, 1);
                tr.removeMark(start, end, deletion);
                tr.addMark(start, end, inserted);
                changed = true;
              }
              if (to > from) {
                const removed = before.slice(from, to);
                if (!hasText(removed.content)) return;
                const kept = markDeleted(removed.content, deleted, newState.schema);
                if (!hasText(kept)) return;
                const at = map(from, -1);
                tr.replace(at, at, new Slice(kept, removed.openStart, removed.openEnd));
                const backwards = oldState.selection.empty && oldState.selection.from === to;
                caret = backwards ? at : at + (tr.doc.nodeSize - newState.doc.nodeSize);
                changed = true;
              }
            });
            base = transaction.doc;
          }
          if (!changed) return null;
          if (caret !== null) {
            const position = Math.max(0, Math.min(caret, tr.doc.content.size));
            tr.setSelection(TextSelection.near(tr.doc.resolve(position)));
          }
          tr.setMeta(TRACK_SKIP, true);
          return tr;
        },
      }),
    ];
  },
  addCommands() {
    const settle =
      (keep: 'insertion' | 'deletion') =>
      () =>
      ({ state, tr, dispatch }: { state: EditorState; tr: Transaction; dispatch?: (tr: Transaction) => void }) => {
        const { from, to } = state.selection;
        const keepType = state.schema.marks[keep];
        const dropType = state.schema.marks[keep === 'insertion' ? 'deletion' : 'insertion'];
        if (!keepType || !dropType) return false;
        const start = Math.max(0, from === to ? from - 1 : from);
        const end = Math.min(state.doc.content.size, from === to ? to + 1 : to);
        const removals: [number, number][] = [];
        let found = false;
        state.doc.nodesBetween(start, end, (node, position) => {
          if (!node.isText) return true;
          const nodeFrom = position;
          const nodeTo = position + node.nodeSize;
          if (keepType.isInSet(node.marks)) {
            tr.removeMark(nodeFrom, nodeTo, keepType);
            found = true;
          }
          if (dropType.isInSet(node.marks)) {
            removals.push([nodeFrom, nodeTo]);
            found = true;
          }
          const formatChange = state.schema.marks.formatChange;
          if (formatChange?.isInSet(node.marks)) {
            if (keep === 'deletion') restoreFormatting(tr, node, nodeFrom, nodeTo);
            else tr.removeMark(nodeFrom, nodeTo, formatChange);
            found = true;
          }
          return false;
        });
        if (!found) return false;
        if (dispatch) {
          for (const [removeFrom, removeTo] of removals.reverse()) tr.delete(removeFrom, removeTo);
          tr.setMeta(TRACK_SKIP, true);
          dispatch(tr);
        }
        return true;
      };
    return {
      setTrackChanges:
        (enabled) =>
        ({ editor }) => {
          (editor.storage as unknown as Record<string, TrackRecordingStorage>).relyTrackRecording.enabled = enabled;
          editor.view.dispatch(editor.state.tr.setMeta(TRACK_SKIP, true));
          return true;
        },
      acceptChangesInSelection: settle('insertion'),
      rejectChangesInSelection: settle('deletion'),
    };
  },
});
