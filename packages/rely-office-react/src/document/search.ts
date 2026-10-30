// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import { Extension } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { Plugin, PluginKey, TextSelection, type EditorState, type Transaction } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    relySearch: {
      /** Look for a text in the document and mark every place it is; an empty one clears. */
      setSearch: (query: string, caseSensitive?: boolean) => ReturnType;
      /** Select the next place, or the previous one, wrapping at the end. */
      findNext: () => ReturnType;
      findPrevious: () => ReturnType;
      /** Put a text in place of the selected place, then select the next one. */
      replaceMatch: (text: string) => ReturnType;
      /** Put a text in place of every place. */
      replaceAllMatches: (text: string) => ReturnType;
    };
  }
}

/** A place a search found, as positions of the document. */
export interface SearchMatch {
  from: number;
  to: number;
}

export interface SearchState {
  query: string;
  caseSensitive: boolean;
  matches: SearchMatch[];
  /** The index of the selected place, -1 when none is. */
  current: number;
}

export const searchKey = new PluginKey<SearchState>('relySearch');

const EMPTY: SearchState = { query: '', caseSensitive: false, matches: [], current: -1 };

/**
 * Every place a text is in a document. Text is read a block at a time, so a match never spans
 * two paragraphs, and an inline atom (an image, a note reference) breaks it as a space would.
 */
export function findMatches(doc: ProseMirrorNode, query: string, caseSensitive: boolean): SearchMatch[] {
  if (query.length === 0) return [];
  const wanted = caseSensitive ? query : query.toLocaleLowerCase();
  const matches: SearchMatch[] = [];
  doc.descendants((node, position) => {
    if (!node.isTextblock) return true;
    let text = '';
    const offsets: number[] = [];
    node.forEach((child, offset) => {
      if (child.isText) {
        for (let index = 0; index < child.text!.length; index += 1) offsets.push(position + 1 + offset + index);
        text += child.text;
      } else {
        offsets.push(-1);
        text += '\u0000';
      }
    });
    const haystack = caseSensitive ? text : text.toLocaleLowerCase();
    let at = haystack.indexOf(wanted);
    while (at !== -1) {
      const start = offsets[at];
      const end = offsets[at + wanted.length - 1];
      if (start >= 0 && end >= 0) matches.push({ from: start, to: end + 1 });
      at = haystack.indexOf(wanted, at + Math.max(1, wanted.length));
    }
    return false;
  });
  return matches;
}

function nearest(matches: SearchMatch[], position: number): number {
  if (matches.length === 0) return -1;
  const after = matches.findIndex((match) => match.from >= position);
  return after === -1 ? 0 : after;
}

function stateOf(state: EditorState): SearchState {
  return searchKey.getState(state) ?? EMPTY;
}

/** Find and replace: the places a text is, drawn over the document, and the commands over them. */
export const RelySearch = Extension.create({
  name: 'relySearch',
  addProseMirrorPlugins() {
    return [
      new Plugin<SearchState>({
        key: searchKey,
        state: {
          init: () => EMPTY,
          apply: (tr, previous, _old, next) => {
            const meta = tr.getMeta(searchKey) as Partial<SearchState> | undefined;
            if (!meta && !tr.docChanged) return previous;
            const query = meta?.query ?? previous.query;
            const caseSensitive = meta?.caseSensitive ?? previous.caseSensitive;
            const matches = findMatches(next.doc, query, caseSensitive);
            const current =
              meta?.current !== undefined
                ? Math.min(meta.current, matches.length - 1)
                : Math.min(previous.current, matches.length - 1);
            return { query, caseSensitive, matches, current };
          },
        },
        props: {
          decorations: (state) => {
            const { matches, current } = stateOf(state);
            if (matches.length === 0) return DecorationSet.empty;
            return DecorationSet.create(
              state.doc,
              matches.map((match, index) =>
                Decoration.inline(match.from, match.to, {
                  class: index === current ? 'rely-search-match rely-search-current' : 'rely-search-match',
                }),
              ),
            );
          },
        },
      }),
    ];
  },
  addCommands() {
    const select =
      (step: 1 | -1) =>
      () =>
      ({ state, tr, dispatch }: { state: EditorState; tr: Transaction; dispatch?: (tr: Transaction) => void }) => {
        const { matches, current } = stateOf(state);
        if (matches.length === 0) return false;
        const start = current === -1 ? nearest(matches, state.selection.from) : (current + step + matches.length) % matches.length;
        const index = current === -1 && step === -1 ? (start - 1 + matches.length) % matches.length : start;
        if (dispatch) {
          const match = matches[index];
          tr.setSelection(TextSelection.create(tr.doc, match.from, match.to)).scrollIntoView();
          tr.setMeta(searchKey, { current: index });
          dispatch(tr);
        }
        return true;
      };
    return {
      setSearch:
        (query, caseSensitive = false) =>
        ({ state, tr, dispatch }) => {
          if (dispatch) {
            const matches = findMatches(state.doc, query, caseSensitive);
            tr.setMeta(searchKey, { query, caseSensitive, current: nearest(matches, state.selection.from) });
            dispatch(tr);
          }
          return true;
        },
      findNext: select(1),
      findPrevious: select(-1),
      replaceMatch:
        (text) =>
        ({ state, tr, dispatch }) => {
          const { matches, current } = stateOf(state);
          if (current < 0 || current >= matches.length) return false;
          if (dispatch) {
            const match = matches[current];
            if (text.length > 0) tr.insertText(text, match.from, match.to);
            else tr.delete(match.from, match.to);
            const remaining = findMatches(tr.doc, stateOf(state).query, stateOf(state).caseSensitive);
            const next = nearest(remaining, match.from + text.length);
            if (next >= 0) tr.setSelection(TextSelection.create(tr.doc, remaining[next].from, remaining[next].to));
            tr.setMeta(searchKey, { current: next });
            dispatch(tr);
          }
          return true;
        },
      replaceAllMatches:
        (text) =>
        ({ state, tr, dispatch }) => {
          const { matches } = stateOf(state);
          if (matches.length === 0) return false;
          if (dispatch) {
            for (const match of [...matches].reverse()) {
              if (text.length > 0) tr.insertText(text, match.from, match.to);
              else tr.delete(match.from, match.to);
            }
            tr.setMeta(searchKey, { current: -1 });
            dispatch(tr);
          }
          return true;
        },
    };
  },
});
