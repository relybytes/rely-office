// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { initDocuments, initSheets } from '../src/runtime/index.js';

const wasmDirectory = resolve(dirname(fileURLToPath(import.meta.url)), '..', 'wasm');

/** Load both modules from their bytes, as a bundler would hand them their URL. */
export async function loadWasm(): Promise<void> {
  await initDocuments(readFileSync(join(wasmDirectory, 'rely_office_wasm_bg.wasm')));
  await initSheets(readFileSync(join(wasmDirectory, 'rely_office_sheet_wasm_bg.wasm')));
}
