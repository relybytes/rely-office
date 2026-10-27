// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// Smoke test of the browser entry, run in Node: the module is handed its .wasm as bytes, as a
// bundler would hand it a URL.
//
// Usage: node packages/rely-office-node/test/smoke-web.mjs [path to the package folder]

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const packagePath = resolve(process.argv[2] ?? join(fileURLToPath(import.meta.url), '..', '..'));
const web = await import(pathToFileURL(join(packagePath, 'web', 'index.js')).href);

await web.initSheets(readFileSync(join(packagePath, 'web', 'rely_office_sheet_wasm_bg.wasm')));
const sheet = await web.createSheet({ locale: 'it' });
sheet.setInputs(0, [
  [0, 0, '10'],
  [0, 1, '=A1*3'],
]);
assert.equal(sheet.cells(0, { top: 0, left: 0, bottom: 0, right: 1 })[1].display, '30');
assert.throws(() => sheet.removeSheet(0), (error) => error instanceof web.RelyOfficeError && error.code === 'LAST_SHEET');
const reopened = await web.openSheet(sheet.save(), 'a.xlsx');
assert.equal(reopened.input(0, 0, 1), '=A1*3');
sheet.free();
reopened.free();
console.log('web smoke test passed');
