// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// The spreadsheet in the browser: the same SheetDocument as the backend package, over the
// WebAssembly module built for the web. `initSheets` loads the module once.

import init, { SheetDocument as WasmSheetDocument } from '../../wasm/rely_office_sheet_wasm.js';
import type {
  Border,
  BorderPlacement,
  CellComment,
  CellFormat,
  CellRange,
  CellStyle,
  CellValidation,
  CellWindow,
  ChartDefinition,
  ChartPlacement,
  ChartType,
  ConditionalFormat,
  ConditionalLook,
  DataValidation,
  DefinedName,
  DocumentBytes,
  FilterChoice,
  SheetCell,
  SheetChart,
  SheetDocument,
  SheetOptions,
  SheetPicture,
  SheetSummary,
  SortKey,
  StylePatch,
} from '../model.js';
import { call, toBytes, toFileName, type WasmSource } from './shared.js';

let loading: Promise<void> | null = null;

/** Load the module once; `wasm` is a URL, a Response, bytes or a compiled module. */
export function initSheets(wasm?: WasmSource): Promise<void> {
  if (loading === null) {
    loading = init(wasm === undefined ? undefined : { module_or_path: wasm }).then(() => undefined);
    loading.catch(() => {
      loading = null;
    });
  }
  return loading;
}

/** Days since 30 December 1899, as spreadsheets count dates; -1 for none. */
function toSerial(today: Date | number | null): number {
  if (today === null || today === undefined) {
    return -1;
  }
  if (today instanceof Date) {
    const midnight = Date.UTC(today.getFullYear(), today.getMonth(), today.getDate());
    const local = today.getHours() / 24 + today.getMinutes() / 1440 + today.getSeconds() / 86400;
    return (midnight - Date.UTC(1899, 11, 30)) / 86400000 + local;
  }
  return Number(today);
}

function parse<T>(json: () => string): T {
  return JSON.parse(call(json)) as T;
}

/** A workbook being edited in the browser; its methods mirror the editor's actions. */
class BrowserSheetDocument implements SheetDocument {
  constructor(private readonly document: WasmSheetDocument) {}

  sheets(): SheetSummary[] {
    return parse(() => this.document.sheetsJson());
  }

  cells(sheet: number, { top, left, bottom, right }: CellWindow): SheetCell[] {
    return parse(() => this.document.cellsJson(sheet, top, left, bottom, right));
  }

  input(sheet: number, row: number, column: number): string {
    return this.document.input(sheet, row, column);
  }

  setInput(sheet: number, row: number, column: number, text: string): void {
    call(() => this.document.setInput(sheet, row, column, String(text)));
  }

  setInputs(sheet: number, inputs: [number, number, string][]): void {
    call(() => this.document.setInputs(sheet, JSON.stringify(inputs)));
  }

  setFormat(sheet: number, row: number, column: number, format: CellFormat): void {
    call(() => this.document.setFormat(sheet, row, column, JSON.stringify(format)));
  }

  setColumnWidth(sheet: number, column: number, width: number | null): void {
    call(() => this.document.setColumnWidth(sheet, column, width ?? -1));
  }

  clearRange(sheet: number, { top, left, bottom, right }: CellWindow): void {
    call(() => this.document.clearRange(sheet, top, left, bottom, right));
  }

  setStyle(sheet: number, { top, left, bottom, right }: CellWindow, patch: StylePatch): void {
    call(() => this.document.setStyle(sheet, top, left, bottom, right, JSON.stringify(patch ?? {})));
  }

  setBorders(
    sheet: number,
    { top, left, bottom, right }: CellWindow,
    placement: BorderPlacement,
    border: Border | null,
  ): void {
    call(() =>
      this.document.setBorders(
        sheet,
        top,
        left,
        bottom,
        right,
        String(placement),
        JSON.stringify(border ?? null),
      ),
    );
  }

  clearStyles(sheet: number, { top, left, bottom, right }: CellWindow): void {
    call(() => this.document.clearStyles(sheet, top, left, bottom, right));
  }

  styles(): CellStyle[] {
    return parse(() => this.document.stylesJson());
  }

  merges(sheet: number): CellRange[] {
    return parse(() => this.document.mergesJson(sheet));
  }

  definedNames(): DefinedName[] {
    return parse<(DefinedName & { sheet?: number | null })[]>(() => this.document.definedNamesJson()).map(
      ({ sheet, ...name }) => (sheet === null || sheet === undefined ? name : { ...name, sheet }),
    );
  }

  setDefinedName(name: string, formula: string, sheet?: number): void {
    call(() => this.document.setDefinedName(name, sheet ?? undefined, formula));
  }

  removeDefinedName(name: string, sheet?: number): boolean {
    return this.document.removeDefinedName(name, sheet ?? undefined);
  }

  charts(sheet: number): SheetChart[] {
    return parse(() => this.document.chartsJson(sheet));
  }

  addChart(sheet: number, chart: ChartDefinition, placement: ChartPlacement): number {
    return call(() => this.document.addChart(sheet, JSON.stringify(chart), JSON.stringify(placement)));
  }

  setChart(sheet: number, index: number, { chart, placement }: { chart?: ChartDefinition; placement?: ChartPlacement } = {}): void {
    call(() =>
      this.document.setChart(
        sheet,
        index,
        chart === undefined ? undefined : JSON.stringify(chart),
        placement === undefined ? undefined : JSON.stringify(placement),
      ),
    );
  }

  removeChart(sheet: number, index: number): void {
    call(() => this.document.removeChart(sheet, index));
  }

  chartForRange(sheet: number, { top, left, bottom, right }: CellWindow, type: ChartType): ChartDefinition | null {
    return parse(() => this.document.chartForRangeJson(sheet, top, left, bottom, right, type));
  }

  comments(sheet: number): CellComment[] {
    return parse(() => this.document.commentsJson(sheet));
  }

  setComment(sheet: number, row: number, column: number, text: string, author = ''): void {
    call(() => this.document.setComment(sheet, row, column, author, text));
  }

  startCommentThread(sheet: number, row: number, column: number, text: string, author = '', date = ''): void {
    call(() => this.document.startCommentThread(sheet, row, column, author, text, date));
  }

  replyToComment(sheet: number, row: number, column: number, text: string, author = '', date = ''): void {
    call(() => this.document.replyToComment(sheet, row, column, author, text, date));
  }

  setCommentResolved(sheet: number, row: number, column: number, resolved: boolean): void {
    call(() => this.document.setCommentResolved(sheet, row, column, resolved));
  }

  removeCommentReply(sheet: number, row: number, column: number, reply: number): void {
    call(() => this.document.removeCommentReply(sheet, row, column, reply));
  }

  mergeCells(sheet: number, { top, left, bottom, right }: CellWindow): void {
    call(() => this.document.mergeCells(sheet, top, left, bottom, right));
  }

  unmergeCells(sheet: number, { top, left, bottom, right }: CellWindow): void {
    call(() => this.document.unmergeCells(sheet, top, left, bottom, right));
  }

  setRowHeight(sheet: number, row: number, height: number | null): void {
    call(() => this.document.setRowHeight(sheet, row, height ?? -1));
  }

  copyRange(
    sheet: number,
    { top, left, bottom, right }: CellWindow,
    targetSheet: number,
    row: number,
    column: number,
  ): void {
    call(() => this.document.copyRange(sheet, top, left, bottom, right, targetSheet, row, column));
  }

  insertRows(sheet: number, at: number, count = 1): void {
    call(() => this.document.insertRows(sheet, at, count));
  }

  deleteRows(sheet: number, at: number, count = 1): void {
    call(() => this.document.deleteRows(sheet, at, count));
  }

  insertColumns(sheet: number, at: number, count = 1): void {
    call(() => this.document.insertColumns(sheet, at, count));
  }

  deleteColumns(sheet: number, at: number, count = 1): void {
    call(() => this.document.deleteColumns(sheet, at, count));
  }

  addSheet(name: string): number {
    return call(() => this.document.addSheet(String(name)));
  }

  renameSheet(sheet: number, name: string): void {
    call(() => this.document.renameSheet(sheet, String(name)));
  }

  removeSheet(sheet: number): void {
    call(() => this.document.removeSheet(sheet));
  }

  conditionalFormats(sheet: number): ConditionalFormat[] {
    return parse(() => this.document.conditionalFormatsJson(sheet));
  }

  setConditionalFormats(sheet: number, formats: ConditionalFormat[]): void {
    call(() => this.document.setConditionalFormats(sheet, JSON.stringify(formats ?? [])));
  }

  validations(sheet: number): DataValidation[] {
    return parse(() => this.document.validationsJson(sheet));
  }

  setValidations(sheet: number, validations: DataValidation[]): void {
    call(() => this.document.setValidations(sheet, JSON.stringify(validations ?? [])));
  }

  conditionalLooks(sheet: number): ConditionalLook[] {
    return parse(() => this.document.conditionalLooksJson(sheet));
  }

  validation(sheet: number, row: number, column: number): CellValidation {
    return parse(() => this.document.validationJson(sheet, row, column));
  }

  invalidCells(sheet: number): [number, number][] {
    return parse(() => this.document.invalidCellsJson(sheet));
  }

  fillRange(sheet: number, source: CellWindow, target: CellWindow): void {
    call(() =>
      this.document.fillRange(
        sheet,
        source.top,
        source.left,
        source.bottom,
        source.right,
        target.top,
        target.left,
        target.bottom,
        target.right,
      ),
    );
  }

  setFrozen(sheet: number, rows: number, columns: number): void {
    call(() => this.document.setFrozen(sheet, rows, columns));
  }

  sortRange(sheet: number, { top, left, bottom, right }: CellWindow, keys: SortKey[], hasHeader: boolean): void {
    call(() => this.document.sortRange(sheet, top, left, bottom, right, JSON.stringify(keys ?? []), Boolean(hasHeader)));
  }

  setAutoFilter(sheet: number, window: CellWindow | null): void {
    if (window === null || window === undefined) {
      call(() => this.document.clearAutoFilter(sheet));
      return;
    }
    const { top, left, bottom, right } = window;
    call(() => this.document.setAutoFilter(sheet, top, left, bottom, right));
  }

  setFilterColumn(sheet: number, column: number, shown: string[] | null): void {
    call(() => this.document.setFilterColumn(sheet, column, JSON.stringify(shown ?? null)));
  }

  filterChoices(sheet: number, column: number): FilterChoice[] {
    return parse(() => this.document.filterChoicesJson(sheet, column));
  }

  pictures(sheet: number): SheetPicture[] {
    return parse(() => this.document.picturesJson(sheet));
  }

  pictureBytes(sheet: number, index: number): Uint8Array {
    return this.document.pictureBytes(sheet, index);
  }

  functionNames(): string[] {
    return JSON.parse(this.document.functionNamesJson()) as string[];
  }

  setToday(today: Date | number | null): void {
    this.document.setToday(toSerial(today));
  }

  previewInput(text: string): 'number' | 'text' | 'bool' | 'formula' | 'empty' | 'invalid' {
    return this.document.previewInput(String(text)) as
      | 'number'
      | 'text'
      | 'bool'
      | 'formula'
      | 'empty'
      | 'invalid';
  }

  save(): Uint8Array {
    return call(() => this.document.save());
  }

  saveOds(): Uint8Array {
    return call(() => this.document.saveOds());
  }

  saveCsv(sheet: number, separator?: string): Uint8Array {
    return call(() => this.document.saveCsv(sheet, separator === undefined ? undefined : String(separator)));
  }

  delimiter(): string | null {
    return this.document.delimiter() ?? null;
  }

  macros(): 'none' | 'kept' | 'dropped' {
    return this.document.macros() as 'none' | 'kept' | 'dropped';
  }

  undo(): boolean {
    return this.document.undo();
  }

  redo(): boolean {
    return this.document.redo();
  }

  canUndo(): boolean {
    return this.document.canUndo();
  }

  canRedo(): boolean {
    return this.document.canRedo();
  }

  free(): void {
    this.document.free();
  }
}

/** A new workbook with one empty sheet, Foglio1, once the module is loaded. */
export async function createSheet(options?: SheetOptions): Promise<SheetDocument> {
  await initSheets();
  const locale = options?.locale ?? 'it';
  return new BrowserSheetDocument(call(() => new WasmSheetDocument(locale)));
}

/** A workbook opened from xlsx, xls, ods, csv or tsv, once the module is loaded. */
export async function openSheet(
  bytes: DocumentBytes,
  fileName?: string,
  options?: SheetOptions,
): Promise<SheetDocument> {
  await initSheets();
  const locale = options?.locale ?? 'it';
  return new BrowserSheetDocument(
    call(() => WasmSheetDocument.open(toBytes(bytes), toFileName(fileName), locale)),
  );
}
