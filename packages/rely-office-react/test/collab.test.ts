// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import * as Y from 'yjs';
import { SharedDocumentAside, slotIndex } from '../src/collab/document-collab.js';
import { encodeJsonFrame, CollabFrame, sameStateVector, fromBase64, toBase64 } from '../src/collab/protocol.js';
import { CollabRoom, claimedStateVector, type CollabRoomState } from '../src/collab/room.js';
import { collabPeers, type CollabSession } from '../src/collab/session.js';
import { SheetCollaboration } from '../src/collab/sheet-collab.js';
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
      [2, 0, '=A6'],
      [5, 0, 'below'],
      [5, 1, 'beside'],
    ]);
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
