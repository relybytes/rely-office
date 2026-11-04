// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-RelyBytes-Attribution

import { Extension } from '@tiptap/core';
import { TableView } from '@tiptap/extension-table';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import type { BorderLine, CellBorders, CellMargins, TableBorders, TableCell } from '../model.js';

/** A colour of the tree, `#RRGGBB`; the `#` may be missing on the way in. */
const HEX_COLOUR = /^#?[0-9a-f]{6}$/i;
const LINE_STYLES: Record<BorderLine['style'], string> = {
  none: 'none',
  single: 'solid',
  double: 'double',
  dotted: 'dotted',
  dashed: 'dashed',
};
const TABLE_SIDES = ['top', 'left', 'bottom', 'right', 'insideH', 'insideV'] as const;
const CELL_SIDES = ['top', 'left', 'bottom', 'right'] as const;
/** In the order of the CSS `padding` shorthand. */
const PADDING_SIDES = ['top', 'right', 'bottom', 'left'] as const;
const VERTICAL_ALIGNS: ReadonlySet<string> = new Set(['top', 'center', 'bottom']);
/** Word's cell margins for a side a table leaves out: none above and below, 5.4 points beside. */
const WORD_CELL_MARGINS_TWIPS: Required<CellMargins> = { top: 0, left: 108, bottom: 0, right: 108 };
/** Word's own page limit, 22 inches. */
const MAX_TWIPS = 31_680;

/** A colour as the tree carries it, `#RRGGBB` in upper case; null for anything else. */
export function cleanColor(value: unknown): string | null {
  if (typeof value !== 'string' || !HEX_COLOUR.test(value.trim())) return null;
  return `#${value.trim().replace('#', '').toUpperCase()}`;
}

/** A border line as the tree takes it; null when `value` is not one. */
export function cleanBorderLine(value: unknown): BorderLine | null {
  if (value === null || typeof value !== 'object') return null;
  const line = value as Record<string, unknown>;
  if (typeof line.style !== 'string' || !Object.hasOwn(LINE_STYLES, line.style)) return null;
  const width = line.widthEighths;
  const color = cleanColor(line.color);
  return {
    style: line.style as BorderLine['style'],
    ...(typeof width === 'number' && Number.isFinite(width) ? { widthEighths: Math.round(width) } : {}),
    ...(color === null ? {} : { color }),
  };
}

function cleanEdges<Side extends string>(value: unknown, sides: readonly Side[]): Partial<Record<Side, BorderLine>> | null {
  if (value === null || typeof value !== 'object') return null;
  const edges: Partial<Record<Side, BorderLine>> = {};
  for (const side of sides) {
    const line = cleanBorderLine((value as Record<string, unknown>)[side]);
    if (line !== null) edges[side] = line;
  }
  return Object.keys(edges).length > 0 ? edges : null;
}

/** The lines of a table as the tree takes them; null when it names none. */
export function cleanTableBorders(value: unknown): TableBorders | null {
  return cleanEdges(value, TABLE_SIDES);
}

/** The edges of a cell as the tree takes them; null when it names none. */
export function cleanCellBorders(value: unknown): CellBorders | null {
  return cleanEdges(value, CELL_SIDES);
}

/** The cell margins of a table in twips; null when it names none. */
export function cleanCellMargins(value: unknown): CellMargins | null {
  if (value === null || typeof value !== 'object') return null;
  const margins: CellMargins = {};
  for (const side of CELL_SIDES) {
    const twips = (value as Record<string, unknown>)[side];
    if (typeof twips === 'number' && Number.isFinite(twips)) {
      margins[side] = Math.min(Math.max(Math.round(twips), 0), MAX_TWIPS);
    }
  }
  return Object.keys(margins).length > 0 ? margins : null;
}

/** What a cell of the editor gives the tree beside its spans and blocks. */
export function cellLooks(attrs: Record<string, unknown> | undefined): Pick<TableCell, 'fill' | 'borders' | 'verticalAlign'> {
  const fill = cleanColor(attrs?.fill);
  const borders = cleanCellBorders(attrs?.borders);
  const vertical = attrs?.verticalAlign;
  return {
    ...(fill === null ? {} : { fill }),
    ...(borders === null ? {} : { borders }),
    ...(typeof vertical === 'string' && VERTICAL_ALIGNS.has(vertical)
      ? { verticalAlign: vertical as NonNullable<TableCell['verticalAlign']> }
      : {}),
  };
}

/**
 * A border line as a CSS border, `1.5pt solid #C00000`; an automatic colour follows the text. A
 * double rule is drawn three times as wide, the room its two lines and their gap take.
 */
export function borderCss(value: unknown): string | null {
  const line = cleanBorderLine(value);
  if (line === null) return null;
  if (line.style === 'none') return 'none';
  const eighths = Math.min(Math.max(line.widthEighths ?? 4, 2), 96);
  const points = (line.style === 'double' ? eighths * 3 : eighths) / 8;
  return `${Math.round(points * 100) / 100}pt ${LINE_STYLES[line.style]} ${line.color ?? 'currentColor'}`;
}

function twipsCss(twips: number): string {
  return `${Math.round((twips / 20) * 100) / 100}pt`;
}

/** The custom properties a table's cells draw its lines with, each side `none` when unnamed. */
function lineProperties(borders: TableBorders | null): [string, string | null][] {
  return TABLE_SIDES.map((side) => [
    `--rely-table-${side.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`,
    borders === null ? null : (borderCss(borders[side]) ?? 'none'),
  ]);
}

/** The padding of a table's cells as its margins give it; null when it names none. */
function paddingProperty(margins: CellMargins | null): string | null {
  if (margins === null) return null;
  return PADDING_SIDES.map((side) => twipsCss(margins[side] ?? WORD_CELL_MARGINS_TWIPS[side])).join(' ');
}

function styleOf(properties: [string, string | null][]): Record<string, string> {
  const set = properties.filter((entry): entry is [string, string] => entry[1] !== null);
  return set.length > 0 ? { style: set.map(([name, value]) => `${name}: ${value}`).join('; ') } : {};
}

function jsonAttribute(element: HTMLElement, name: string): unknown {
  const text = element.getAttribute(name);
  if (!text) return null;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return null;
  }
}

function applyTableLooks(table: HTMLTableElement, node: ProseMirrorNode): void {
  const borders = cleanTableBorders(node.attrs.borders);
  table.classList.toggle('rely-table-lines', borders !== null);
  const properties = [
    ...lineProperties(borders),
    ['--rely-cell-padding', paddingProperty(cleanCellMargins(node.attrs.cellMarginsTwips))] as [string, string | null],
  ];
  for (const [name, value] of properties) {
    if (value === null) table.style.removeProperty(name);
    else table.style.setProperty(name, value);
  }
}

/**
 * The table node view, drawing the lines and cell margins the document gives the table: the
 * view TipTap builds for resizing receives no HTML attributes, so it sets them itself.
 */
export class RelyTableView extends TableView {
  constructor(...args: ConstructorParameters<typeof TableView>) {
    super(...args);
    applyTableLooks(this.table, args[0]);
  }

  override update(node: ProseMirrorNode): boolean {
    if (!super.update(node)) return false;
    applyTableLooks(this.table, node);
    return true;
  }
}

/**
 * The looks a table and its cells bring from a document, kept in the editor so a save writes them
 * back: the table's `borders` and `cellMarginsTwips`, each cell's `fill`, `borders` and
 * `verticalAlign`. A table, a row or a column made in the editor has none of its own.
 */
export const RelyTableLooks = Extension.create({
  name: 'relyTableLooks',
  addGlobalAttributes() {
    return [
      {
        types: ['table'],
        attributes: {
          borders: {
            default: null,
            parseHTML: (element) => cleanTableBorders(jsonAttribute(element, 'data-borders')),
            renderHTML: (attributes) => {
              const borders = cleanTableBorders(attributes.borders);
              if (borders === null) return {};
              return {
                class: 'rely-table-lines',
                'data-borders': JSON.stringify(borders),
                ...styleOf(lineProperties(borders)),
              };
            },
          },
          cellMarginsTwips: {
            default: null,
            parseHTML: (element) => cleanCellMargins(jsonAttribute(element, 'data-cell-margins')),
            renderHTML: (attributes) => {
              const margins = cleanCellMargins(attributes.cellMarginsTwips);
              if (margins === null) return {};
              return {
                'data-cell-margins': JSON.stringify(margins),
                ...styleOf([['--rely-cell-padding', paddingProperty(margins)]]),
              };
            },
          },
        },
      },
      {
        types: ['tableCell', 'tableHeader'],
        attributes: {
          fill: {
            default: null,
            parseHTML: (element) => cleanColor(element.getAttribute('data-fill')),
            renderHTML: (attributes) => {
              const fill = cleanColor(attributes.fill);
              return fill === null ? {} : { 'data-fill': fill, style: `background-color: ${fill}` };
            },
          },
          borders: {
            default: null,
            parseHTML: (element) => cleanCellBorders(jsonAttribute(element, 'data-borders')),
            renderHTML: (attributes) => {
              const borders = cleanCellBorders(attributes.borders);
              if (borders === null) return {};
              return {
                'data-borders': JSON.stringify(borders),
                ...styleOf(CELL_SIDES.map((side) => [`border-${side}`, borderCss(borders[side])])),
              };
            },
          },
          verticalAlign: {
            default: null,
            parseHTML: (element) => {
              const value = element.getAttribute('data-vertical-align');
              return value !== null && VERTICAL_ALIGNS.has(value) ? value : null;
            },
            renderHTML: (attributes) => {
              const vertical = attributes.verticalAlign;
              if (typeof vertical !== 'string' || !VERTICAL_ALIGNS.has(vertical)) return {};
              return {
                'data-vertical-align': vertical,
                style: `vertical-align: ${vertical === 'center' ? 'middle' : vertical}`,
              };
            },
          },
        },
      },
    ];
  },
});
