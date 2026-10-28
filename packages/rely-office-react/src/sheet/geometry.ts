// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type { CellRange, CellWindow } from '../model.js';

/** A cell of the grid, zero-based. */
export interface CellPosition {
  row: number;
  column: number;
}

/** The letters of a column: 0 is A, 25 is Z, 26 is AA. */
export function columnName(column: number): string {
  let name = '';
  let rest = column + 1;
  while (rest > 0) {
    const digit = (rest - 1) % 26;
    name = String.fromCharCode(65 + digit) + name;
    rest = Math.floor((rest - 1) / 26);
  }
  return name;
}

/** A cell's name as a formula writes it: B3 for row 2, column 1. */
export function cellName({ row, column }: CellPosition): string {
  return `${columnName(column)}${row + 1}`;
}

/** The cell a name like `B3` or `aa10` names, or null. */
export function parseCellName(name: string): CellPosition | null {
  const match = /^\s*([A-Za-z]{1,3})(\d{1,7})\s*$/.exec(name);
  if (!match) return null;
  const column = [...match[1].toUpperCase()].reduce((value, letter) => value * 26 + letter.charCodeAt(0) - 64, 0) - 1;
  const row = Number(match[2]) - 1;
  return row < 0 ? null : { row, column };
}

/** The window two corners span, whatever order they are in. */
export function windowBetween(anchor: CellPosition, focus: CellPosition): CellWindow {
  return {
    top: Math.min(anchor.row, focus.row),
    bottom: Math.max(anchor.row, focus.row),
    left: Math.min(anchor.column, focus.column),
    right: Math.max(anchor.column, focus.column),
  };
}

/** A window grown to cover every merge it touches, as a spreadsheet selects. */
export function coverMerges(window: CellWindow, merges: CellRange[]): CellWindow {
  let current = { ...window };
  let grown = true;
  while (grown) {
    grown = false;
    for (const merge of merges) {
      const touches =
        merge.firstRow <= current.bottom &&
        merge.lastRow >= current.top &&
        merge.firstColumn <= current.right &&
        merge.lastColumn >= current.left;
      if (!touches) continue;
      const next = {
        top: Math.min(current.top, merge.firstRow),
        bottom: Math.max(current.bottom, merge.lastRow),
        left: Math.min(current.left, merge.firstColumn),
        right: Math.max(current.right, merge.lastColumn),
      };
      if (next.top !== current.top || next.bottom !== current.bottom || next.left !== current.left || next.right !== current.right) {
        current = next;
        grown = true;
      }
    }
  }
  return current;
}

/** Excel's column width in characters as pixels, for its default font. */
export function columnWidthPixels(characters: number): number {
  return Math.round(characters * 7 + 5);
}

/** A row height in points as pixels. */
export function rowHeightPixels(points: number): number {
  return Math.round((points * 96) / 72);
}

/**
 * One axis of the grid: rows or columns of a default size, some of a size of their own.
 * Offsets are sums of sizes from the first; the overrides are few, so they are summed directly.
 */
export class GridAxis {
  private readonly overrides: [number, number][];

  constructor(
    readonly defaultSize: number,
    sizes: Iterable<[number, number]>,
  ) {
    this.overrides = [...sizes].filter(([, size]) => size !== defaultSize).sort((a, b) => a[0] - b[0]);
  }

  size(index: number): number {
    const override = this.overrides.find(([at]) => at === index);
    return override === undefined ? this.defaultSize : override[1];
  }

  /** Where an index starts. */
  offset(index: number): number {
    let offset = index * this.defaultSize;
    for (const [at, size] of this.overrides) {
      if (at >= index) break;
      offset += size - this.defaultSize;
    }
    return offset;
  }

  /** The index at an offset; past the end, the index the offset would reach. */
  indexAt(offset: number): number {
    let low = 0;
    let high = Math.max(1, Math.ceil(offset / Math.max(1, this.minimumSize())) + 1);
    while (this.offset(high) <= offset) high *= 2;
    while (low < high) {
      const middle = Math.floor((low + high + 1) / 2);
      if (this.offset(middle) <= offset) low = middle;
      else high = middle - 1;
    }
    return low;
  }

  private minimumSize(): number {
    return this.overrides.reduce((smallest, [, size]) => (size > 0 ? Math.min(smallest, size) : smallest), this.defaultSize);
  }
}

/** Cells as tab separated text, as spreadsheets put them on the clipboard. */
export function toTabSeparated(rows: string[][]): string {
  const quote = (value: string): string =>
    /[\t\n"]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value;
  return rows.map((row) => row.map(quote).join('\t')).join('\n');
}

/** Tab separated text from the clipboard as rows of cells, quoted fields included. */
export function parseTabSeparated(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = '';
  let quoted = false;
  const normalised = text.replace(/\r\n?/g, '\n').replace(/\n$/, '');
  for (let index = 0; index < normalised.length; index += 1) {
    const character = normalised[index];
    if (quoted) {
      if (character === '"' && normalised[index + 1] === '"') {
        field += '"';
        index += 1;
      } else if (character === '"') {
        quoted = false;
      } else {
        field += character;
      }
    } else if (character === '"' && field.length === 0) {
      quoted = true;
    } else if (character === '\t') {
      row.push(field);
      field = '';
    } else if (character === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
    } else {
      field += character;
    }
  }
  row.push(field);
  rows.push(row);
  return rows;
}
