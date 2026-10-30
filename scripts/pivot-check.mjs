#!/usr/bin/env node
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

// Compare the pivot tables rely-office lays out with the ones LibreOffice builds from the same
// definitions, as an independent check of the numbers.
//
// 1. The backend package writes a workbook of sales records with pivot tables in the outline
//    and tabular forms, with column fields, several values and blank amounts, and plants a wrong
//    number in the first table's cells.
// 2. LibreOffice opens the xlsx, which makes it build every pivot table again from its source,
//    and saves it as ods.
// 3. Each table's numbers are compared: LibreOffice and Excel caption and order a table their own
//    ways, so what must agree is the list of numbers each holds. The planted number must be gone,
//    or LibreOffice kept the file's cells and the check proves nothing.
//
// GROUPBY and PIVOTBY are not compared: LibreOffice 24.2 does not know them and shows the value
// the file stored.
//
// Usage: node scripts/pivot-check.mjs, after node scripts/build-npm.mjs. SOFFICE names
// LibreOffice (default soffice). Everything is written under target/pivot-check/.

import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '..', '..');
const output = join(root, 'target', 'pivot-check');
const soffice = process.env.SOFFICE ?? 'soffice';
const office = createRequire(import.meta.url)(join(root, 'packages', 'rely-office-node'));

rmSync(output, { recursive: true, force: true });
mkdirSync(output, { recursive: true });

const regions = ['Nord', 'Sud', 'Centro', 'Isole'];
const products = ['Mele', 'Pere', 'Uva'];
const months = ['Gen', 'Feb', 'Mar'];
const records = [['Regione', 'Prodotto', 'Mese', 'Importo']];
for (let index = 0; index < 60; index += 1) {
  const amount = index % 11 === 0 ? '' : String(((index * 37) % 90) + 10);
  records.push([regions[index % 4], products[(index * 7) % 3], months[(index * 5) % 3], amount]);
}

const sheet = office.createSheet({ locale: 'en' });
let planted = 0;
const tables = [
  { name: 'outline', layout: 'outline', rows: [0, 1], columns: [], values: [['sum']] },
  { name: 'tabular', layout: 'tabular', rows: [0], columns: [2], values: [['sum'], ['count']] },
  { name: 'nested', layout: 'tabular', rows: [1], columns: [2, 0], values: [['average']] },
];
try {
  sheet.setInputs(
    0,
    records.flatMap((record, row) => record.map((text, column) => [row, column, text])),
  );
  const source = { top: 0, left: 0, bottom: records.length - 1, right: 3 };
  tables.forEach((table, index) => {
    sheet.addPivotTable(0, {
      name: table.name,
      sourceSheet: 0,
      source,
      anchor: { row: index * 40, column: 6 + index * 12 },
      rows: table.rows.map((field) => ({ field })),
      columns: table.columns.map((field) => ({ field })),
      values: table.values.map(([fn]) => ({ field: 3, function: fn })),
      layout: table.layout,
      subtotals: 'none',
    });
  });
  // A wrong number in the first table's cells: LibreOffice must build the table again to lose it.
  const cell = sheet
    .cells(0, { top: 0, left: 6, bottom: 38, right: 17 })
    .find((each) => each.display !== '' && !Number.isNaN(Number(each.display)));
  planted = Number(cell.display);
  sheet.setInput(0, cell.row, cell.column, '987654');
  writeFileSync(join(output, 'ours.xlsx'), sheet.save());
} finally {
  sheet.free();
}

const profile = pathToFileURL(join(output, 'profile')).href;
execFileSync(
  soffice,
  [`-env:UserInstallation=${profile}`, '--headless', '--norestore', '--convert-to', 'ods', '--outdir', output, join(output, 'ours.xlsx')],
  { stdio: 'inherit' },
);

/** Every number of a table's area, with the nearest labels to its left and above it. */
function numbersOf(document, area) {
  const cells = document.cells(0, area);
  const grid = new Map(cells.map((cell) => [`${cell.row}:${cell.column}`, cell]));
  const at = (row, column) => grid.get(`${row}:${column}`);
  const numeric = (cell) => cell && cell.display !== '' && !Number.isNaN(Number(cell.display.replace(',', '.')));
  const found = [];
  for (const cell of cells) {
    if (!numeric(cell)) continue;
    // The nearest labels to the left on the row and above in the column.
    let rowLabel = '';
    for (let column = cell.column - 1; column >= area.left && !rowLabel; column -= 1) {
      const label = at(cell.row, column);
      if (label && !numeric(label)) rowLabel = label.display;
    }
    let columnLabel = '';
    for (let row = cell.row - 1; row >= area.top && !columnLabel; row -= 1) {
      const label = at(row, cell.column);
      if (label && !numeric(label)) columnLabel = label.display;
    }
    found.push([`${rowLabel} | ${columnLabel}`, Number(cell.display.replace(',', '.'))]);
  }
  return found;
}

const ours = office.openSheet(readFileSync(join(output, 'ours.xlsx')), 'ours.xlsx', { locale: 'en' });
const theirs = office.openSheet(readFileSync(join(output, 'ours.ods')), 'ours.ods', { locale: 'en' });
let differences = 0;
try {
  tables.forEach((table, index) => {
    const area = { top: index * 40, left: 6 + index * 12, bottom: index * 40 + 38, right: 6 + index * 12 + 11 };
    const mine = numbersOf(ours, area);
    const libre = numbersOf(theirs, area);
    const numbers = (found) =>
      found
        .map(([, value]) => (value === 987654 ? planted : value))
        .map((value) => value.toFixed(6))
        .sort();
    if (index === 0 && libre.some(([, value]) => value === 987654)) {
      console.log('LibreOffice kept the cells of the file instead of building the table: no check');
      differences += 1;
      return;
    }
    const same = JSON.stringify(numbers(mine)) === JSON.stringify(numbers(libre));
    console.log(`${table.name}: ${mine.length} numbers, LibreOffice ${libre.length}: ${same ? 'the same' : 'DIFFERENT'}`);
    if (!same) {
      differences += 1;
      console.log('  ours', mine);
      console.log('  LibreOffice', libre);
    }
  });
} finally {
  ours.free();
  theirs.free();
}
process.exit(differences === 0 ? 0 : 1);
