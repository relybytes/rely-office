// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import {
  GridAxis,
  cellName,
  columnName,
  columnWidthCharacters,
  columnWidthPixels,
  coverMerges,
  fillTarget,
  parseCellName,
  parseTabSeparated,
  rowHeightPixels,
  rowHeightPoints,
  toTabSeparated,
  windowBetween,
} from './geometry.js';

describe('cell names', () => {
  test.each([
    [0, 'A'],
    [25, 'Z'],
    [26, 'AA'],
    [51, 'AZ'],
    [701, 'ZZ'],
    [702, 'AAA'],
  ])('column %i is %s, and back', (column, name) => {
    expect(columnName(column)).toBe(name);
    expect(parseCellName(`${name}1`)).toEqual({ row: 0, column });
  });

  test('a cell is named by its column and its row from 1', () => {
    expect(cellName({ row: 2, column: 1 })).toBe('B3');
    expect(parseCellName(' b3 ')).toEqual({ row: 2, column: 1 });
  });

  test.each(['', 'A0', '3B', 'A1:B2', 'ABCD1'])('%j names no cell', (name) => {
    expect(parseCellName(name)).toBeNull();
  });
});

describe('windows', () => {
  test('two corners span the same window in either order', () => {
    expect(windowBetween({ row: 4, column: 1 }, { row: 1, column: 3 })).toEqual({ top: 1, bottom: 4, left: 1, right: 3 });
  });

  test('a window grows to the merges it touches, and to the ones those touch', () => {
    const merges = [
      { firstRow: 0, lastRow: 1, firstColumn: 1, lastColumn: 2 },
      { firstRow: 1, lastRow: 3, firstColumn: 2, lastColumn: 4 },
      { firstRow: 9, lastRow: 9, firstColumn: 9, lastColumn: 9 },
    ];
    expect(coverMerges({ top: 0, bottom: 0, left: 0, right: 1 }, merges)).toEqual({ top: 0, bottom: 3, left: 0, right: 4 });
  });
});

describe('sizes', () => {
  test('Excel units become pixels', () => {
    expect(columnWidthPixels(8.43)).toBe(64);
    expect(rowHeightPixels(15)).toBe(20);
  });

  test('an axis sums its default and its own sizes', () => {
    const axis = new GridAxis(20, [
      [2, 40],
      [5, 0],
      [7, 20],
    ]);
    expect(axis.size(2)).toBe(40);
    expect(axis.size(3)).toBe(20);
    expect(axis.offset(0)).toBe(0);
    expect(axis.offset(3)).toBe(80);
    expect(axis.offset(6)).toBe(120);
    expect(axis.indexAt(0)).toBe(0);
    expect(axis.indexAt(79)).toBe(2);
    expect(axis.indexAt(80)).toBe(3);
    expect(axis.indexAt(100)).toBe(4);
    expect(axis.indexAt(119)).toBe(4);
    expect(axis.indexAt(120)).toBe(6);
    expect(axis.indexAt(5000)).toBe(250);
  });
});

describe('the clipboard', () => {
  test('cells with tabs, quotes and lines are quoted, and read back', () => {
    const rows = [
      ['a', 'b\tc'],
      ['say "hi"', 'two\nlines'],
    ];
    const text = toTabSeparated(rows);
    expect(text).toBe('a\t"b\tc"\n"say ""hi"""\t"two\nlines"');
    expect(parseTabSeparated(text)).toEqual(rows);
  });

  test('a trailing line end and Windows line ends are what spreadsheets copy', () => {
    expect(parseTabSeparated('1\t2\r\n3\t4\r\n')).toEqual([
      ['1', '2'],
      ['3', '4'],
    ]);
  });
});

describe('fillTarget', () => {
  const source = { top: 2, left: 1, bottom: 3, right: 2 };

  it('grows the source towards the pointer along the side it went furthest past', () => {
    expect(fillTarget(source, { row: 9, column: 3 })).toEqual({ top: 2, left: 1, bottom: 9, right: 2 });
    expect(fillTarget(source, { row: 0, column: 2 })).toEqual({ top: 0, left: 1, bottom: 3, right: 2 });
    expect(fillTarget(source, { row: 4, column: 7 })).toEqual({ top: 2, left: 1, bottom: 3, right: 7 });
    expect(fillTarget(source, { row: 3, column: 0 })).toEqual({ top: 2, left: 0, bottom: 3, right: 2 });
  });

  it('keeps the source while the pointer stays inside it', () => {
    expect(fillTarget(source, { row: 3, column: 1 })).toBe(source);
  });
});

describe('sizes back to file units', () => {
  it('inverts the pixel conversions', () => {
    expect(columnWidthCharacters(columnWidthPixels(10))).toBe(10);
    expect(rowHeightPoints(rowHeightPixels(15))).toBe(15);
    expect(columnWidthCharacters(2)).toBe(0);
  });
});
