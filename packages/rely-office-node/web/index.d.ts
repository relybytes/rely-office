// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type { DocumentBytes, ErrorCode, SheetDocument, SheetOptions } from '../index';

export type {
  Border,
  BorderPlacement,
  CellFormat,
  CellRange,
  CellStyle,
  CellWindow,
  DocumentBytes,
  ErrorCode,
  SheetCell,
  SheetDocument,
  SheetOptions,
  SheetSummary,
  StylePatch,
} from '../index';

export class RelyOfficeError extends Error {
  readonly name: 'RelyOfficeError';
  readonly code: ErrorCode;
}

/**
 * Load the WebAssembly module once. By default it fetches the .wasm next to this module, which
 * bundlers resolve as an asset; pass a URL, a Response, the bytes or a compiled module otherwise.
 */
export function initSheets(
  wasm?: string | URL | Response | BufferSource | WebAssembly.Module,
): Promise<unknown>;

/** A new workbook with one empty sheet, Foglio1. */
export function createSheet(options?: SheetOptions): Promise<SheetDocument>;

/** A workbook opened from xlsx, xls, ods, csv or tsv. */
export function openSheet(
  bytes: DocumentBytes,
  fileName?: string,
  options?: SheetOptions,
): Promise<SheetDocument>;
