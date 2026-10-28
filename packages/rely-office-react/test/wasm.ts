// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

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
