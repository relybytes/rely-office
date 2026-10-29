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
  /** The header and the footer of a page, its number and the page count filled in. */
  header: (page: number, pages: number) => string;
  footer: (page: number, pages: number) => string;
  /** The words under a page: its number; `null` when the footer or the header shows it. */
  pageLabel: ((page: number) => string) | null;
}

export const DEFAULT_PAGINATION: PaginationOptions = {
  enabled: false,
  contentHeight: 0,
  marginTop: 96,
  marginBottom: 96,
  header: () => '',
  footer: () => '',
  pageLabel: (page) => String(page),
};

/** The height of the gap drawn between two pages, as the stylesheet sets it. */
const PAGE_GAP = 24;
/** The positions a long paragraph is measured at, past which it is left whole. */
const MAX_MEASURED_POSITIONS = 20_000;

export const paginationKey = new PluginKey<number[]>('relyPagination');

/** A stretch of a block that a page can end after: a line of a paragraph, a row of a table. */
export interface BlockSegment {
  /** Where the stretch starts in the document. */
  position: number;
  height: number;
}

/**
 * Where pages start: before a top-level block that no longer fits or that breaks the page
 * itself, or inside a block that does not fit, at the line or the row the page ends before. A
 * block is split only when `segments` gives at least four stretches of it, so no page keeps a
 * single line of it; a block left whole that is taller than a page runs over its page.
 */
export function pageBreaks(
  doc: ProseMirrorNode,
  heights: (offset: number) => number,
  contentHeight: number,
  segments: (offset: number, node: ProseMirrorNode) => BlockSegment[] | null = () => null,
): number[] {
  const breaks: number[] = [];
  let used = 0;
  doc.forEach((node, offset) => {
    const height = heights(offset);
    const forced = Boolean((node.attrs.format as { pageBreakBefore?: boolean } | null)?.pageBreakBefore);
    if (offset > 0 && used > 0 && forced) {
      breaks.push(offset);
      used = 0;
    }
    if (contentHeight <= 0 || used + height <= contentHeight) {
      used += height;
      return;
    }
    const stretches = segments(offset, node);
    if (!stretches || stretches.length < 4) {
      if (offset > 0 && used > 0) {
        breaks.push(offset);
        used = 0;
      }
      used += height;
      return;
    }
    const before = used;
    for (let index = 0; index < stretches.length; index += 1) {
      const stretch = stretches[index];
      if (used > 0 && used + stretch.height > contentHeight) {
        // A page does not end after the first line of a block, nor start with its last.
        let at = index;
        if (at === 1 && before > 0) at = 0;
        else if (at === stretches.length - 1 && at > 1) at -= 1;
        if (at === 0 && before === 0) at = index;
        const position = at === 0 ? offset : stretches[at].position;
        if (breaks[breaks.length - 1] !== position) breaks.push(position);
        used = stretches.slice(at, index).reduce((sum, kept) => sum + kept.height, 0);
      }
      used += stretch.height;
    }
  });
  return breaks;
}

function outerHeight(element: HTMLElement): number {
  const style = window.getComputedStyle(element);
  return element.offsetHeight + (parseFloat(style.marginTop) || 0) + (parseFloat(style.marginBottom) || 0);
}

/** The height of what the page view itself drew inside a block, which is not the block's own. */
function drawnInside(element: HTMLElement, options: PaginationOptions): number {
  let height = 0;
  element.querySelectorAll('.rely-page-separator').forEach((separator) => {
    height += (separator as HTMLElement).offsetHeight;
  });
  height += element.querySelectorAll('.rely-page-break-row').length * rowBreakHeight(options);
  return height;
}

function rowBreakHeight(options: PaginationOptions): number {
  return options.marginBottom + PAGE_GAP + options.marginTop;
}

/** The lines of a paragraph, measured where they are drawn, without what the page view drew. */
function lineSegments(view: EditorView, offset: number, node: ProseMirrorNode, element: HTMLElement): BlockSegment[] | null {
  if (node.content.size > MAX_MEASURED_POSITIONS) return null;
  const separators = Array.from(element.querySelectorAll('.rely-page-separator')).map((separator) => {
    const box = separator.getBoundingClientRect();
    return { top: box.top, height: box.height };
  });
  const shift = (top: number) =>
    separators.reduce((sum, separator) => (separator.top < top ? sum + separator.height : sum), 0);
  const box = element.getBoundingClientRect();
  const lines: { position: number; top: number }[] = [];
  let lastTop = Number.NEGATIVE_INFINITY;
  for (let position = offset + 1; position < offset + node.nodeSize - 1; position += 1) {
    const coords = view.coordsAtPos(position, 1);
    const top = coords.top - shift(coords.top);
    if (top > lastTop + 1) {
      lines.push({ position, top });
      lastTop = top;
    }
  }
  if (lines.length < 2) return null;
  const blockTop = box.top - (parseFloat(window.getComputedStyle(element).marginTop) || 0);
  const blockBottom = blockTop + outerHeight(element) - shift(Number.POSITIVE_INFINITY);
  return lines.map((line, index) => ({
    position: index === 0 ? offset : line.position,
    height: (index + 1 < lines.length ? lines[index + 1].top : blockBottom) - (index === 0 ? blockTop : line.top),
  }));
}

/** The rows of a table, each a stretch a page can end after. */
function rowSegments(view: EditorView, offset: number, node: ProseMirrorNode, element: HTMLElement, options: PaginationOptions): BlockSegment[] | null {
  const rows: BlockSegment[] = [];
  node.forEach((_row, rowOffset) => {
    const position = offset + 1 + rowOffset;
    const dom = view.nodeDOM(position);
    if (!(dom instanceof HTMLElement)) return;
    const extra = dom.classList.contains('rely-page-break-row') ? rowBreakHeight(options) : 0;
    rows.push({ position, height: dom.offsetHeight - extra });
  });
  if (rows.length < 2) return null;
  const drawn = rows.reduce((sum, row) => sum + row.height, 0);
  rows[0] = { position: offset, height: rows[0].height + Math.max(0, outerHeight(element) - drawnInside(element, options) - drawn) };
  return rows;
}

function separator(page: number, pages: number, options: PaginationOptions, inline: boolean): HTMLElement {
  const tag = inline ? 'span' : 'div';
  const widget = document.createElement(tag);
  widget.className = inline ? 'rely-page-separator rely-page-separator-inline' : 'rely-page-separator';
  widget.contentEditable = 'false';
  const footer = document.createElement(tag);
  footer.className = 'rely-page-footer';
  footer.style.height = `${options.marginBottom}px`;
  footer.textContent = options.footer(page, pages);
  if (options.pageLabel) {
    const number = document.createElement('span');
    number.className = 'rely-page-number';
    number.textContent = options.pageLabel(page);
    footer.append(number);
  }
  const gap = document.createElement(tag);
  gap.className = 'rely-page-gap';
  const header = document.createElement(tag);
  header.className = 'rely-page-header';
  header.style.height = `${options.marginTop}px`;
  header.textContent = options.header(page + 1, pages);
  widget.append(footer, gap, header);
  return widget;
}

/** What the page view draws where a page ends at `position`. */
function pageEnd(state: EditorState, position: number, index: number, pages: number, options: PaginationOptions): Decoration {
  const resolved = state.doc.resolve(position);
  const key = `page-${index}-${pages}-${options.header(index + 2, pages)}-${options.footer(index + 1, pages)}`;
  if (resolved.depth > 0 && resolved.parent.type.spec.tableRole === 'table') {
    const row = resolved.nodeAfter;
    return Decoration.node(position, position + (row?.nodeSize ?? 0), {
      class: 'rely-page-break-row',
      style: `--rely-page-break: ${rowBreakHeight(options)}px`,
    });
  }
  const inline = resolved.depth > 0 && resolved.parent.isTextblock;
  return Decoration.widget(position, () => separator(index + 1, pages, options, inline), {
    side: -1,
    key: `${key}-${inline ? 'line' : 'block'}`,
    ignoreSelection: true,
  });
}

/**
 * The page view: the text drawn on pages of the document's size, a page ending where its text
 * no longer fits, inside a paragraph or a table too, or where a paragraph breaks the page, each
 * with its header and footer.
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
      const element = (offset: number) => {
        const dom = view.nodeDOM(offset);
        return dom instanceof HTMLElement ? dom : null;
      };
      const breaks = options.enabled
        ? pageBreaks(
            view.state.doc,
            (offset) => {
              const dom = element(offset);
              return dom ? outerHeight(dom) - drawnInside(dom, options) : 0;
            },
            options.contentHeight,
            (offset, node) => {
              const dom = element(offset);
              if (!dom) return null;
              if (node.isTextblock) return lineSegments(view, offset, node, dom);
              if (node.type.spec.tableRole === 'table') return rowSegments(view, offset, node, dom, options);
              return null;
            },
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
            const pages = breaks.length + 1;
            return DecorationSet.create(
              state.doc,
              breaks
                .filter((position) => position >= 0 && position <= state.doc.content.size)
                .map((position, index) => pageEnd(state, position, index, pages, storage.options)),
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
