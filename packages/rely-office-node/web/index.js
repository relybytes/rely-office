// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// The spreadsheet for the browser: the same SheetDocument as the Node package, over the
// WebAssembly module built for the web. `initSheets` loads the module once; by default it
// fetches the .wasm next to this file, which bundlers resolve as an asset.

import init, { SheetDocument as WasmSheetDocument } from './rely_office_sheet_wasm.js';

let ready = null;

/** Load the WebAssembly module once; `wasm` is a URL, a Response, bytes or a compiled module. */
export function initSheets(wasm) {
  if (ready === null) {
    ready = init(wasm === undefined ? undefined : { module_or_path: wasm });
  }
  return ready;
}

/** An error of rely-office, with the stable code of what went wrong. */
export class RelyOfficeError extends Error {
  constructor(code, message) {
    super(message);
    this.name = 'RelyOfficeError';
    this.code = code;
  }
}

const CODED_MESSAGE = /^([A-Z_]+): ([\s\S]*)$/;

/** Run a wasm call, turning its coded `Error` into a `RelyOfficeError`. */
function call(operation) {
  try {
    return operation();
  } catch (error) {
    const match = error instanceof Error ? CODED_MESSAGE.exec(error.message) : null;
    if (match) {
      throw new RelyOfficeError(match[1], match[2]);
    }
    throw error;
  }
}

/** The document as a `Uint8Array`; a Node `Buffer` already is one. */
function toBytes(input) {
  if (input instanceof Uint8Array) {
    return input;
  }
  if (input instanceof ArrayBuffer) {
    return new Uint8Array(input);
  }
  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  }
  throw new TypeError('rely-office: the document must be a Uint8Array, a Buffer or an ArrayBuffer');
}

function toFileName(fileName) {
  return fileName === undefined || fileName === null ? undefined : String(fileName);
}

/** A spreadsheet being edited; its methods mirror the editor's actions. */
export class SheetDocument {
  constructor(document) {
    this.document = document;
  }

  sheets() {
    return JSON.parse(call(() => this.document.sheetsJson()));
  }

  cells(sheet, { top, left, bottom, right }) {
    return JSON.parse(call(() => this.document.cellsJson(sheet, top, left, bottom, right)));
  }

  input(sheet, row, column) {
    return this.document.input(sheet, row, column);
  }

  setInput(sheet, row, column, text) {
    call(() => this.document.setInput(sheet, row, column, String(text)));
  }

  setInputs(sheet, inputs) {
    call(() => this.document.setInputs(sheet, JSON.stringify(inputs)));
  }

  setFormat(sheet, row, column, format) {
    call(() => this.document.setFormat(sheet, row, column, JSON.stringify(format)));
  }

  setColumnWidth(sheet, column, width) {
    call(() => this.document.setColumnWidth(sheet, column, width ?? -1));
  }

  setStyle(sheet, { top, left, bottom, right }, patch) {
    call(() => this.document.setStyle(sheet, top, left, bottom, right, JSON.stringify(patch ?? {})));
  }

  setBorders(sheet, { top, left, bottom, right }, placement, border) {
    call(() =>
      this.document.setBorders(sheet, top, left, bottom, right, String(placement), JSON.stringify(border ?? null)),
    );
  }

  clearStyles(sheet, { top, left, bottom, right }) {
    call(() => this.document.clearStyles(sheet, top, left, bottom, right));
  }

  styles() {
    return JSON.parse(call(() => this.document.stylesJson()));
  }

  merges(sheet) {
    return JSON.parse(call(() => this.document.mergesJson(sheet)));
  }

  mergeCells(sheet, { top, left, bottom, right }) {
    call(() => this.document.mergeCells(sheet, top, left, bottom, right));
  }

  unmergeCells(sheet, { top, left, bottom, right }) {
    call(() => this.document.unmergeCells(sheet, top, left, bottom, right));
  }

  setRowHeight(sheet, row, height) {
    call(() => this.document.setRowHeight(sheet, row, height ?? -1));
  }

  clearRange(sheet, { top, left, bottom, right }) {
    call(() => this.document.clearRange(sheet, top, left, bottom, right));
  }

  copyRange(sheet, { top, left, bottom, right }, targetSheet, row, column) {
    call(() => this.document.copyRange(sheet, top, left, bottom, right, targetSheet, row, column));
  }

  insertRows(sheet, at, count = 1) {
    call(() => this.document.insertRows(sheet, at, count));
  }

  deleteRows(sheet, at, count = 1) {
    call(() => this.document.deleteRows(sheet, at, count));
  }

  insertColumns(sheet, at, count = 1) {
    call(() => this.document.insertColumns(sheet, at, count));
  }

  deleteColumns(sheet, at, count = 1) {
    call(() => this.document.deleteColumns(sheet, at, count));
  }

  addSheet(name) {
    return call(() => this.document.addSheet(String(name)));
  }

  renameSheet(sheet, name) {
    call(() => this.document.renameSheet(sheet, String(name)));
  }

  removeSheet(sheet) {
    call(() => this.document.removeSheet(sheet));
  }

  conditionalFormats(sheet) {
    return JSON.parse(call(() => this.document.conditionalFormatsJson(sheet)));
  }

  setConditionalFormats(sheet, formats) {
    call(() => this.document.setConditionalFormats(sheet, JSON.stringify(formats ?? [])));
  }

  validations(sheet) {
    return JSON.parse(call(() => this.document.validationsJson(sheet)));
  }

  setValidations(sheet, validations) {
    call(() => this.document.setValidations(sheet, JSON.stringify(validations ?? [])));
  }

  conditionalLooks(sheet) {
    return JSON.parse(call(() => this.document.conditionalLooksJson(sheet)));
  }

  validation(sheet, row, column) {
    return JSON.parse(call(() => this.document.validationJson(sheet, row, column)));
  }

  invalidCells(sheet) {
    return JSON.parse(call(() => this.document.invalidCellsJson(sheet)));
  }

  /** Today's date for TODAY() and NOW(): a Date, a serial day count, or null to unset it. */
  setToday(today) {
    this.document.setToday(toSerial(today));
  }

  previewInput(text) {
    return this.document.previewInput(String(text));
  }

  save() {
    return call(() => this.document.save());
  }

  saveOds() {
    return call(() => this.document.saveOds());
  }

  free() {
    this.document.free();
  }
}

/** Days since 30 December 1899, as spreadsheets count dates; -1 for none. */
function toSerial(today) {
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


/** A new workbook with one empty sheet, after `initSheets`. */
export async function createSheet(options) {
  await initSheets();
  const locale = options?.locale ?? 'it';
  return new SheetDocument(call(() => new WasmSheetDocument(locale)));
}

/** A workbook opened from a spreadsheet file, after `initSheets`. */
export async function openSheet(bytes, fileName, options) {
  await initSheets();
  const locale = options?.locale ?? 'it';
  return new SheetDocument(call(() => WasmSheetDocument.open(toBytes(bytes), toFileName(fileName), locale)));
}
