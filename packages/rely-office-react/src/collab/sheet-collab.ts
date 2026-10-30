// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import type { CellRange, CellWindow, ChartDefinition, ConditionalFormat, PivotTableDefinition, SheetDocument } from '../model.js';
import { shiftFormula, type ReferenceShift } from './formula-references.js';
import type { SequencedOperation, SubmittedOperation } from './protocol.js';
import type { CollabSession } from './session.js';
import {
  SHEET_NAMED_MUTATORS,
  isSheetMutator,
  shiftSpan,
  transformSheetOperation,
  type RestoredBand,
  type SheetMutator,
  type SheetOperation,
} from './sheet-operations.js';

/** The most edits a collaborative grid can undo. */
export const MAX_SHARED_UNDO = 100;

/** The most cells an edit's undo keeps; a larger edit cannot be undone. */
const MAX_UNDO_AREA = 200_000;

/** The most cells of a sheet read for the formulas a deletion breaks; a larger sheet is not read. */
const MAX_SCANNED_AREA = 5_000_000;

/** The most cells the workbook hands back in one read. */
const MAX_WINDOW_CELLS = 250_000;

interface PendingOperation extends SubmittedOperation<SheetOperation> {
  operation: SheetOperation;
}

/** How to put back what an edit changed, made before the edit from the workbook as it was. */
interface UndoStep {
  operations: SheetOperation[];
  /** The last operation of the room the step was made against. */
  base: number;
}

/** Where another client's selection is, for the grid to draw. */
export interface RemoteSelection {
  client: number;
  name: string;
  color: string;
  sheet: number;
  window: CellWindow;
}

/** The selection a grid shares with the others. */
export interface SharedSelection {
  sheet: number;
  window: CellWindow;
}

/** The formulas of a conditional format's condition. */
function conditionFormulas(condition: ConditionalFormat['rules'][number]['condition']): string[] {
  const scale = (value: { kind: string; value?: string }) => (value.kind === 'formula' && typeof value.value === 'string' ? [value.value] : []);
  switch (condition.kind) {
    case 'cellIs':
      return condition.formulas;
    case 'expression':
      return [condition.formula];
    case 'colorScale':
      return condition.points.flatMap((point) => scale(point.value));
    case 'dataBar':
      return [...scale(condition.min), ...scale(condition.max)];
    case 'iconSet':
      return condition.thresholds.flatMap(scale);
    default:
      return [];
  }
}

/** The ranges of a chart, as formulas. */
function chartFormulas(chart: ChartDefinition): string[] {
  return [
    ...(typeof chart.categories === 'string' ? [chart.categories] : []),
    ...chart.series.flatMap((series) => [series.values, ...(typeof series.name === 'string' ? [series.name] : [])]),
  ];
}

export interface SheetCollaborationOptions {
  /** Called when the workbook can no longer follow the room and the file must be opened again. */
  onReload?: () => void;
}

function windowOf(cells: [number, number][]): CellWindow | null {
  if (cells.length === 0) return null;
  let top = Infinity;
  let left = Infinity;
  let bottom = -1;
  let right = -1;
  for (const [row, column] of cells) {
    top = Math.min(top, row);
    left = Math.min(left, column);
    bottom = Math.max(bottom, row);
    right = Math.max(right, column);
  }
  return { top, left, bottom, right };
}

function areaOf(window: CellWindow): number {
  return (window.bottom - window.top + 1) * (window.right - window.left + 1);
}

function grown(window: CellWindow): CellWindow {
  return {
    top: Math.max(0, window.top - 1),
    left: Math.max(0, window.left - 1),
    bottom: window.bottom + 1,
    right: window.right + 1,
  };
}

/**
 * A workbook edited by several people at once. Every edit is applied at once and sent to the
 * room, which orders it; edits of the others are applied as the room orders them, and an edit of
 * this client still waiting for its turn is put back on top of them. Undo and redo take back this
 * client's own edits only.
 *
 * The grid is given `document` instead of the workbook: the same calls, with the edits shared.
 */
export class SheetCollaboration {
  /** The workbook as the grid should use it. */
  readonly document: SheetDocument;
  private readonly sheet: SheetDocument;
  private readonly session: CollabSession;
  private readonly options: SheetCollaborationOptions;
  private confirmed: number;
  private pending: PendingOperation[] = [];
  private log: SequencedOperation<SheetOperation>[] = [];
  private undoSteps: UndoStep[] = [];
  private redoSteps: UndoStep[] = [];
  private counter = 0;
  private readonly listeners = new Set<() => void>();
  private readonly idleWaiters: (() => void)[] = [];
  private readonly unsubscribe: (() => void)[] = [];

  /**
   * `sheet` must be the base version of the room's welcome, freshly opened: the operations the
   * room has after it are applied here as they arrive.
   */
  constructor(session: CollabSession, sheet: SheetDocument, options: SheetCollaborationOptions = {}) {
    this.session = session;
    this.sheet = sheet;
    this.options = options;
    this.confirmed = session.welcome?.baseSequence ?? -1;
    session.lastSequence = this.confirmed;
    sheet.setHistoryEnabled(false);
    this.document = new Proxy(sheet, {
      get: (target, property, receiver) => {
        if (property === 'undo') return () => this.undo();
        if (property === 'redo') return () => this.redo();
        if (property === 'canUndo') return () => this.canUndo();
        if (property === 'canRedo') return () => this.canRedo();
        if (isSheetMutator(property)) {
          return (...args: unknown[]) => this.local(property, args);
        }
        const value = Reflect.get(target, property, receiver) as unknown;
        return typeof value === 'function' ? (value as (...args: unknown[]) => unknown).bind(target) : value;
      },
    });
    this.unsubscribe.push(
      session.on('operation', (operation) => this.remote(operation as SequencedOperation<SheetOperation>)),
      session.on('ready', () => this.resend()),
      session.on('welcome', (welcome) => {
        if (welcome.reload) this.options.onReload?.();
      }),
      session.on('control', (message) => {
        if (message.type === 'resync') this.options.onReload?.();
        if (message.type === 'saved') this.notify();
      }),
    );
  }

  /** The last operation of the room the workbook holds. */
  get sequence(): number {
    return this.confirmed;
  }

  /** How many edits of this client wait for their turn. */
  get waiting(): number {
    return this.pending.length;
  }

  /** Called after an edit of another client changed the workbook. */
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /**
   * Resolves once every edit of this client has its turn, so a save writes what the room has
   * and nothing it does not; rejects after `timeout` milliseconds.
   */
  whenIdle(timeout = 10_000): Promise<void> {
    if (this.pending.length === 0) return Promise.resolve();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        const index = this.idleWaiters.indexOf(done);
        if (index >= 0) this.idleWaiters.splice(index, 1);
        reject(new Error('COLLAB_TIMEOUT'));
      }, timeout);
      const done = () => {
        clearTimeout(timer);
        resolve();
      };
      this.idleWaiters.push(done);
    });
  }

  /** Share the selection of this client, for the others' grids. */
  setSelection(selection: SharedSelection | null): void {
    this.session.awareness.setLocalStateField('selection', selection);
  }

  /** The selections of the others. */
  remoteSelections(): RemoteSelection[] {
    const selections: RemoteSelection[] = [];
    for (const [client, state] of this.session.awareness.getStates()) {
      if (client === this.session.awareness.clientID) continue;
      const { user, selection } = state as { user?: { name: string; color: string }; selection?: SharedSelection | null };
      if (!user || !selection) continue;
      selections.push({ client, name: user.name, color: user.color, sheet: selection.sheet, window: selection.window });
    }
    return selections;
  }

  /** Called when the others' presence changes. */
  onPresence(listener: () => void): () => void {
    const handler = () => listener();
    this.session.awareness.on('change', handler);
    return () => this.session.awareness.off('change', handler);
  }

  canUndo(): boolean {
    return this.undoSteps.length > 0;
  }

  canRedo(): boolean {
    return this.redoSteps.length > 0;
  }

  /** Take back this client's last edit; false when there is none. */
  undo(): boolean {
    return this.replay(this.undoSteps, 'undo');
  }

  /** Put back the edit the last undo took away; false when there is none. */
  redo(): boolean {
    return this.replay(this.redoSteps, 'redo');
  }

  /** Stop following the room; the workbook keeps its undo history off. */
  destroy(): void {
    for (const stop of this.unsubscribe.splice(0)) stop();
    this.listeners.clear();
  }

  private replay(steps: UndoStep[], kind: 'undo' | 'redo'): boolean {
    const step = steps.pop();
    if (!step) return false;
    const inverse: SheetOperation[] = [];
    for (const operation of step.operations) {
      const moved = this.movedSince(operation, step.base, this.session.sessionKey);
      if (!moved) continue;
      // What an undo put back after the rows it inserted again goes with them: redoing deletes them.
      const undo = moved.follows ? [] : this.inverseOf(moved);
      const result = this.submit(operation, step.base);
      inverse.unshift(...undo, ...this.creationInverse(moved, result));
    }
    if (inverse.length > 0) {
      (kind === 'undo' ? this.redoSteps : this.undoSteps).push({ operations: inverse, base: this.confirmed });
    }
    this.notify();
    return true;
  }

  /** The removal of the sheet or chart an operation added, which only its result names. */
  private creationInverse(operation: SheetOperation, result: unknown): SheetOperation[] {
    if (typeof result !== 'number') return [];
    if (operation.method === 'addSheet') return [{ method: 'removeSheet', args: [result], locale: operation.locale }];
    if (operation.method === 'addChart') {
      return [{ method: 'removeChart', args: [operation.args[0], result], locale: operation.locale }];
    }
    if (operation.method === 'addPivotTable') {
      return [{ method: 'removePivotTable', args: [operation.args[0], result], locale: operation.locale }];
    }
    return [];
  }

  private nextId(): string {
    this.counter += 1;
    return `${this.counter}`;
  }

  /** An edit of this client: applied now, its undo kept, sent to the room. */
  private local(method: SheetMutator, args: unknown[]): unknown {
    const operation: SheetOperation = { method, args: structuredClone(args), locale: this.sheet.locale() };
    const inverse = this.inverseOf(operation);
    const result = this.submit(operation, this.confirmed);
    inverse.push(...this.creationInverse(operation, result));
    if (method === 'removeSheet') {
      this.undoSteps = [];
      this.redoSteps = [];
    } else if (inverse.length > 0) {
      this.undoSteps.push({ operations: inverse, base: this.confirmed });
      this.redoSteps = [];
      if (this.undoSteps.length > MAX_SHARED_UNDO) this.undoSteps.shift();
    }
    return result;
  }

  /**
   * Apply an operation made against `base` and send it: moved past what the others did since,
   * as the room will move it, and kept until the room gives it its turn.
   */
  private submit(made: SheetOperation, base: number): unknown {
    const found = base < this.confirmed ? this.movedSince(made, base, this.session.sessionKey) : made;
    if (!found) return undefined;
    // The others' formulas name the sheet, so a change to its rows, its name or its place carries it.
    const sheetName = SHEET_NAMED_MUTATORS.has(found.method)
      ? this.sheet.sheets()[found.args[0] as number]?.name
      : undefined;
    const operation = sheetName === undefined ? made : { ...made, sheetName };
    const moved = sheetName === undefined ? found : { ...found, sheetName };
    if (this.pending.length === 0) this.sheet.checkpoint();
    let result: unknown;
    try {
      result = this.apply(moved, true);
    } catch (error) {
      if (this.pending.length === 0) this.sheet.discardCheckpoint();
      throw error;
    }
    const submitted: PendingOperation = { id: this.nextId(), base, operation };
    this.pending.push(submitted);
    this.session.sendOperation(submitted);
    return result;
  }

  /** The calls that put back what an operation will change, made from the workbook before it. */
  private inverseOf(operation: SheetOperation): SheetOperation[] {
    const { method, args, locale } = operation;
    const sheet = args[0] as number;
    const restore = (index: number, window: CellWindow | null): SheetOperation[] => {
      if (!window || areaOf(window) > MAX_UNDO_AREA) return [];
      try {
        return [{ method: 'restoreRegion', args: [index, this.sheet.region(index, window)], locale }];
      } catch {
        return [];
      }
    };
    const summary = () => this.sheet.sheets()[sheet];
    switch (method) {
      case 'setInput':
      case 'setFormat':
      case 'setComment':
      case 'startCommentThread':
      case 'replyToComment':
      case 'setCommentResolved':
      case 'removeCommentReply':
        return restore(sheet, { top: args[1] as number, left: args[2] as number, bottom: args[1] as number, right: args[2] as number });
      case 'setInputs':
        return restore(sheet, windowOf((args[1] as [number, number, string][]).map(([row, column]) => [row, column])));
      case 'clearRange':
      case 'setStyle':
      case 'clearStyles':
      case 'mergeCells':
      case 'unmergeCells':
      case 'sortRange':
      case 'restoreRegion':
        return method === 'restoreRegion'
          ? restore(sheet, (() => {
              const area = (args[1] as { area: { firstRow: number; firstColumn: number; lastRow: number; lastColumn: number } }).area;
              return { top: area.firstRow, left: area.firstColumn, bottom: area.lastRow, right: area.lastColumn };
            })())
          : restore(sheet, args[1] as CellWindow);
      case 'setBorders':
        return restore(sheet, grown(args[1] as CellWindow));
      case 'fillRange':
        return restore(sheet, args[2] as CellWindow);
      case 'copyRange': {
        const source = args[1] as CellWindow;
        const row = args[3] as number;
        const column = args[4] as number;
        return restore(args[2] as number, {
          top: row,
          left: column,
          bottom: row + source.bottom - source.top,
          right: column + source.right - source.left,
        });
      }
      case 'insertRows':
        return [{ method: 'deleteRows', args: [sheet, args[1], args[2] ?? 1], locale }];
      case 'insertColumns':
        return [{ method: 'deleteColumns', args: [sheet, args[1], args[2] ?? 1], locale }];
      case 'deleteRows':
      case 'deleteColumns': {
        const at = args[1] as number;
        const count = (args[2] as number | undefined) ?? 1;
        const extent = summary();
        const rows = method === 'deleteRows';
        const band: CellWindow = rows
          ? { top: at, left: 0, bottom: at + count - 1, right: Math.max(0, (extent?.columns ?? 1) - 1) }
          : { top: 0, left: at, bottom: Math.max(0, (extent?.rows ?? 1) - 1), right: at + count - 1 };
        const follows: RestoredBand = { sheet, axis: rows ? 'row' : 'column', at, count };
        return [
          { method: rows ? 'insertRows' : 'insertColumns', args: [sheet, at, count], locale },
          ...restore(sheet, band).map((operation) => ({ ...operation, follows })),
          ...this.brokenFormulas(sheet, { ...follows, inserted: false }, band, locale).map((operation) => ({ ...operation, follows })),
        ];
      }
      case 'renameSheet': {
        const name = summary()?.name;
        return name === undefined ? [] : [{ method: 'renameSheet', args: [sheet, name], locale }];
      }
      case 'setColumnWidth': {
        const width = summary()?.columnWidths.find(([column]) => column === args[1])?.[1] ?? null;
        return [{ method: 'setColumnWidth', args: [sheet, args[1], width], locale }];
      }
      case 'setRowHeight': {
        const height = summary()?.rowHeights.find(([row]) => row === args[1])?.[1] ?? null;
        return [{ method: 'setRowHeight', args: [sheet, args[1], height], locale }];
      }
      case 'setFrozen': {
        const extent = summary();
        return extent ? [{ method: 'setFrozen', args: [sheet, extent.frozenRows, extent.frozenColumns], locale }] : [];
      }
      case 'setAutoFilter': {
        const filter = summary()?.autoFilter;
        return [{ method: 'setAutoFilter', args: [sheet, filter ? { top: filter.top, left: filter.left, bottom: filter.bottom, right: filter.right } : null], locale }];
      }
      case 'setConditionalFormats':
        return [{ method: 'setConditionalFormats', args: [sheet, this.sheet.conditionalFormats(sheet)], locale }];
      case 'setValidations':
        return [{ method: 'setValidations', args: [sheet, this.sheet.validations(sheet)], locale }];
      case 'setDefinedName':
      case 'removeDefinedName': {
        const name = (args[0] as string).toLowerCase();
        const scope = (method === 'setDefinedName' ? args[2] : args[1]) as number | undefined;
        const before = this.sheet
          .definedNames()
          .find((defined) => defined.name.toLowerCase() === name && defined.sheet === scope);
        return before
          ? [{ method: 'setDefinedName', args: [before.name, before.formula, before.sheet], locale }]
          : [{ method: 'removeDefinedName', args: [args[0], scope], locale }];
      }
      case 'setChart': {
        const chart = this.sheet.charts(sheet)[args[1] as number];
        return chart?.spec
          ? [{ method: 'setChart', args: [sheet, args[1], { chart: chart.spec, placement: chart.placement }], locale }]
          : [];
      }
      case 'removeChart': {
        const chart = this.sheet.charts(sheet)[args[1] as number];
        return chart?.spec ? [{ method: 'addChart', args: [sheet, chart.spec, chart.placement], locale }] : [];
      }
      case 'setPivotTable':
      case 'removePivotTable': {
        const definition = this.pivotDefinition(sheet, args[1] as number);
        if (!definition) return [];
        return method === 'setPivotTable'
          ? [{ method: 'setPivotTable', args: [sheet, args[1], definition], locale }]
          : [{ method: 'addPivotTable', args: [sheet, definition], locale }];
      }
      default:
        return [];
    }
  }

  /**
   * What putting the rows or columns back would not restore of a deletion: the formulas, rules,
   * validations, names and charts elsewhere whose references it cut to `#REF!` or shrank, as
   * they are now. The rows or columns a deletion takes are put back by the region of `band`.
   */
  private brokenFormulas(sheet: number, deletion: ReferenceShift, band: CellWindow, locale: SheetOperation['locale']): SheetOperation[] {
    const sheets = this.sheet.sheets();
    const name = sheets[sheet]?.name;
    if (name === undefined) return [];
    const insertion: ReferenceShift = { ...deletion, inserted: true };
    const survives = (formula: string, home: number): boolean => {
      const target = { sheetName: name, ownSheet: home === sheet };
      return shiftFormula(shiftFormula(formula, deletion, target), insertion, target) === formula;
    };
    const rangeSurvives = (range: CellRange): boolean => {
      const [first, last] = deletion.axis === 'row' ? [range.firstRow, range.lastRow] : [range.firstColumn, range.lastColumn];
      const cut = shiftSpan(first, last, deletion);
      if (!cut) return false;
      const back = shiftSpan(cut[0], cut[1], insertion);
      return back !== null && back[0] === first && back[1] === last;
    };
    const restores: SheetOperation[] = [];
    let kept = 0;
    sheets.forEach((summary, index) => {
      const inputs = this.formulaCells(index, summary.rows, summary.columns).filter(
        ([row, column, input]) =>
          !(index === sheet && row >= band.top && row <= band.bottom && column >= band.left && column <= band.right) &&
          !survives(input, index),
      );
      if (inputs.length > 0 && kept + inputs.length <= MAX_UNDO_AREA) {
        kept += inputs.length;
        restores.push({ method: 'setInputs', args: [index, inputs], locale, guarded: true });
      }
      const formats = this.sheet.conditionalFormats(index);
      const formatBroken = formats.some(
        (format) =>
          (index === sheet && !format.ranges.every(rangeSurvives)) ||
          !format.rules.every((rule) => conditionFormulas(rule.condition).every((formula) => survives(formula, index))),
      );
      if (formatBroken) restores.push({ method: 'setConditionalFormats', args: [index, formats], locale, guarded: true });
      const validations = this.sheet.validations(index);
      const validationBroken = validations.some(
        (validation) =>
          (index === sheet && !validation.ranges.every(rangeSurvives)) ||
          !(validation.formulas ?? []).every((formula) => survives(formula, index)),
      );
      if (validationBroken) restores.push({ method: 'setValidations', args: [index, validations], locale, guarded: true });
      for (const chart of this.sheet.charts(index)) {
        if (chart.spec && !chartFormulas(chart.spec).every((formula) => survives(formula, index))) {
          restores.push({ method: 'setChart', args: [index, chart.index, { chart: chart.spec }], locale, guarded: true });
        }
      }
    });
    for (const defined of this.sheet.definedNames()) {
      if (!survives(defined.formula, defined.sheet ?? 0)) {
        restores.push({ method: 'setDefinedName', args: [defined.name, defined.formula, defined.sheet], locale, guarded: true });
      }
    }
    return restores;
  }

  /** The cells of a sheet that hold a formula, as `[row, column, input]`; none when the sheet is too large to read. */
  private formulaCells(sheet: number, rows: number, columns: number): [number, number, string][] {
    if (rows === 0 || columns === 0 || rows * columns > MAX_SCANNED_AREA) return [];
    const stripe = Math.max(1, Math.floor(MAX_WINDOW_CELLS / columns));
    const found: [number, number, string][] = [];
    for (let top = 0; top < rows; top += stripe) {
      const window = { top, left: 0, bottom: Math.min(rows, top + stripe) - 1, right: columns - 1 };
      for (const cell of this.sheet.cells(sheet, window)) {
        if (cell.input.startsWith('=')) found.push([cell.row, cell.column, cell.input]);
      }
    }
    return found;
  }

  /** A pivot table as the definition that makes it again; null when its source sheet is gone. */
  private pivotDefinition(sheet: number, index: number): PivotTableDefinition | null {
    const table = this.sheet.pivotTables(sheet).find((each) => each.index === index);
    if (!table || table.sourceSheet === null) return null;
    return {
      name: table.name,
      sourceSheet: table.sourceSheet,
      source: table.source,
      anchor: table.anchor,
      rows: table.rows,
      columns: table.columns,
      values: table.values,
      filters: table.filters,
      rowGrandTotals: table.rowGrandTotals,
      columnGrandTotals: table.columnGrandTotals,
      layout: table.layout,
      subtotals: table.subtotals,
      repeatLabels: table.repeatLabels,
      valuesAxis: table.valuesAxis,
      valuesPosition: table.valuesPosition,
    };
  }

  /** Apply an operation in the locale it was typed in; a refused one changes nothing. */
  private apply(operation: SheetOperation, rethrow: boolean): unknown {
    const own = this.sheet.locale();
    const switched = operation.locale !== own;
    if (switched) this.sheet.setLocale(operation.locale);
    try {
      const call = (this.sheet as unknown as Record<string, (...args: unknown[]) => unknown>)[operation.method];
      return call!.apply(this.sheet, operation.args);
    } catch (error) {
      if (rethrow) throw error;
      return undefined;
    } finally {
      if (switched) this.sheet.setLocale(own);
    }
  }

  /** An operation moved past every operation of the others the room ordered after `base`. */
  private movedSince(operation: SheetOperation, base: number, author: string | null): SheetOperation | null {
    let moved: SheetOperation | null = operation;
    for (const entry of this.log) {
      if (moved === null) break;
      if (entry.sequence <= base || entry.operation === null) continue;
      if (author !== null && entry.author === author) continue;
      moved = transformSheetOperation(moved, entry.operation);
    }
    return moved;
  }

  private remote(entry: SequencedOperation<SheetOperation>): void {
    if (entry.sequence <= this.confirmed) return;
    const mine = entry.author === this.session.sessionKey;
    const head = this.pending[0];
    if (mine && head && head.id === entry.id) {
      this.pending.shift();
      this.confirmed = entry.sequence;
      this.log.push(entry);
      if (this.pending.length > 0) {
        this.rebuild(entry);
      } else if (this.sameAsApplied(head.operation, entry.operation, head.base)) {
        this.sheet.discardCheckpoint();
      } else {
        this.rebuild(entry);
      }
    } else {
      this.log.push(entry);
      if (this.pending.length === 0) {
        if (entry.operation !== null) this.apply(entry.operation, false);
        this.confirmed = entry.sequence;
      } else {
        this.rebuild(entry);
        this.confirmed = entry.sequence;
      }
      this.notify();
    }
    this.session.lastSequence = this.confirmed;
    this.prune();
    if (this.pending.length === 0) for (const done of this.idleWaiters.splice(0)) done();
  }

  /** Whether the room placed this client's operation where this client had already applied it. */
  private sameAsApplied(applied: SheetOperation, placed: SheetOperation | null, base: number): boolean {
    const expected = this.movedSince(applied, base, this.session.sessionKey);
    return JSON.stringify(expected) === JSON.stringify(placed);
  }

  /**
   * Go back to what the room agreed, apply what it ordered next, keep that, and put this
   * client's waiting edits back on top, each moved past what the others did since it was made.
   */
  private rebuild(entry: SequencedOperation<SheetOperation>): void {
    if (!this.sheet.rollback()) {
      this.options.onReload?.();
      return;
    }
    if (entry.operation !== null) this.apply(entry.operation, false);
    if (this.pending.length > 0) {
      this.sheet.checkpoint();
      for (const pending of this.pending) {
        const moved = this.movedSince(pending.operation, pending.base, this.session.sessionKey);
        if (moved) this.apply(moved, false);
      }
    }
    this.notify();
  }

  private resend(): void {
    for (const pending of this.pending) this.session.sendOperation(pending);
  }

  private prune(): void {
    const bases = [
      ...this.pending.map((pending) => pending.base),
      ...this.undoSteps.map((step) => step.base),
      ...this.redoSteps.map((step) => step.base),
    ];
    const oldest = bases.length > 0 ? Math.min(...bases) : this.confirmed;
    const keepFrom = this.log.findIndex((entry) => entry.sequence > oldest);
    if (keepFrom > 0) this.log.splice(0, keepFrom);
    else if (keepFrom === -1) this.log = [];
  }

  private notify(): void {
    for (const listener of this.listeners) listener();
  }
}
