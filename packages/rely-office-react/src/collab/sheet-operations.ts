// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

// The operations of a shared spreadsheet and how one moves past another. A room orders them,
// and every client applies the same ones in the same order to the same file; `transformSheetOperation`
// is what makes an operation made before another apply where its author meant it to.

import type {
  CellRange,
  CellWindow,
  ChartPlacement,
  Condition,
  ConditionalFormat,
  DataValidation,
  PivotTableDefinition,
  ScaleValue,
  SheetRegion,
} from '../model.js';
import { removeFormulaSheet, renameFormulaSheet, shiftFormula } from './formula-references.js';

/** The calls of a `SheetDocument` that change it, and so travel between clients. */
export const SHEET_MUTATORS = [
  'setInput',
  'setInputs',
  'setFormat',
  'setColumnWidth',
  'clearRange',
  'setStyle',
  'setBorders',
  'clearStyles',
  'setDefinedName',
  'removeDefinedName',
  'addChart',
  'setChart',
  'removeChart',
  'setComment',
  'mergeCells',
  'unmergeCells',
  'setRowHeight',
  'copyRange',
  'insertRows',
  'deleteRows',
  'insertColumns',
  'deleteColumns',
  'addSheet',
  'renameSheet',
  'removeSheet',
  'setConditionalFormats',
  'setValidations',
  'fillRange',
  'setFrozen',
  'sortRange',
  'setAutoFilter',
  'setFilterColumn',
  'restoreRegion',
  'startCommentThread',
  'replyToComment',
  'setCommentResolved',
  'removeCommentReply',
  'addPivotTable',
  'setPivotTable',
  'refreshPivotTable',
  'refreshPivotTables',
  'removePivotTable',
] as const;

export type SheetMutator = (typeof SHEET_MUTATORS)[number];

/** One change to a shared workbook: the call, its arguments and the locale its text was typed in. */
export interface SheetOperation {
  method: SheetMutator;
  args: unknown[];
  locale: 'it' | 'en';
  /**
   * The name the sheet of a row or column insertion or deletion, a rename or a removal had when
   * the change was made, so the formulas of the others that name that sheet move with it.
   */
  sheetName?: string;
}

/** The calls whose sheet the formulas of others may name, and so carry its name. */
export const SHEET_NAMED_MUTATORS: ReadonlySet<SheetMutator> = new Set<SheetMutator>([
  'insertRows',
  'deleteRows',
  'insertColumns',
  'deleteColumns',
  'renameSheet',
  'removeSheet',
]);

const MUTATOR_SET: ReadonlySet<string> = new Set(SHEET_MUTATORS);

/** Whether a name is a call that changes a workbook. */
export function isSheetMutator(name: string | symbol): name is SheetMutator {
  return typeof name === 'string' && MUTATOR_SET.has(name);
}

type Axis = 'row' | 'column';

/** A structural change: rows or columns inserted or deleted on a sheet. */
interface Shift {
  sheet: number;
  sheetName?: string;
  axis: Axis;
  at: number;
  count: number;
  inserted: boolean;
}

function structural(operation: SheetOperation): Shift | null {
  const [sheet, at, count = 1] = operation.args as [number, number, number | undefined];
  const named = operation.sheetName === undefined ? {} : { sheetName: operation.sheetName };
  switch (operation.method) {
    case 'insertRows':
      return { sheet, ...named, axis: 'row', at, count, inserted: true };
    case 'deleteRows':
      return { sheet, ...named, axis: 'row', at, count, inserted: false };
    case 'insertColumns':
      return { sheet, ...named, axis: 'column', at, count, inserted: true };
    case 'deleteColumns':
      return { sheet, ...named, axis: 'column', at, count, inserted: false };
    default:
      return null;
  }
}

/** A row or column moved past a shift; null when it was deleted. */
export function shiftIndex(index: number, shift: Pick<Shift, 'at' | 'count' | 'inserted'>): number | null {
  if (shift.inserted) return index >= shift.at ? index + shift.count : index;
  if (index < shift.at) return index;
  if (index < shift.at + shift.count) return null;
  return index - shift.count;
}

/** A span of rows or columns moved past a shift; null when every one of them was deleted. */
export function shiftSpan(
  first: number,
  last: number,
  shift: Pick<Shift, 'at' | 'count' | 'inserted'>,
): [number, number] | null {
  if (shift.inserted) {
    return [first >= shift.at ? first + shift.count : first, last >= shift.at ? last + shift.count : last];
  }
  const end = shift.at + shift.count - 1;
  if (first >= shift.at && last <= end) return null;
  const newFirst = first < shift.at ? first : first > end ? first - shift.count : shift.at;
  const newLast = last < shift.at ? last : last > end ? last - shift.count : shift.at - 1;
  return newLast < newFirst ? null : [newFirst, newLast];
}

function shiftWindow(window: CellWindow, shift: Shift): CellWindow | null {
  if (shift.axis === 'row') {
    const span = shiftSpan(window.top, window.bottom, shift);
    return span ? { ...window, top: span[0], bottom: span[1] } : null;
  }
  const span = shiftSpan(window.left, window.right, shift);
  return span ? { ...window, left: span[0], right: span[1] } : null;
}

function shiftPoint(row: number, column: number, shift: Shift): [number, number] | null {
  if (shift.axis === 'row') {
    const moved = shiftIndex(row, shift);
    return moved === null ? null : [moved, column];
  }
  const moved = shiftIndex(column, shift);
  return moved === null ? null : [row, moved];
}

function shiftRegion(region: SheetRegion, shift: Shift): SheetRegion | null {
  const area = shiftWindow(
    { top: region.area.firstRow, left: region.area.firstColumn, bottom: region.area.lastRow, right: region.area.lastColumn },
    shift,
  );
  if (!area) return null;
  const move = <T extends { row: number; column: number }>(items: T[]): T[] =>
    items.flatMap((item) => {
      const point = shiftPoint(item.row, item.column, shift);
      return point ? [{ ...item, row: point[0], column: point[1] }] : [];
    });
  const merges = region.merges.flatMap((merge: CellRange) => {
    const moved = shiftWindow(
      { top: merge.firstRow, left: merge.firstColumn, bottom: merge.lastRow, right: merge.lastColumn },
      shift,
    );
    return moved ? [{ firstRow: moved.top, firstColumn: moved.left, lastRow: moved.bottom, lastColumn: moved.right }] : [];
  });
  return {
    area: { firstRow: area.top, firstColumn: area.left, lastRow: area.bottom, lastColumn: area.right },
    cells: move(region.cells),
    merges,
    comments: move(region.comments),
  };
}

function shiftRanges(ranges: CellRange[], shift: Shift): CellRange[] {
  return ranges.flatMap((range) => {
    const moved = shiftWindow(
      { top: range.firstRow, left: range.firstColumn, bottom: range.lastRow, right: range.lastColumn },
      shift,
    );
    return moved ? [{ firstRow: moved.top, firstColumn: moved.left, lastRow: moved.bottom, lastColumn: moved.right }] : [];
  });
}

/** Conditional formats or validations moved past a shift; those left with no cells go. */
function shiftRuleRanges<T extends { ranges: CellRange[] }>(items: T[], shift: Shift): T[] {
  return items.flatMap((item) => {
    const ranges = shiftRanges(item.ranges, shift);
    return ranges.length === 0 ? [] : [{ ...item, ranges }];
  });
}

function shiftPlacement(placement: ChartPlacement, shift: Shift): ChartPlacement {
  const point = shiftPoint(placement.row, placement.column, shift);
  if (point) return { ...placement, row: point[0], column: point[1] };
  return { ...placement, row: shift.axis === 'row' ? shift.at : placement.row, column: shift.axis === 'column' ? shift.at : placement.column };
}

/** The operation moved past rows or columns another one inserted or deleted: its cells, then its formulas. */
function pastShift(operation: SheetOperation, shift: Shift): SheetOperation | null {
  const moved = positionsPastShift(operation, shift);
  if (!moved) return null;
  const target = shift.sheetName === undefined ? {} : { sheetName: shift.sheetName };
  return mapFormulas(moved, (formula, home) =>
    shiftFormula(formula, shift, { ...target, ownSheet: home === shift.sheet }),
  );
}

/** The cells, ranges, rows and columns an operation names, moved past a shift on the same sheet. */
function positionsPastShift(operation: SheetOperation, shift: Shift): SheetOperation | null {
  const args = operation.args;
  const sheet = args[0] as number;
  const same = (index: number) => index === shift.sheet;
  const window = (value: unknown) => shiftWindow(value as CellWindow, shift);
  const replace = (index: number, value: unknown): SheetOperation => {
    const next = [...args];
    next[index] = value;
    return { ...operation, args: next };
  };
  switch (operation.method) {
    case 'setInput':
    case 'setFormat':
    case 'setComment':
    case 'startCommentThread':
    case 'replyToComment':
    case 'setCommentResolved':
    case 'removeCommentReply': {
      if (!same(sheet)) return operation;
      const point = shiftPoint(args[1] as number, args[2] as number, shift);
      if (!point) return null;
      const next = [...args];
      next[1] = point[0];
      next[2] = point[1];
      return { ...operation, args: next };
    }
    case 'setInputs': {
      if (!same(sheet)) return operation;
      const inputs = (args[1] as [number, number, string][]).flatMap(([row, column, text]) => {
        const point = shiftPoint(row, column, shift);
        return point ? [[point[0], point[1], text] as [number, number, string]] : [];
      });
      return inputs.length === 0 ? null : replace(1, inputs);
    }
    case 'clearRange':
    case 'setStyle':
    case 'setBorders':
    case 'clearStyles':
    case 'mergeCells':
    case 'unmergeCells':
    case 'sortRange': {
      if (!same(sheet)) return operation;
      const moved = window(args[1]);
      return moved ? replace(1, moved) : null;
    }
    case 'setAutoFilter': {
      if (!same(sheet) || args[1] === null || args[1] === undefined) return operation;
      const moved = window(args[1]);
      return moved ? replace(1, moved) : replace(1, null);
    }
    case 'fillRange': {
      if (!same(sheet)) return operation;
      const source = window(args[1]);
      const target = window(args[2]);
      if (!source || !target) return null;
      return { ...operation, args: [sheet, source, target] };
    }
    case 'copyRange': {
      let next = [...args];
      if (same(sheet)) {
        const moved = window(args[1]);
        if (!moved) return null;
        next[1] = moved;
      }
      if (same(args[2] as number)) {
        const point = shiftPoint(args[3] as number, args[4] as number, shift);
        if (!point) return null;
        next = [next[0], next[1], next[2], point[0], point[1]];
      }
      return { ...operation, args: next };
    }
    case 'setColumnWidth':
    case 'setFilterColumn': {
      if (!same(sheet) || shift.axis !== 'column') return operation;
      const moved = shiftIndex(args[1] as number, shift);
      return moved === null ? null : replace(1, moved);
    }
    case 'setRowHeight': {
      if (!same(sheet) || shift.axis !== 'row') return operation;
      const moved = shiftIndex(args[1] as number, shift);
      return moved === null ? null : replace(1, moved);
    }
    case 'addChart': {
      if (!same(sheet)) return operation;
      return replace(2, shiftPlacement(args[2] as ChartPlacement, shift));
    }
    case 'setChart': {
      const change = (args[2] ?? {}) as { placement?: ChartPlacement };
      if (!same(sheet) || !change.placement) return operation;
      return replace(2, { ...change, placement: shiftPlacement(change.placement, shift) });
    }
    case 'restoreRegion': {
      if (!same(sheet)) return operation;
      const moved = shiftRegion(args[1] as SheetRegion, shift);
      return moved ? replace(1, moved) : null;
    }
    case 'addPivotTable':
    case 'setPivotTable': {
      const position = operation.method === 'addPivotTable' ? 1 : 2;
      const moved = pivotPastShift(args[position] as PivotTableDefinition, sheet, shift);
      return moved ? replace(position, moved) : null;
    }
    case 'setConditionalFormats':
    case 'setValidations': {
      if (!same(sheet)) return operation;
      return replace(1, shiftRuleRanges(args[1] as { ranges: CellRange[] }[], shift));
    }
    case 'insertRows':
    case 'deleteRows':
    case 'insertColumns':
    case 'deleteColumns': {
      const own = structural(operation)!;
      if (!same(sheet) || own.axis !== shift.axis) return operation;
      if (own.inserted) {
        const at = shift.inserted
          ? own.at >= shift.at ? own.at + shift.count : own.at
          : own.at <= shift.at ? own.at : Math.max(shift.at, own.at - shift.count);
        return { ...operation, args: [sheet, at, own.count] };
      }
      if (shift.inserted) {
        if (shift.at >= own.at + own.count) return operation;
        if (shift.at <= own.at) return { ...operation, args: [sheet, own.at + shift.count, own.count] };
        return { ...operation, args: [sheet, own.at, own.count + shift.count] };
      }
      const span = shiftSpan(own.at, own.at + own.count - 1, shift);
      return span ? { ...operation, args: [sheet, span[0], span[1] - span[0] + 1] } : null;
    }
    default:
      return operation;
  }
}

/** A pivot table definition moved past rows or columns: its anchor on its sheet, its source on the source's. */
function pivotPastShift(definition: PivotTableDefinition, sheet: number, shift: Shift): PivotTableDefinition | null {
  let next = definition;
  if (sheet === shift.sheet) {
    const anchor = shiftPoint(definition.anchor.row, definition.anchor.column, shift);
    if (!anchor) return null;
    next = { ...next, anchor: { row: anchor[0], column: anchor[1] } };
  }
  if (definition.sourceSheet === shift.sheet) {
    const source = shiftWindow(definition.source, shift);
    if (!source) return null;
    next = { ...next, source };
  }
  return next;
}

/**
 * The operation moved past the removal of a sheet: later sheets move up, the removed one's
 * operations go, and references to it in formulas become `#REF!`.
 */
function pastRemovedSheet(operation: SheetOperation, removed: number, name: string | undefined): SheetOperation | null {
  const moved = indexesPastRemovedSheet(operation, removed);
  if (!moved || name === undefined) return moved;
  return mapFormulas(moved, (formula) => removeFormulaSheet(formula, name));
}

function indexesPastRemovedSheet(operation: SheetOperation, removed: number): SheetOperation | null {
  const move = (index: number): number | null => (index === removed ? null : index > removed ? index - 1 : index);
  const args = [...operation.args];
  switch (operation.method) {
    case 'addSheet':
    case 'refreshPivotTables':
      return operation;
    case 'addPivotTable':
    case 'setPivotTable': {
      const position = operation.method === 'addPivotTable' ? 1 : 2;
      const definition = args[position] as PivotTableDefinition;
      const sheet = move(args[0] as number);
      const source = move(definition.sourceSheet);
      if (sheet === null || source === null) return null;
      args[0] = sheet;
      args[position] = { ...definition, sourceSheet: source };
      return { ...operation, args };
    }
    case 'setDefinedName':
    case 'removeDefinedName': {
      const position = operation.method === 'setDefinedName' ? 2 : 1;
      const sheet = args[position];
      if (typeof sheet !== 'number') return operation;
      const moved = move(sheet);
      if (moved === null) return null;
      args[position] = moved;
      return { ...operation, args };
    }
    case 'copyRange': {
      const source = move(args[0] as number);
      const target = move(args[2] as number);
      if (source === null || target === null) return null;
      args[0] = source;
      args[2] = target;
      return { ...operation, args };
    }
    default: {
      if (typeof args[0] !== 'number') return operation;
      const moved = move(args[0]);
      if (moved === null) return null;
      args[0] = moved;
      return { ...operation, args };
    }
  }
}

/** The operation moved past a pivot table removed from the same sheet. */
function pastRemovedPivot(operation: SheetOperation, sheet: number, index: number): SheetOperation | null {
  const indexed = ['setPivotTable', 'refreshPivotTable', 'removePivotTable'];
  if (!indexed.includes(operation.method) || operation.args[0] !== sheet) return operation;
  const own = operation.args[1] as number;
  if (own === index) return null;
  if (own < index) return operation;
  const args = [...operation.args];
  args[1] = own - 1;
  return { ...operation, args };
}

/** The operation moved past a chart removed from the same sheet. */
function pastRemovedChart(operation: SheetOperation, sheet: number, index: number): SheetOperation | null {
  if (operation.method !== 'setChart' && operation.method !== 'removeChart') return operation;
  if (operation.args[0] !== sheet) return operation;
  const own = operation.args[1] as number;
  if (own === index) return null;
  if (own < index) return operation;
  const args = [...operation.args];
  args[1] = own - 1;
  return { ...operation, args };
}

/** A cell's input with its references moved, when it is a formula; other text as it is. */
function mapInput(input: string, map: (formula: string) => string): string {
  return input.startsWith('=') ? map(input) : input;
}

function mapScaleValue(value: ScaleValue, map: (formula: string) => string): ScaleValue {
  return value.kind === 'formula' ? { ...value, value: map(value.value) } : value;
}

function mapCondition(condition: Condition, map: (formula: string) => string): Condition {
  switch (condition.kind) {
    case 'cellIs':
      return { ...condition, formulas: condition.formulas.map(map) };
    case 'expression':
      return { ...condition, formula: map(condition.formula) };
    case 'colorScale':
      return { ...condition, points: condition.points.map((point) => ({ ...point, value: mapScaleValue(point.value, map) })) };
    case 'dataBar':
      return { ...condition, min: mapScaleValue(condition.min, map), max: mapScaleValue(condition.max, map) };
    case 'iconSet':
      return { ...condition, thresholds: condition.thresholds.map((threshold) => mapScaleValue(threshold, map)) };
    default:
      return condition;
  }
}

/**
 * The operation with every formula it carries passed through `map`, given the sheet the formula
 * belongs to: cell inputs, a region put back, the rules of conditional formats and validations,
 * and a defined name. The same operation when none changes. Chart ranges are left as they are,
 * as the workbook leaves those of a chart it holds.
 */
function mapFormulas(operation: SheetOperation, map: (formula: string, home: number) => string): SheetOperation {
  const args = operation.args;
  const sheet = args[0] as number;
  const own = (formula: string) => map(formula, sheet);
  let changed: unknown[] | null = null;
  const replace = (index: number, value: unknown) => {
    if (JSON.stringify(value) === JSON.stringify(args[index])) return;
    changed ??= [...args];
    changed[index] = value;
  };
  switch (operation.method) {
    case 'setInput':
      if (typeof args[3] === 'string') replace(3, mapInput(args[3], own));
      break;
    case 'setInputs':
      replace(
        1,
        (args[1] as [number, number, string][]).map(([row, column, text]) => [row, column, mapInput(text, own)]),
      );
      break;
    case 'restoreRegion': {
      const region = args[1] as SheetRegion;
      replace(1, { ...region, cells: region.cells.map((cell) => ({ ...cell, input: mapInput(cell.input, own) })) });
      break;
    }
    case 'setConditionalFormats':
      replace(
        1,
        (args[1] as ConditionalFormat[]).map((format) => ({
          ...format,
          rules: format.rules.map((rule) => ({ ...rule, condition: mapCondition(rule.condition, own) })),
        })),
      );
      break;
    case 'setValidations':
      replace(
        1,
        (args[1] as DataValidation[]).map((validation) =>
          validation.formulas ? { ...validation, formulas: validation.formulas.map(own) } : validation,
        ),
      );
      break;
    case 'setDefinedName': {
      // A name without a sheet writes its unqualified references on the first sheet.
      const home = typeof args[2] === 'number' ? args[2] : 0;
      if (typeof args[1] === 'string') replace(1, map(args[1], home));
      break;
    }
    default:
      break;
  }
  return changed ? { ...operation, args: changed } : operation;
}

/**
 * An operation moved past one the room ordered before it: rows, columns, sheets and charts
 * shifted by insertions and removals, the references of its formulas moved and renamed the way
 * the workbook moves those it holds, and null when what it changes no longer exists. Every
 * other pair is left as it is, the later one winning where both change the same cell.
 */
export function transformSheetOperation(operation: SheetOperation, earlier: SheetOperation): SheetOperation | null {
  const shift = structural(earlier);
  if (shift) return pastShift(operation, shift);
  if (earlier.method === 'removeSheet') {
    if (operation.method === 'removeSheet' && operation.args[0] === earlier.args[0]) return null;
    return pastRemovedSheet(operation, earlier.args[0] as number, earlier.sheetName);
  }
  if (earlier.method === 'renameSheet' && earlier.sheetName !== undefined) {
    const from = earlier.sheetName;
    const to = earlier.args[1] as string;
    return mapFormulas(operation, (formula) => renameFormulaSheet(formula, from, to));
  }
  if (earlier.method === 'removeChart') {
    return pastRemovedChart(operation, earlier.args[0] as number, earlier.args[1] as number);
  }
  if (earlier.method === 'removePivotTable') {
    return pastRemovedPivot(operation, earlier.args[0] as number, earlier.args[1] as number);
  }
  return operation;
}
