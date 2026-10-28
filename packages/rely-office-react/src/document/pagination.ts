// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { Extension } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { Plugin, PluginKey, type EditorState } from '@tiptap/pm/state';
import { Decoration, DecorationSet, type EditorView } from '@tiptap/pm/view';

/** What the page view draws: the pages' text height, their margins and their headers. */
export interface PaginationOptions {
  enabled: boolean;
  /** The height of the text of a page, between the margins, in CSS pixels. */
  contentHeight: number;
  marginTop: number;
  marginBottom: number;
  header: string;
  footer: string;
  /** The words under a page: its number. */
  pageLabel: (page: number) => string;
}

export const DEFAULT_PAGINATION: PaginationOptions = {
  enabled: false,
  contentHeight: 0,
  marginTop: 96,
  marginBottom: 96,
  header: '',
  footer: '',
  pageLabel: (page) => String(page),
};

export const paginationKey = new PluginKey<number[]>('relyPagination');

/**
 * Where pages start: the positions of the top-level blocks that open a page, because the one
 * before filled it or because they break the page themselves. A block taller than a page stays
 * whole on its page.
 */
export function pageBreaks(
  doc: ProseMirrorNode,
  heights: (offset: number) => number,
  contentHeight: number,
): number[] {
  const breaks: number[] = [];
  let used = 0;
  doc.forEach((node, offset) => {
    const height = heights(offset);
    const forced = Boolean((node.attrs.format as { pageBreakBefore?: boolean } | null)?.pageBreakBefore);
    if (offset > 0 && used > 0 && (forced || (contentHeight > 0 && used + height > contentHeight))) {
      breaks.push(offset);
      used = 0;
    }
    used += height;
  });
  return breaks;
}

function outerHeight(element: HTMLElement): number {
  const style = window.getComputedStyle(element);
  return element.offsetHeight + (parseFloat(style.marginTop) || 0) + (parseFloat(style.marginBottom) || 0);
}

function separator(page: number, options: PaginationOptions): HTMLElement {
  const widget = document.createElement('div');
  widget.className = 'rely-page-separator';
  widget.contentEditable = 'false';
  const footer = document.createElement('div');
  footer.className = 'rely-page-footer';
  footer.style.height = `${options.marginBottom}px`;
  footer.textContent = options.footer;
  const number = document.createElement('span');
  number.className = 'rely-page-number';
  number.textContent = options.pageLabel(page);
  footer.append(number);
  const gap = document.createElement('div');
  gap.className = 'rely-page-gap';
  const header = document.createElement('div');
  header.className = 'rely-page-header';
  header.style.height = `${options.marginTop}px`;
  header.textContent = options.header;
  widget.append(footer, gap, header);
  return widget;
}

/**
 * The page view: the text drawn on pages of the document's size, a page ending where its text
 * no longer fits or where a paragraph breaks the page, each with its header and footer.
 */
export const RelyPagination = Extension.create<object, { options: PaginationOptions; pages: number }>({
  name: 'relyPagination',
  addStorage() {
    return { options: DEFAULT_PAGINATION, pages: 1 };
  },
  addProseMirrorPlugins() {
    const storage = this.storage;
    let scheduled = false;
    const measure = (view: EditorView) => {
      scheduled = false;
      if ((view as EditorView & { isDestroyed?: boolean }).isDestroyed) return;
      const options = storage.options;
      const breaks = options.enabled
        ? pageBreaks(
            view.state.doc,
            (offset) => {
              const dom = view.nodeDOM(offset);
              return dom instanceof HTMLElement ? outerHeight(dom) : 0;
            },
            options.contentHeight,
          )
        : [];
      const current = paginationKey.getState(view.state) ?? [];
      storage.pages = breaks.length + 1;
      if (breaks.length === current.length && breaks.every((position, index) => position === current[index])) return;
      view.dispatch(view.state.tr.setMeta(paginationKey, breaks).setMeta('addToHistory', false).setMeta('relyTrackSkip', true));
    };
    const schedule = (view: EditorView) => {
      if (scheduled) return;
      scheduled = true;
      const run = () => measure(view);
      if (typeof requestAnimationFrame === 'function') requestAnimationFrame(run);
      else setTimeout(run, 0);
    };
    return [
      new Plugin<number[]>({
        key: paginationKey,
        state: {
          init: () => [],
          apply: (tr, previous) => {
            const meta = tr.getMeta(paginationKey) as number[] | undefined;
            if (meta) return meta;
            return tr.docChanged ? previous.map((position) => tr.mapping.map(position)) : previous;
          },
        },
        props: {
          decorations: (state: EditorState) => {
            const breaks = paginationKey.getState(state) ?? [];
            if (!storage.options.enabled || breaks.length === 0) return DecorationSet.empty;
            return DecorationSet.create(
              state.doc,
              breaks.map((position, index) =>
                Decoration.widget(position, () => separator(index + 1, storage.options), {
                  side: -1,
                  key: `page-${index}-${storage.options.header}-${storage.options.footer}`,
                  ignoreSelection: true,
                }),
              ),
            );
          },
        },
        view: (view) => {
          schedule(view);
          return { update: (updated) => schedule(updated) };
        },
      }),
    ];
  },
});
