// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import * as Y from 'yjs';
import { SharedDocumentAside, slotIndex } from '../src/collab/document-collab.js';
import { encodeJsonFrame, CollabFrame, sameStateVector, fromBase64, toBase64 } from '../src/collab/protocol.js';
import { CollabRoom, claimedStateVector, type CollabRoomState } from '../src/collab/room.js';
import { collabPeers, type CollabSession } from '../src/collab/session.js';
import { SheetCollaboration } from '../src/collab/sheet-collab.js';
import { removeFormulaSheet, renameFormulaSheet, shiftFormula, type ReferenceShift } from '../src/collab/formula-references.js';
import { shiftIndex, shiftSpan, transformSheetOperation, type SheetOperation } from '../src/collab/sheet-operations.js';
import { SharedPresentation, applyKeyChanges, matchKeys } from '../src/collab/slides-collab.js';
import type { EditorDocument } from '../src/document/editor-document.js';
import type { Presentation, SlideShape } from '../src/model.js';
import { createSheet, openSheet, type SheetDocument } from '../src/runtime/index.js';
import { CollabNetwork } from './collab-network.js';
import { loadWasm } from './wasm.js';

beforeAll(loadWasm);

const ada = { name: 'Ada', color: '#2563eb' };
const bea = { name: 'Bea', color: '#15803d' };

function textRoom(): CollabRoomState {
  return { kind: 'text', baseVersionId: 'v1', baseSequence: -1, sequence: -1, seeded: false, dirty: false, operations: [] };
}

describe('protocol', () => {
  it('round-trips base64 and compares state vectors by content', () => {
    const bytes = new Uint8Array([0, 1, 2, 250, 255]);
    expect(fromBase64(toBase64(bytes))).toEqual(bytes);
    const doc = new Y.Doc();
    doc.getText('t').insert(0, 'a');
    const other = new Y.Doc();
    Y.applyUpdate(other, Y.encodeStateAsUpdate(doc));
    expect(sameStateVector(Y.encodeStateVector(doc), Y.encodeStateVector(other))).toBe(true);
    other.getText('t').insert(0, 'b');
    expect(sameStateVector(Y.encodeStateVector(doc), Y.encodeStateVector(other))).toBe(false);
  });
});

describe('a room of a shared document', () => {
  it('lets the first writer seed and keeps the others in sync', async () => {
    const network = new CollabNetwork(textRoom());
    const first = network.session('text', ada);
    await network.flush();
    expect(first.welcome).toMatchObject({ seed: true, seeded: false, slot: 0, canWrite: true });
    first.doc.getText('text').insert(0, 'hello');
    first.markSeeded();
    const second = network.session('text', bea);
    await network.flush();
    expect(second.welcome).toMatchObject({ seed: false, seeded: true, slot: 1 });
    expect(second.doc.getText('text').toString()).toBe('hello');
    expect(network.room.isDirty).toBe(false);

    first.doc.getText('text').insert(5, ' world');
    second.doc.getText('text').insert(0, '> ');
    await network.flush();
    expect(first.doc.getText('text').toString()).toBe('> hello world');
    expect(second.doc.getText('text').toString()).toBe('> hello world');
    expect(network.room.isDirty).toBe(true);
    expect(collabPeers(first.awareness).map((peer) => peer.name)).toEqual(['Bea']);
  });

  it('refuses the changes of a reader and hands the seeding on', async () => {
    const network = new CollabNetwork(textRoom());
    const reader = network.session('text', bea, false);
    const seeder = network.session('text', ada);
    await network.flush();
    expect(reader.welcome?.seed).toBe(false);
    expect(seeder.welcome?.seed).toBe(true);
    const errors: unknown[] = [];
    reader.on('control', (message) => errors.push(message));
    reader.doc.getText('text').insert(0, 'nope');
    await network.flush();
    expect(network.room.doc.getText('text').toString()).toBe('');
    expect(errors).toContainEqual({ type: 'error', code: 'READ_ONLY' });

    const late = network.session('text', ada);
    await network.flush();
    const told: string[] = [];
    late.on('control', (message) => told.push(message.type));
    network.drop(seeder);
    await network.flush();
    expect(told).toContain('seed');
  });

  it('is dirty until a save holds everything it has', async () => {
    const network = new CollabNetwork(textRoom());
    const client = network.session('text', ada);
    await network.flush();
    client.markSeeded();
    client.doc.getText('text').insert(0, 'x');
    await network.flush();
    expect(network.room.isDirty).toBe(true);
    const claims: unknown[] = [];
    const saved = network.room;
    saved.markSaved({ versionId: 'v2', stateVector: new Uint8Array([0]) });
    expect(saved.isDirty).toBe(true);
    saved.markSaved({ versionId: 'v3', stateVector: Y.encodeStateVector(client.doc), by: 'Ada' });
    await network.flush();
    expect(saved.isDirty).toBe(false);
    expect(client.welcome?.baseVersionId).toBe('v3');
    expect(saved.snapshot().baseVersionId).toBe('v3');
    expect(claims).toEqual([]);
    expect(claimedStateVector({ type: 'saved', versionId: 'v', stateVector: toBase64(new Uint8Array([1, 2])) })).toEqual(
      new Uint8Array([1, 2]),
    );
    expect(claimedStateVector({ type: 'saved', versionId: 'v' })).toBeUndefined();
  });

  it('passes save claims of writers to its host and comes back from a snapshot', async () => {
    const claims: string[] = [];
    const updates: Uint8Array[] = [];
    const network = new CollabNetwork(textRoom(), {
      events: { saveClaim: (_peer, claim) => claims.push(claim.versionId), update: (update) => updates.push(update) },
    });
    const client = network.session('text', ada);
    await network.flush();
    client.doc.getText('text').insert(0, 'kept');
    client.markSeeded();
    client.claimSaved('v9', { stateVector: Y.encodeStateVector(client.doc) });
    await network.flush();
    expect(claims).toEqual(['v9']);
    expect(updates.length).toBeGreaterThan(0);

    const restored = new CollabRoom(network.room.snapshot());
    expect(restored.doc.getText('text').toString()).toBe('kept');
    expect(restored.snapshot().seeded).toBe(true);
  });

  it('answers a frame it cannot read with an error', () => {
    const room = new CollabRoom(textRoom());
    const sent: Uint8Array[] = [];
    room.join({ id: 'p', canWrite: true, send: (frame) => sent.push(frame) });
    room.receive('p', new Uint8Array([]));
    room.receive('p', encodeJsonFrame(CollabFrame.Control, { type: 'hello', sessionKey: '' }));
    expect(sent.length).toBe(2);
    room.receive('unknown', new Uint8Array([0]));
    room.leave('unknown');
    expect(room.size).toBe(1);
  });
});

describe('sheet operations', () => {
  const op = (method: SheetOperation['method'], ...args: unknown[]): SheetOperation => ({ method, args, locale: 'it' });

  it('shifts indexes and spans past insertions and deletions', () => {
    expect(shiftIndex(5, { at: 2, count: 3, inserted: true })).toBe(8);
    expect(shiftIndex(1, { at: 2, count: 3, inserted: true })).toBe(1);
    expect(shiftIndex(3, { at: 2, count: 3, inserted: false })).toBeNull();
    expect(shiftIndex(6, { at: 2, count: 3, inserted: false })).toBe(3);
    expect(shiftSpan(1, 6, { at: 2, count: 3, inserted: false })).toEqual([1, 3]);
    expect(shiftSpan(2, 4, { at: 2, count: 3, inserted: false })).toBeNull();
    expect(shiftSpan(3, 8, { at: 2, count: 3, inserted: false })).toEqual([2, 5]);
  });

  it('moves cell edits past rows and columns', () => {
    expect(transformSheetOperation(op('setInput', 0, 4, 1, 'x'), op('insertRows', 0, 2, 1))!.args).toEqual([0, 5, 1, 'x']);
    expect(transformSheetOperation(op('setInput', 0, 4, 1, 'x'), op('insertRows', 1, 2, 1))!.args).toEqual([0, 4, 1, 'x']);
    expect(transformSheetOperation(op('setInput', 0, 2, 1, 'x'), op('deleteRows', 0, 2, 1))).toBeNull();
    expect(transformSheetOperation(op('setInput', 0, 2, 5, 'x'), op('deleteColumns', 0, 0, 2))!.args).toEqual([0, 2, 3, 'x']);
    expect(
      transformSheetOperation(op('setInputs', 0, [[1, 0, 'a'], [3, 0, 'b']]), op('deleteRows', 0, 1, 1))!.args,
    ).toEqual([0, [[2, 0, 'b']]]);
    expect(
      transformSheetOperation(op('setStyle', 0, { top: 1, left: 0, bottom: 4, right: 0 }, {}), op('deleteRows', 0, 2, 2))!.args[1],
    ).toEqual({ top: 1, left: 0, bottom: 2, right: 0 });
  });

  it('moves structure past structure and sheets past removed ones', () => {
    expect(transformSheetOperation(op('insertRows', 0, 5, 1), op('insertRows', 0, 5, 2))!.args).toEqual([0, 7, 1]);
    expect(transformSheetOperation(op('deleteRows', 0, 5, 2), op('insertRows', 0, 6, 1))!.args).toEqual([0, 5, 3]);
    expect(transformSheetOperation(op('deleteRows', 0, 5, 2), op('deleteRows', 0, 4, 2))!.args).toEqual([0, 4, 1]);
    expect(transformSheetOperation(op('deleteRows', 0, 5, 1), op('deleteRows', 0, 5, 1))).toBeNull();
    expect(transformSheetOperation(op('setInput', 2, 0, 0, 'x'), op('removeSheet', 1))!.args[0]).toBe(1);
    expect(transformSheetOperation(op('setInput', 1, 0, 0, 'x'), op('removeSheet', 1))).toBeNull();
    expect(transformSheetOperation(op('removeSheet', 1), op('removeSheet', 1))).toBeNull();
    expect(transformSheetOperation(op('setChart', 0, 2, {}), op('removeChart', 0, 1))!.args[1]).toBe(1);
    expect(transformSheetOperation(op('setChart', 0, 1, {}), op('removeChart', 0, 1))).toBeNull();
    expect(transformSheetOperation(op('setInput', 0, 1, 1, 'x'), op('setInput', 0, 1, 1, 'y'))!.args[3]).toBe('x');
  });

  it('carries the new name of a sheet on a change to its rows made before the rename', () => {
    const rename: SheetOperation = { ...op('renameSheet', 1, 'Nuovo'), sheetName: 'Dati' };
    expect(transformSheetOperation({ ...op('deleteRows', 1, 0, 1), sheetName: 'Dati' }, rename)!.sheetName).toBe('Nuovo');
    expect(transformSheetOperation({ ...op('insertRows', 0, 0, 1), sheetName: 'Foglio1' }, rename)!.sheetName).toBe('Foglio1');
    expect(transformSheetOperation({ ...op('renameSheet', 1, 'Altro'), sheetName: 'Dati' }, rename)!.sheetName).toBe('Nuovo');
    expect(transformSheetOperation(op('setInput', 0, 0, 0, '=Dati!A1'), rename)!.args[3]).toBe('=Nuovo!A1');
  });

  it('moves what an undo puts back around the rows it inserts again, and keeps what others wrote', () => {
    const follows = { sheet: 0, axis: 'row' as const, at: 1, count: 2 };
    const restore: SheetOperation = { ...op('setInputs', 0, [[5, 2, '=A2'], [7, 2, '=A9']]), guarded: true, follows };
    // An insertion after the band, in the rows as the others saw them: past a cell put back at 5 (3 for them).
    const after = transformSheetOperation(restore, op('insertRows', 0, 4, 1))!;
    expect(after.args).toEqual([0, [[5, 2, '=A2'], [8, 2, '=A10']]]);
    expect(after.follows).toEqual(follows);
    // An insertion above it moves the band too.
    const above = transformSheetOperation(restore, op('insertRows', 0, 0, 1))!;
    expect(above.args).toEqual([0, [[6, 2, '=A3'], [8, 2, '=A10']]]);
    expect(above.follows).toEqual({ ...follows, at: 2 });
    // A deletion across the band's place takes the rows on both sides of it.
    const across = transformSheetOperation(restore, op('deleteRows', 0, 0, 2))!;
    expect(across.follows).toEqual({ ...follows, at: 0 });
    expect(across.args).toEqual([0, [[3, 2, '=A1'], [5, 2, '=A7']]]);
    // A cell another client wrote since is left as they wrote it: 7 is 5 for them.
    expect(transformSheetOperation(restore, op('setInput', 0, 5, 2, 'mine'))!.args).toEqual([0, [[5, 2, '=A2']]]);
    expect(transformSheetOperation(restore, op('setInput', 0, 7, 2, 'mine'))!.args).toEqual(restore.args);
    expect(transformSheetOperation({ ...op('setConditionalFormats', 0, []), guarded: true }, op('setConditionalFormats', 0, []))).toBeNull();
    expect(transformSheetOperation({ ...op('setDefinedName', 'Somma', '=A1'), guarded: true }, op('setDefinedName', 'SOMMA', '=B1'))).toBeNull();
  });

  it('moves comment threads and pivot tables past structure', () => {
    expect(transformSheetOperation(op('replyToComment', 0, 3, 0, 'ok'), op('insertRows', 0, 0, 2))!.args).toEqual([0, 5, 0, 'ok']);
    const pivot = { sourceSheet: 1, source: { top: 0, left: 0, bottom: 9, right: 2 }, anchor: { row: 4, column: 4 } };
    const moved = transformSheetOperation(op('addPivotTable', 0, pivot), op('insertRows', 0, 0, 1))!.args[1];
    expect(moved).toMatchObject({ anchor: { row: 5, column: 4 }, source: { top: 0, bottom: 9 } });
    const source = transformSheetOperation(op('addPivotTable', 0, pivot), op('deleteRows', 1, 9, 1))!.args[1];
    expect(source).toMatchObject({ source: { top: 0, bottom: 8 } });
    expect(transformSheetOperation(op('addPivotTable', 0, pivot), op('removeSheet', 1))).toBeNull();
    expect(transformSheetOperation(op('setPivotTable', 0, 2, pivot), op('removePivotTable', 0, 1))!.args[1]).toBe(1);
    expect(transformSheetOperation(op('refreshPivotTable', 0, 1), op('removePivotTable', 0, 1))).toBeNull();
    expect(transformSheetOperation(op('refreshPivotTables'), op('removeSheet', 0))!.method).toBe('refreshPivotTables');
  });
});

describe('the references of a formula', () => {
  const rows = (at: number, count: number, inserted: boolean): ReferenceShift => ({ axis: 'row', at, count, inserted });
  const columns = (at: number, count: number, inserted: boolean): ReferenceShift => ({ axis: 'column', at, count, inserted });
  const own = { ownSheet: true, sheetName: 'Uno' };

  it('moves cells, ranges and whole columns past inserted rows and columns', () => {
    expect(shiftFormula('=A1+A3+$A$5+A$3+SUM(A2:A4)+SUM(A3:A4)+SUM(A:A)', rows(2, 1, true), own)).toBe(
      '=A1+A4+$A$6+A$4+SUM(A2:A5)+SUM(A4:A5)+SUM(A:A)',
    );
    expect(shiftFormula('=B2+$A1+SUM(A:C)+SUM(C:D)', columns(1, 2, true), own)).toBe('=D2+$A1+SUM(A:E)+SUM(E:F)');
    expect(shiftFormula('=SUM(2:4)+SUM(5:5)', rows(3, 1, true), own)).toBe('=SUM(2:5)+SUM(6:6)');
    expect(shiftFormula('=a3+$b$4', rows(0, 1, true), own)).toBe('=A4+$B$5');
  });

  it('shrinks ranges and writes #REF! for what deleted rows took', () => {
    expect(shiftFormula('=A1+A3+$A$5+A$3+SUM(A1:A10)+SUM(A2:A4)+SUM(A3:A4)', rows(2, 2, false), own)).toBe(
      '=A1+#REF!+$A$3+#REF!+SUM(A1:A8)+SUM(A2:A2)+SUM(#REF!)',
    );
    expect(shiftFormula('=Uno!C1+Uno!B1:B3', columns(1, 2, false), own)).toBe('=#REF!+#REF!');
    expect(shiftFormula('=SUM(A4:A2)', rows(1, 1, false), own)).toBe('=SUM(A3:A2)');
  });

  it('writes #REF! for a reference pushed past the last row or column', () => {
    expect(shiftFormula('=A1048576+XFD1+A1', rows(0, 1, true), own)).toBe('=#REF!+XFD2+A2');
    expect(shiftFormula('=XFD1', columns(0, 1, true), own)).toBe('=#REF!');
  });

  it('follows the sheet a reference names', () => {
    const elsewhere = { ownSheet: false, sheetName: 'Due' };
    expect(shiftFormula("=A3+Due!A3+'Due'!$B$3:$B$9+due!A3+Tre!A3", rows(0, 1, true), elsewhere)).toBe(
      '=A3+Due!A4+Due!$B$4:$B$10+due!A4+Tre!A3',
    );
    expect(shiftFormula("='My Sheet'!A3+A3", rows(0, 1, true), { ownSheet: true, sheetName: 'My Sheet' })).toBe(
      "='My Sheet'!A4+A4",
    );
    expect(shiftFormula('=Due!A3+A3', rows(0, 1, true), { ownSheet: true })).toBe('=Due!A3+A4');
  });

  it('leaves texts, functions, names, numbers, tables and errors alone', () => {
    const untouched = [
      '=LEN("A3")&"B2:B9"',
      '=LOG10(4)+ATAN2(1;2)+LOG10 (5)',
      '=Tax+TAX2023x+_A3',
      '=Uno!Total+Uno!TotalA3',
      '=SOMMA(2,5;1E5;1e-3)+CONTA.VALORI(Tab[A3])',
      '=Tab[[#Headers],[A3]]',
      '=#REF!+#N/A+#DIV/0!',
      '=[Book2]Uno!A3',
      "=IF(A1=\"x\";'Tre'!Nome;1)",
    ];
    for (const formula of untouched) {
      const moved = shiftFormula(formula, rows(0, 1, true), { ownSheet: false, sheetName: 'Uno' });
      expect(moved).toBe(formula);
    }
    expect(shiftFormula('=SOMMA(A1:A3;2,5)+CONTA.VALORI(B2:B4)', rows(1, 1, true), own)).toBe(
      '=SOMMA(A1:A4;2,5)+CONTA.VALORI(B3:B5)',
    );
    expect(shiftFormula('=SUM( A2 : A4 )', rows(0, 1, true), own)).toBe('=SUM( A3:A5 )');
  });

  it('renames and removes the sheet references name', () => {
    expect(renameFormulaSheet("=Due!A3+'Due'!B2:B9+A3+Duet!A1", 'Due', "It's new")).toBe(
      "='It''s new'!A3+'It''s new'!B2:B9+A3+Duet!A1",
    );
    expect(renameFormulaSheet("='My Sheet'!B3", 'my sheet', 'Plain')).toBe('=Plain!B3');
    expect(renameFormulaSheet('=Old!A1', 'Old', 'A1')).toBe("='A1'!A1");
    expect(renameFormulaSheet('=Old!A1', 'Old', '2024')).toBe("='2024'!A1");
    expect(removeFormulaSheet("=Due!A3+'Due'!B2:B9+A3+\"Due!A3\"", 'Due')).toBe('=#REF!+#REF!+A3+"Due!A3"');
  });
});

describe('formulas carried past structure, against the workbook', () => {
  type Workbook = Awaited<ReturnType<typeof createSheet>>;

  async function workbook(locale: 'it' | 'en'): Promise<Workbook> {
    const sheet = await createSheet({ locale });
    sheet.renameSheet(0, 'Uno');
    sheet.addSheet('Due');
    sheet.addSheet('My Sheet');
    return sheet;
  }

  const named = (operation: SheetOperation, sheet: Workbook): SheetOperation =>
    ['insertRows', 'deleteRows', 'insertColumns', 'deleteColumns', 'renameSheet', 'removeSheet'].includes(operation.method)
      ? { ...operation, sheetName: sheet.sheets()[operation.args[0] as number]!.name }
      : operation;

  function apply(sheet: Workbook, operation: SheetOperation): void {
    (sheet as unknown as Record<string, (...args: unknown[]) => unknown>)[operation.method]!(...operation.args);
  }

  /**
   * What the workbook does to an edit made first and then moved by a structural change, against
   * the edit moved by the transform and made after it: the two must leave the same workbook.
   */
  async function bothWays(
    locale: 'it' | 'en',
    edit: SheetOperation,
    change: SheetOperation,
    read: (sheet: Workbook) => unknown,
  ): Promise<[unknown, unknown]> {
    const first = await workbook(locale);
    apply(first, edit);
    apply(first, named(change, first));
    const second = await workbook(locale);
    const sent = named(change, second);
    apply(second, sent);
    const moved = transformSheetOperation(edit, sent);
    if (moved) apply(second, moved);
    const result: [unknown, unknown] = [read(first), read(second)];
    first.free();
    second.free();
    return result;
  }

  const changes = (sheet: number): SheetOperation[] => [
    { method: 'insertRows', args: [sheet, 0, 1], locale: 'en' },
    { method: 'insertRows', args: [sheet, 2, 3], locale: 'en' },
    { method: 'insertRows', args: [sheet, 9, 1], locale: 'en' },
    { method: 'deleteRows', args: [sheet, 2, 1], locale: 'en' },
    { method: 'deleteRows', args: [sheet, 1, 3], locale: 'en' },
    { method: 'deleteRows', args: [sheet, 0, 12], locale: 'en' },
    { method: 'insertColumns', args: [sheet, 0, 2], locale: 'en' },
    { method: 'insertColumns', args: [sheet, 1, 1], locale: 'en' },
    { method: 'deleteColumns', args: [sheet, 1, 1], locale: 'en' },
    { method: 'deleteColumns', args: [sheet, 0, 3], locale: 'en' },
  ];

  const formulas: Record<'it' | 'en', string[]> = {
    en: [
      '=A1+A3+$A$5+A$3+$B4',
      '=SUM(A2:A4)+SUM(A3:A4)+SUM($A$3:$B$4)+SUM(B:B)+SUM(A:C)',
      "=Uno!A3+Due!B3+'My Sheet'!C3:C9+Due!$A$1:$C$12",
      '=LOG10(4)+ATAN2(1,2)+LEN("A3")+IF(A1>0,B2,C3)',
      '=SUM(A1:J10)+INDEX(B2:D20,2,2)',
    ],
    it: [
      '=SOMMA(A1:A3;2,5)+CONTA.VALORI(B2:B4)+SE(A1>0;B2;C3)',
      "=MEDIA(Uno!A2:A9;'My Sheet'!$B$3)+Due!C2",
    ],
  };

  it('moves cell formulas as the workbook moves those it holds', async () => {
    for (const locale of ['en', 'it'] as const) {
      for (const formula of formulas[locale]) {
        for (const home of [0, 1]) {
          for (const change of [...changes(0), { method: 'renameSheet', args: [1, "It's new"], locale: 'en' } as SheetOperation, { method: 'removeSheet', args: [2], locale: 'en' } as SheetOperation]) {
            const edit: SheetOperation = { method: 'setInput', args: [home, 20, 12, formula], locale };
            const read = (sheet: Workbook) =>
              sheet
                .cells(home, { top: 0, left: 0, bottom: 40, right: 20 })
                .filter((cell) => cell.input !== '')
                .map((cell) => [cell.row, cell.column, cell.input]);
            const [made, moved] = await bothWays(locale, edit, change, read);
            expect(moved, `${formula} on ${home} past ${JSON.stringify(change)}`).toEqual(made);
          }
        }
      }
    }
  });

  it('moves the ranges of a chart as the workbook moves those of a chart it holds', async () => {
    const chart = {
      chartType: 'column',
      stacked: false,
      legend: true,
      categories: 'Uno!$A$2:$A$6',
      series: [
        { name: 'Uno!$B$1', values: 'Uno!$B$2:$B$6' },
        { name: "'My Sheet'!$C$1", values: "'My Sheet'!$C$3:$C$4" },
      ],
    };
    const placement = { row: 0, column: 8, offsetX: 0, offsetY: 0, width: 3_000_000, height: 2_000_000 };
    for (const change of [...changes(0), ...changes(2).slice(3, 5), { method: 'renameSheet', args: [0, 'Primo'], locale: 'en' } as SheetOperation, { method: 'removeSheet', args: [2], locale: 'en' } as SheetOperation]) {
      const [made, moved] = await bothWays(
        'en',
        { method: 'addChart', args: [1, chart, placement], locale: 'en' },
        change,
        (sheet) => sheet.charts(1).map((each) => each.spec),
      );
      if (JSON.stringify(made).includes('#REF!')) {
        // The workbook keeps a chart whose data went as #REF!; one added after is made without it.
        expect(JSON.stringify(moved), JSON.stringify(change)).not.toContain('#REF!');
      } else {
        expect(moved, JSON.stringify(change)).toEqual(made);
      }
    }
  });

  it('moves the ranges and formulas of rules and validations, and defined names', async () => {
    const formats = [
      {
        ranges: [{ firstRow: 2, firstColumn: 0, lastRow: 4, lastColumn: 0 }, { firstRow: 1, firstColumn: 2, lastRow: 1, lastColumn: 2 }],
        rules: [
          { condition: { kind: 'expression', formula: 'A3>B$1+Due!C3' } },
          { condition: { kind: 'cellIs', operator: 'greater', formulas: ['$B$2'] } },
        ],
      },
    ];
    const validations = [
      { ranges: [{ firstRow: 2, firstColumn: 1, lastRow: 3, lastColumn: 1 }], kind: 'custom', formulas: ['B3>$A$1'] },
      { ranges: [{ firstRow: 5, firstColumn: 3, lastRow: 6, lastColumn: 3 }], kind: 'list', formulas: ['$A$1:$A$4'] },
    ];
    for (const change of [...changes(0), ...changes(1).slice(0, 2), { method: 'renameSheet', args: [1, 'Nuovo foglio'], locale: 'en' } as SheetOperation, { method: 'removeSheet', args: [1], locale: 'en' } as SheetOperation]) {
      const label = JSON.stringify(change);
      const [madeRules, movedRules] = await bothWays(
        'en',
        { method: 'setConditionalFormats', args: [0, formats], locale: 'en' },
        change,
        (sheet) => sheet.conditionalFormats(0),
      );
      expect(movedRules, label).toEqual(madeRules);
      const [madeValidations, movedValidations] = await bothWays(
        'en',
        { method: 'setValidations', args: [0, validations], locale: 'en' },
        change,
        (sheet) => sheet.validations(0),
      );
      expect(movedValidations, label).toEqual(madeValidations);
      for (const scope of [undefined, 0, 1]) {
        const [madeNames, movedNames] = await bothWays(
          'en',
          { method: 'setDefinedName', args: ['Totale', '=$A$3:$B$9', scope], locale: 'en' },
          change,
          (sheet) => sheet.definedNames(),
        );
        expect(movedNames, `${label} scope ${scope}`).toEqual(madeNames);
      }
    }
  });
});

describe('a shared workbook', () => {
  function sheetRoom(): CollabRoomState<SheetOperation> {
    return { kind: 'spreadsheet', baseVersionId: 'v1', baseSequence: -1, sequence: -1, seeded: true, dirty: false, operations: [] };
  }

  async function client(network: CollabNetwork<SheetOperation>, user = ada, base?: Uint8Array) {
    const session = network.session('spreadsheet', user);
    await network.flush();
    const sheet = base ? await openSheet(base, 'base.xlsx', { locale: 'it' }) : await createSheet({ locale: 'it' });
    const binding = new SheetCollaboration(session, sheet);
    await network.flush();
    return { session, sheet, binding, grid: binding.document };
  }

  function contents(sheet: SheetDocument): [number, number, string][] {
    return sheet
      .cells(0, { top: 0, left: 0, bottom: 30, right: 10 })
      .filter((cell) => cell.input !== '')
      .map((cell) => [cell.row, cell.column, cell.input]);
  }

  it('applies concurrent edits in the same order everywhere', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    const b = await client(network, bea);
    a.grid.setInput(0, 0, 0, '1');
    b.grid.setInput(0, 1, 1, '2');
    b.grid.setInput(0, 0, 0, '9');
    await network.flush();
    expect(contents(a.sheet)).toEqual(contents(b.sheet));
    expect(contents(a.sheet)).toEqual([
      [0, 0, '9'],
      [1, 1, '2'],
    ]);
    expect(a.binding.sequence).toBe(b.binding.sequence);
    expect(a.binding.waiting).toBe(0);
  });

  it('moves an edit made before a row was inserted above it', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    const b = await client(network, bea);
    a.grid.setInput(0, 3, 0, 'below');
    await network.flush();
    a.grid.insertRows(0, 0, 2);
    b.grid.setInput(0, 3, 1, 'beside');
    b.grid.setInput(0, 0, 0, '=A6');
    await network.flush();
    expect(contents(a.sheet)).toEqual(contents(b.sheet));
    expect(contents(b.sheet)).toEqual([
      [2, 0, '=A8'],
      [5, 0, 'below'],
      [5, 1, 'beside'],
    ]);
  });

  it('moves the references of a formula written while another inserted rows', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    const b = await client(network, bea);
    a.grid.setInputs(0, [
      [0, 0, '1'],
      [1, 0, '2'],
      [2, 0, '3'],
    ]);
    await network.flush();
    a.grid.insertRows(0, 1, 2);
    b.grid.setInput(0, 5, 1, '=SOMMA(A1:A3)+A3*$A$2');
    b.grid.setInput(0, 6, 1, '=A1');
    await network.flush();
    expect(contents(a.sheet)).toEqual(contents(b.sheet));
    const engine = await createSheet({ locale: 'it' });
    engine.setInputs(0, [
      [0, 0, '1'],
      [1, 0, '2'],
      [2, 0, '3'],
      [5, 1, '=SOMMA(A1:A3)+A3*$A$2'],
      [6, 1, '=A1'],
    ]);
    engine.insertRows(0, 1, 2);
    expect(contents(b.sheet)).toEqual(contents(engine));
    expect(contents(b.sheet)).toContainEqual([7, 1, '=SOMMA(A1:A5)+A5*$A$4']);
    expect(a.sheet.cells(0, { top: 7, left: 1, bottom: 7, right: 1 })[0]!.display).toEqual(b.sheet.cells(0, { top: 7, left: 1, bottom: 7, right: 1 })[0]!.display);
    engine.free();
  });

  it('writes #REF! for a reference to rows another deleted, and follows a renamed sheet', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    const b = await client(network, bea);
    a.grid.addSheet('Dati');
    await network.flush();
    a.grid.deleteRows(0, 2, 1);
    a.grid.renameSheet(1, 'Dati 2026');
    b.grid.setInput(0, 0, 3, '=A3+A4+Dati!B2');
    await network.flush();
    expect(contents(a.sheet)).toEqual(contents(b.sheet));
    expect(contents(a.sheet)).toEqual([[0, 3, "=#REF!+A3+'Dati 2026'!B2"]]);
  });

  it('undoes only its own edits', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    const b = await client(network, bea);
    a.grid.setInput(0, 0, 0, 'mine');
    await network.flush();
    b.grid.setInput(0, 1, 0, 'theirs');
    await network.flush();
    expect(a.grid.canUndo()).toBe(true);
    expect(a.grid.undo()).toBe(true);
    await network.flush();
    expect(contents(a.sheet)).toEqual([[1, 0, 'theirs']]);
    expect(contents(b.sheet)).toEqual([[1, 0, 'theirs']]);
    expect(a.grid.canRedo()).toBe(true);
    expect(a.grid.redo()).toBe(true);
    await network.flush();
    expect(contents(b.sheet)).toEqual([
      [0, 0, 'mine'],
      [1, 0, 'theirs'],
    ]);
    expect(b.grid.undo()).toBe(true);
    await network.flush();
    expect(contents(a.sheet)).toEqual([[0, 0, 'mine']]);
  });

  it('undoes a row insertion and a deletion with what the rows held', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    a.grid.setInputs(0, [
      [0, 0, 'a'],
      [1, 0, 'b'],
    ]);
    a.grid.deleteRows(0, 0, 1);
    await network.flush();
    expect(contents(a.sheet)).toEqual([[0, 0, 'b']]);
    a.grid.undo();
    await network.flush();
    expect(contents(a.sheet)).toEqual([
      [0, 0, 'a'],
      [1, 0, 'b'],
    ]);
  });

  /** Everything a deletion can break, on every sheet. */
  function everything(sheet: SheetDocument) {
    return {
      sheets: sheet.sheets().map((summary, index) => ({
        name: summary.name,
        cells: sheet
          .cells(index, { top: 0, left: 0, bottom: 30, right: 10 })
          .filter((cell) => cell.input !== '')
          .map((cell) => [cell.row, cell.column, cell.input]),
        formats: sheet.conditionalFormats(index),
        validations: sheet.validations(index),
        charts: sheet.charts(index).map((chart) => chart.spec),
      })),
      names: sheet.definedNames(),
    };
  }

  /** A workbook whose other cells, rules, names and a chart point into rows 2 and 3 of the first sheet. */
  function fill(grid: SheetDocument): void {
    const first = grid.sheets()[0]!.name;
    grid.setInputs(0, [
      [0, 0, '1'],
      [1, 0, '2'],
      [2, 0, '3'],
      [3, 0, '4'],
      [4, 0, '5'],
      [5, 2, '=A2'],
      [6, 2, '=SOMMA(A2:A3)'],
      [7, 2, '=SOMMA(A3:A5)'],
      [8, 2, '=A5'],
    ]);
    grid.addSheet('Dati');
    grid.setInput(1, 0, 0, `=${first}!A2+${first}!A4`);
    grid.setDefinedName('Coppia', `=${first}!$A$2:$A$3`);
    grid.setConditionalFormats(0, [
      { ranges: [{ firstRow: 1, firstColumn: 0, lastRow: 3, lastColumn: 0 }], rules: [{ condition: { kind: 'expression', formula: 'A2>$A$3' } }] },
    ]);
    grid.setValidations(0, [
      { ranges: [{ firstRow: 2, firstColumn: 1, lastRow: 4, lastColumn: 1 }], kind: 'custom', formulas: ['B3>$A$2'] },
    ]);
    grid.addChart(
      1,
      { chartType: 'column', stacked: false, legend: true, series: [{ values: `${first}!$A$2:$A$3` }] },
      { row: 2, column: 3, offsetX: 0, offsetY: 0, width: 3_000_000, height: 2_000_000 },
    );
  }

  it('undoes a row deletion with the formulas, rules, names and charts elsewhere it broke', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    const b = await client(network, bea);
    fill(a.grid);
    await network.flush();
    const before = everything(a.sheet);
    a.grid.deleteRows(0, 1, 2);
    await network.flush();
    expect(a.sheet.input(0, 3, 2)).toBe('=#REF!');
    expect(a.sheet.input(1, 0, 0)).toContain('#REF!');
    const deleted = everything(a.sheet);
    expect(a.grid.undo()).toBe(true);
    await network.flush();
    expect(everything(a.sheet)).toEqual(before);
    expect(everything(b.sheet)).toEqual(before);
    expect(a.grid.redo()).toBe(true);
    await network.flush();
    expect(everything(b.sheet)).toEqual(deleted);
    expect(a.grid.undo()).toBe(true);
    await network.flush();
    expect(everything(b.sheet)).toEqual(before);
  });

  it('undoes a column deletion with the formulas it broke', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    a.grid.setInputs(0, [
      [0, 0, '1'],
      [0, 1, '2'],
      [0, 2, '3'],
      [0, 3, '4'],
      [3, 0, '=B1'],
      [3, 1, '=SOMMA(B1:C1)'],
      [3, 4, '=SOMMA(C1:D1)+A1'],
    ]);
    await network.flush();
    const before = everything(a.sheet);
    a.grid.deleteColumns(0, 1, 2);
    await network.flush();
    a.grid.undo();
    await network.flush();
    expect(everything(a.sheet)).toEqual(before);
  });

  it('undoes a deletion past what others did since, keeping a cell they wrote', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    const b = await client(network, bea);
    const cells: [number, number, string][] = [
      [0, 0, '1'],
      [1, 0, '2'],
      [2, 0, '3'],
      [3, 0, '4'],
      [4, 0, '5'],
      [5, 2, '=A2'],
      [6, 2, '=SOMMA(A2:A3)'],
      [7, 2, '=SOMMA(A3:A5)'],
      [8, 2, '=A5'],
    ];
    a.grid.setInputs(0, cells);
    await network.flush();
    a.grid.deleteRows(0, 1, 2);
    await network.flush();
    // For Bea the first two formulas are now on rows 4 and 5: a row goes between them, and she
    // writes over the third, now on row 7.
    b.grid.insertRows(0, 4, 1);
    b.grid.setInput(0, 6, 2, 'mio');
    await network.flush();
    a.grid.undo();
    await network.flush();
    const engine = await createSheet({ locale: 'it' });
    engine.setInputs(0, cells);
    engine.insertRows(0, 6, 1);
    engine.setInput(0, 8, 2, 'mio');
    expect(contents(a.sheet)).toEqual(contents(engine));
    expect(contents(b.sheet)).toEqual(contents(engine));
    engine.free();
  });

  it('moves a formula past rows inserted on a sheet renamed at the same time', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    const b = await client(network, bea);
    a.grid.addSheet('Dati');
    await network.flush();
    // Bea inserts a row on the sheet before she hears of its new name; Ada, who renamed it, then
    // writes a formula naming it before she hears of the row.
    a.grid.renameSheet(1, 'Nuovo');
    b.grid.insertRows(1, 0, 1);
    a.grid.setInput(0, 0, 0, '=Nuovo!A1+Nuovo!B3');
    await network.flush();
    expect(contents(a.sheet)).toEqual(contents(b.sheet));
    const engine = await createSheet({ locale: 'it' });
    engine.addSheet('Dati');
    engine.renameSheet(1, 'Nuovo');
    engine.setInput(0, 0, 0, '=Nuovo!A1+Nuovo!B3');
    engine.insertRows(1, 0, 1);
    expect(contents(a.sheet)).toEqual(contents(engine));
    expect(a.sheet.input(0, 0, 0)).toBe('=Nuovo!A2+Nuovo!B4');
    engine.free();
  });

  it('sends again what waited while it was cut off', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    const b = await client(network, bea);
    network.drop(a.session);
    a.grid.setInput(0, 2, 2, 'offline');
    b.grid.setInput(0, 0, 0, 'online');
    await network.flush();
    expect(a.binding.waiting).toBe(0);
    expect(contents(a.sheet)).toEqual(contents(b.sheet));
    expect(contents(b.sheet)).toEqual([
      [0, 0, 'online'],
      [2, 2, 'offline'],
    ]);
  });

  it('brings a late client to the same workbook from the base version', async () => {
    const base = await createSheet({ locale: 'it' });
    base.setInput(0, 0, 0, 'base');
    const bytes = base.save();
    base.free();
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network, ada, bytes);
    a.grid.setInput(0, 1, 0, 'later');
    await network.flush();
    const c = await client(network, bea, bytes);
    expect(contents(c.sheet)).toEqual(contents(a.sheet));
    expect(contents(c.sheet)).toEqual([
      [0, 0, 'base'],
      [1, 0, 'later'],
    ]);
    await expect(a.binding.whenIdle()).resolves.toBeUndefined();
  });

  it('tells its host when a save moved the base', async () => {
    const network = new CollabNetwork<SheetOperation>(sheetRoom(), { transform: transformSheetOperation });
    const a = await client(network);
    a.grid.setInput(0, 0, 0, '1');
    await network.flush();
    network.room.markSaved({ versionId: 'v2', sequence: a.binding.sequence });
    await network.flush();
    expect(network.room.isDirty).toBe(false);
    expect(a.session.welcome).toMatchObject({ baseVersionId: 'v2', baseSequence: a.binding.sequence });
    expect(network.room.snapshot().operations).toEqual([]);
  });
});

describe('a shared document aside', () => {
  it('gives each slot its own indexes', () => {
    expect(slotIndex(0, 0)).toBe(0);
    expect(slotIndex(0, 3)).toBe(3);
    expect(slotIndex(5, 3)).toBe(67);
    expect(slotIndex(64, 0)).toBe(64);
  });

  it('keeps two comments added at once', () => {
    const left = new Y.Doc();
    const right = new Y.Doc();
    const document: EditorDocument = {
      content: { type: 'doc', content: [] },
      media: [],
      aside: {
        report: {} as EditorDocument['aside']['report'],
        notes: [],
        headersFooters: { header: [], footer: [] },
        page: { widthTwips: 11906, heightTwips: 16838, marginsTwips: [1440, 1440, 1440, 1440] },
        comments: [],
      } as unknown as EditorDocument['aside'],
    };
    const first = new SharedDocumentAside(left, 0);
    first.seed(document);
    Y.applyUpdate(right, Y.encodeStateAsUpdate(left));
    const second = new SharedDocumentAside(right, 1);
    const comment = (author: string) => ({ author, date: '', blocks: [], resolved: false });
    const leftIndex = first.allocate('comment', 0);
    const rightIndex = second.allocate('comment', 0);
    expect(leftIndex).not.toBe(rightIndex);
    const base = first.readAside(document.aside);
    first.writeAside(base, { ...base, comments: [comment('Ada')] });
    second.writeAside(base, { ...base, comments: [comment('Bea')] });
    Y.applyUpdate(right, Y.encodeStateAsUpdate(left));
    Y.applyUpdate(left, Y.encodeStateAsUpdate(right));
    const authors = first.readAside(document.aside).comments!.map((each) => each.author).filter(Boolean);
    expect(authors.sort()).toEqual(['Ada', 'Bea']);
    expect(second.readAside(document.aside).comments).toEqual(first.readAside(document.aside).comments);
  });
});

describe('a shared presentation', () => {
  const box = (x: number): SlideShape => ({
    frame: { x, y: 0, width: 100, height: 100 },
    rotation: 0,
    geometry: { type: 'rectangle' },
    fill: null,
    line: null,
    content: { type: 'text', paragraphs: [] },
  }) as unknown as SlideShape;

  const deck = (): Presentation => ({
    widthEmu: 1000,
    heightEmu: 500,
    truncated: false,
    media: [],
    slides: [{ shapes: [box(0), box(10)] }, { shapes: [] }],
  });

  function pair() {
    const left = new Y.Doc();
    const right = new Y.Doc();
    left.on('update', (update: Uint8Array, origin: unknown) => {
      if (origin !== 'remote') Y.applyUpdate(right, update, 'remote');
    });
    right.on('update', (update: Uint8Array, origin: unknown) => {
      if (origin !== 'remote') Y.applyUpdate(left, update, 'remote');
    });
    const first = new SharedPresentation(left, 0);
    first.seed(deck());
    const second = new SharedPresentation(right, 1);
    return { first, second };
  }

  it('matches keys by object and by place', () => {
    const one = { a: 1 };
    const two = { a: 2 };
    const changed = { a: 3 };
    let counter = 0;
    const result = matchKeys([one, two], ['k1', 'k2'], [two, changed, { a: 4 }], () => `n${(counter += 1)}`);
    expect(result.keys).toEqual(['k2', 'k1', 'n1']);
    expect([...result.changed].sort()).toEqual(['k1', 'n1']);
  });

  it('changes a shared list of keys without losing one it has not seen', () => {
    const doc = new Y.Doc();
    const array = doc.getArray<string>('keys');
    array.insert(0, ['a', 'b', 'c', 'remote']);
    applyKeyChanges(array, ['a', 'b', 'c'], ['c', 'a', 'd']);
    expect(array.toArray()).toEqual(['c', 'a', 'd', 'remote']);
  });

  it('merges changes to different shapes and keeps slides added at once', () => {
    const { first, second } = pair();
    const mine = first.read();
    const theirs = second.read();
    expect(theirs.slides.length).toBe(2);
    first.write({ ...mine, slides: [{ ...mine.slides[0]!, shapes: [box(50), mine.slides[0]!.shapes![1]!] }, mine.slides[1]!, { shapes: [] }] });
    second.write({ ...theirs, slides: [{ ...theirs.slides[0]!, shapes: [theirs.slides[0]!.shapes![0]!, box(70)] }, theirs.slides[1]!, { shapes: [box(1)] }] });
    const left = first.read();
    const right = second.read();
    expect(left).toEqual(right);
    expect(left.slides.length).toBe(4);
    expect(left.slides[0]!.shapes!.map((shape) => shape.frame.x)).toEqual([50, 70]);
  });

  it('undoes only the changes of its own client', () => {
    const { first, second } = pair();
    const mine = first.read();
    first.write({ ...mine, slides: [mine.slides[0]!] });
    first.stopCapturing();
    const theirs = second.read();
    second.write({ ...theirs, slides: [{ shapes: [box(5)] }, ...theirs.slides] });
    expect(first.canUndo()).toBe(true);
    expect(first.undo()).toBe(true);
    expect(first.read().slides.length).toBe(3);
    expect(first.canRedo()).toBe(true);
    expect(first.redo()).toBe(true);
    expect(second.read().slides.length).toBe(2);
  });

  it('moves an image added under an index another client took', () => {
    const { first, second } = pair();
    const image = (byte: number) => ({ index: 0, contentType: 'image/png', bytes: new Uint8Array([byte]) });
    const base = first.read();
    second.write({ ...second.read(), media: [image(2)] });
    const written = first.write({
      ...base,
      media: [image(1)],
      slides: [{ shapes: [{ ...box(0), content: { type: 'picture', media: 0 } } as unknown as SlideShape] }],
    });
    expect(written.media[0]!.index).toBe(64);
    expect((written.slides[0]!.shapes![0]!.content as { media: number }).media).toBe(64);
    expect(first.read().media.map((item) => item.index)).toEqual([0, 64]);
  });
});

describe('a session', () => {
  it('reconnects and reports where it stands', async () => {
    const network = new CollabNetwork(textRoom());
    const session: CollabSession = network.session('text', ada);
    const statuses: string[] = [];
    session.on('status', (status) => statuses.push(status));
    await network.flush();
    network.drop(session);
    await network.flush();
    expect(statuses).toContain('disconnected');
    expect(session.currentStatus).toBe('connected');
    session.destroy();
    expect(session.currentStatus).toBe('closed');
  });
});
