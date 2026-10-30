// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// Smoke test of the built runtime, run in Node: the modules are handed their .wasm as bytes, as a
// bundler would hand them a URL.
//
// Usage: node test/smoke-dist.mjs [path to the package folder]

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const packagePath = resolve(process.argv[2] ?? join(fileURLToPath(import.meta.url), '..', '..'));
const runtime = await import(pathToFileURL(join(packagePath, 'dist', 'runtime', 'index.js')).href);

await runtime.initDocuments(readFileSync(join(packagePath, 'wasm', 'rely_office_wasm_bg.wasm')));
await runtime.initSheets(readFileSync(join(packagePath, 'wasm', 'rely_office_sheet_wasm_bg.wasm')));

const docx = runtime.writeDocx({ blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Ciao' }] }] });
assert.equal(runtime.detectFormat(docx), 'docx');
assert.equal(runtime.readDocumentTree(docx).blocks[0].content[0].text, 'Ciao');
assert.match(readFileSync(join(packagePath, 'dist', 'fonts.css'), 'utf8'), /Carlito-Regular\.ttf/);
assert.ok(readFileSync(join(packagePath, 'dist', 'fonts', 'OFL-Carlito.txt')).length > 0);
const library = await import(pathToFileURL(join(packagePath, 'dist', 'document', 'fonts.js')).href);
assert.equal(library.fontStack('Cambria'), "'Cambria', 'Caladea', serif");

const sheet = await runtime.createSheet({ locale: 'it' });
sheet.setInputs(0, [[0, 0, '2'], [0, 1, '=A1*21']]);
assert.equal(sheet.cells(0, { top: 0, left: 0, bottom: 0, right: 1 })[1].display, '42');
assert.throws(() => sheet.removeSheet(0), (error) => error instanceof runtime.RelyOfficeError && error.code === 'LAST_SHEET');
sheet.free();

console.log('rely-office-react runtime: ok');
