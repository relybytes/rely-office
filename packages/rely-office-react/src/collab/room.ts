// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';
import {
  Awareness,
  applyAwarenessUpdate,
  encodeAwarenessUpdate,
  removeAwarenessStates,
} from 'y-protocols/awareness';
import * as sync from 'y-protocols/sync';
import * as Y from 'yjs';
import {
  CollabFrame,
  encodeJsonFrame,
  fromBase64,
  isSharedDocumentKind,
  MAX_SLOTS,
  readJson,
  sameStateVector,
  type CollabKind,
  type ClientControlMessage,
  type SavedMessage,
  type SequencedOperation,
  type ServerControlMessage,
  type SubmittedOperation,
  type WelcomeMessage,
} from './protocol.js';

/**
 * An operation moved past one that the room ordered before it, or null when nothing of it is
 * left to apply (its cells were deleted). The same function runs on every client.
 */
export type OperationTransform<Operation> = (operation: Operation, earlier: Operation) => Operation | null;

/** What a room is made of, as its host stores it between connections. */
export interface CollabRoomState<Operation = unknown> {
  kind: CollabKind;
  baseVersionId: string;
  /** The last operation the base version holds. */
  baseSequence: number;
  /** The last operation of the room. */
  sequence: number;
  seeded: boolean;
  /** Whether the room holds changes the base version does not. */
  dirty: boolean;
  /** The shared document as one Yjs update. */
  document?: Uint8Array;
  /** The operations after the base version, in order. */
  operations: SequencedOperation<Operation>[];
}

/** One connection to a room. */
export interface CollabPeer {
  id: string;
  canWrite: boolean;
  send: (frame: Uint8Array) => void;
}

/** What a room tells its host, to store and to check. */
export interface CollabRoomEvents<Operation> {
  /** A change to the shared document, to append to what the host stores. */
  update?: (update: Uint8Array) => void;
  /** An operation the room ordered, to append to what the host stores. */
  operation?: (operation: SequencedOperation<Operation>) => void;
  /** The header changed: base version, sequence, seeded or dirty. */
  change?: () => void;
  /**
   * A client says it saved the room as a version; the host checks the version is the file's
   * current one and was written by that client's user before calling `markSaved`.
   */
  saveClaim?: (peerId: string, claim: SavedMessage) => void;
}

export interface CollabRoomOptions<Operation> {
  transform?: OperationTransform<Operation>;
  events?: CollabRoomEvents<Operation>;
  /** How many operations older than the base version the room keeps to place late ones. */
  historyLimit?: number;
}

interface PeerState {
  peer: CollabPeer;
  sessionKey: string | null;
  slot: number;
  awarenessClients: Set<number>;
}

/**
 * The server side of a collaboration channel, without the transport: the host feeds it frames
 * and gives it a way to send, and it keeps the shared document or the operation log, orders the
 * operations, relays presence and decides who puts the file into a new room.
 */
export class CollabRoom<Operation = unknown> {
  readonly doc = new Y.Doc();
  readonly awareness: Awareness;
  private readonly peers = new Map<string, PeerState>();
  private readonly transform: OperationTransform<Operation> | undefined;
  private readonly events: CollabRoomEvents<Operation>;
  private readonly historyLimit: number;
  private readonly kind: CollabKind;
  private baseVersionId: string;
  private baseSequence: number;
  private sequence: number;
  private seeded: boolean;
  private dirty: boolean;
  private log: SequencedOperation<Operation>[];
  private seederId: string | null = null;

  constructor(state: CollabRoomState<Operation>, options: CollabRoomOptions<Operation> = {}) {
    this.kind = state.kind;
    this.baseVersionId = state.baseVersionId;
    this.baseSequence = state.baseSequence;
    this.sequence = state.sequence;
    this.seeded = isSharedDocumentKind(state.kind) ? state.seeded : true;
    this.dirty = state.dirty;
    this.log = [...state.operations];
    this.transform = options.transform;
    this.events = options.events ?? {};
    this.historyLimit = options.historyLimit ?? 2_000;
    if (state.document) Y.applyUpdate(this.doc, state.document, 'load');
    this.awareness = new Awareness(this.doc);
    this.awareness.setLocalState(null);
    this.doc.on('update', (update: Uint8Array, origin: unknown) => this.onDocumentUpdate(update, origin));
    this.awareness.on(
      'update',
      ({ added, updated, removed }: { added: number[]; updated: number[]; removed: number[] }, origin: unknown) => {
        const source = typeof origin === 'string' ? this.peers.get(origin) : undefined;
        if (source) {
          for (const client of added) source.awarenessClients.add(client);
          for (const client of removed) source.awarenessClients.delete(client);
        }
        const changed = [...added, ...updated, ...removed];
        if (changed.length === 0) return;
        const encoder = encoding.createEncoder();
        encoding.writeVarUint(encoder, CollabFrame.Awareness);
        encoding.writeVarUint8Array(encoder, encodeAwarenessUpdate(this.awareness, changed));
        this.broadcast(encoding.toUint8Array(encoder), typeof origin === 'string' ? origin : null);
      },
    );
  }

  /** How many clients are connected. */
  get size(): number {
    return this.peers.size;
  }

  /** Whether the room holds changes no version of the file has. */
  get isDirty(): boolean {
    return this.dirty;
  }

  /** A new connection; it is welcomed once it says hello. */
  join(peer: CollabPeer): void {
    this.peers.set(peer.id, { peer, sessionKey: null, slot: -1, awarenessClients: new Set() });
  }

  /** A connection closed: its presence goes, and another client seeds if it was seeding. */
  leave(peerId: string): void {
    const state = this.peers.get(peerId);
    if (!state) return;
    this.peers.delete(peerId);
    if (state.awarenessClients.size > 0) removeAwarenessStates(this.awareness, [...state.awarenessClients], null);
    if (this.seederId === peerId) {
      this.seederId = null;
      this.chooseSeeder();
    }
  }

  /** A frame from a connection. */
  receive(peerId: string, frame: Uint8Array): void {
    const state = this.peers.get(peerId);
    if (!state) return;
    const decoder = decoding.createDecoder(frame);
    let kind: number;
    try {
      kind = decoding.readVarUint(decoder);
    } catch {
      this.sendControl(state, { type: 'error', code: 'INVALID_FRAME' });
      return;
    }
    if (state.sessionKey === null && kind !== CollabFrame.Control) return;
    try {
      switch (kind) {
        case CollabFrame.Sync:
          this.receiveSync(state, decoder);
          break;
        case CollabFrame.Awareness:
          applyAwarenessUpdate(this.awareness, decoding.readVarUint8Array(decoder), peerId);
          break;
        case CollabFrame.Control:
          this.receiveControl(state, readJson(decoder) as ClientControlMessage | null);
          break;
        case CollabFrame.Operation:
          this.receiveOperation(state, readJson(decoder) as SubmittedOperation<Operation> | null);
          break;
        default:
          this.sendControl(state, { type: 'error', code: 'INVALID_FRAME' });
      }
    } catch {
      this.sendControl(state, { type: 'error', code: 'INVALID_FRAME' });
    }
  }

  /**
   * The room was saved as a version: what the version holds is no longer a change, and every
   * client is told so it saves over that version next time.
   */
  markSaved(claim: { versionId: string; sequence?: number; stateVector?: Uint8Array; by?: string }): void {
    this.baseVersionId = claim.versionId;
    if (isSharedDocumentKind(this.kind)) {
      this.dirty = !(claim.stateVector && sameStateVector(claim.stateVector, Y.encodeStateVector(this.doc)));
    } else if (claim.sequence !== undefined) {
      this.baseSequence = Math.min(Math.max(this.baseSequence, claim.sequence), this.sequence);
      this.dirty = this.sequence > this.baseSequence;
      this.pruneLog();
    }
    this.events.change?.();
    const saved: SavedMessage = { type: 'saved', versionId: claim.versionId, dirty: this.dirty };
    if (claim.sequence !== undefined) saved.sequence = this.baseSequence;
    if (claim.by !== undefined) saved.by = claim.by;
    for (const state of this.peers.values()) {
      if (state.sessionKey !== null) this.sendControl(state, saved);
    }
  }

  /** The room as its host stores it. */
  snapshot(): CollabRoomState<Operation> {
    return {
      kind: this.kind,
      baseVersionId: this.baseVersionId,
      baseSequence: this.baseSequence,
      sequence: this.sequence,
      seeded: this.seeded,
      dirty: this.dirty,
      ...(isSharedDocumentKind(this.kind) ? { document: Y.encodeStateAsUpdate(this.doc) } : {}),
      operations: this.log.filter((entry) => entry.sequence > this.baseSequence),
    };
  }

  /** Close the room's document; the room is not used after this. */
  destroy(): void {
    this.awareness.destroy();
    this.doc.destroy();
  }

  private onDocumentUpdate(update: Uint8Array, origin: unknown): void {
    if (origin === 'load') return;
    const encoder = encoding.createEncoder();
    encoding.writeVarUint(encoder, CollabFrame.Sync);
    sync.writeUpdate(encoder, update);
    this.broadcast(encoding.toUint8Array(encoder), typeof origin === 'string' ? origin : null);
    this.events.update?.(update);
    if (this.seeded && !this.dirty) {
      this.dirty = true;
      this.events.change?.();
    }
  }

  private receiveSync(state: PeerState, decoder: decoding.Decoder): void {
    if (!isSharedDocumentKind(this.kind)) return;
    const type = decoding.readVarUint(decoder);
    if (type === sync.messageYjsSyncStep1) {
      const encoder = encoding.createEncoder();
      encoding.writeVarUint(encoder, CollabFrame.Sync);
      sync.writeSyncStep2(encoder, this.doc, decoding.readVarUint8Array(decoder));
      state.peer.send(encoding.toUint8Array(encoder));
      return;
    }
    if (!state.peer.canWrite) {
      this.sendControl(state, { type: 'error', code: 'READ_ONLY' });
      return;
    }
    if (type === sync.messageYjsSyncStep2) sync.readSyncStep2(decoder, this.doc, state.peer.id);
    else if (type === sync.messageYjsUpdate) sync.readUpdate(decoder, this.doc, state.peer.id);
  }

  private receiveControl(state: PeerState, message: ClientControlMessage | null): void {
    if (!message || typeof message !== 'object') return;
    if (message.type === 'hello') {
      this.welcome(state, message.sessionKey, message.lastSequence);
      return;
    }
    if (state.sessionKey === null) return;
    if (message.type === 'seeded') {
      if (this.seederId !== state.peer.id) {
        this.sendControl(state, { type: 'error', code: 'NOT_SEEDER' });
        return;
      }
      this.seederId = null;
      this.seeded = true;
      this.events.change?.();
      for (const other of this.peers.values()) {
        if (other !== state && other.sessionKey !== null) this.sendControl(other, { type: 'seeded' });
      }
      return;
    }
    if (message.type === 'saved') {
      if (!state.peer.canWrite) {
        this.sendControl(state, { type: 'error', code: 'READ_ONLY' });
        return;
      }
      this.events.saveClaim?.(state.peer.id, message);
    }
  }

  private welcome(state: PeerState, sessionKey: unknown, lastSequence: unknown): void {
    if (typeof sessionKey !== 'string' || sessionKey.length === 0 || sessionKey.length > 100) {
      this.sendControl(state, { type: 'error', code: 'INVALID_FRAME' });
      return;
    }
    const first = state.sessionKey === null;
    state.sessionKey = sessionKey;
    if (first) state.slot = this.freeSlot(state);
    const shared = isSharedDocumentKind(this.kind);
    let seed = false;
    if (shared && !this.seeded && state.peer.canWrite && this.seederId === null) {
      this.seederId = state.peer.id;
      seed = true;
    }
    const last = typeof lastSequence === 'number' && Number.isInteger(lastSequence) ? lastSequence : -1;
    let from = last >= 0 ? last : this.baseSequence;
    let reload = false;
    const oldest = this.log.length > 0 ? this.log[0]!.sequence - 1 : this.sequence;
    if (!shared && (from > this.sequence || from < Math.min(oldest, this.baseSequence))) {
      reload = last >= 0;
      from = this.baseSequence;
    }
    const welcome: WelcomeMessage = {
      type: 'welcome',
      kind: this.kind,
      slot: state.slot,
      canWrite: state.peer.canWrite,
      baseVersionId: this.baseVersionId,
      baseSequence: this.baseSequence,
      sequence: this.sequence,
      seeded: this.seeded,
      seed,
      reload,
      dirty: this.dirty,
    };
    this.sendControl(state, welcome);
    if (shared) {
      const encoder = encoding.createEncoder();
      encoding.writeVarUint(encoder, CollabFrame.Sync);
      sync.writeSyncStep1(encoder, this.doc);
      state.peer.send(encoding.toUint8Array(encoder));
      const clients = [...this.awareness.getStates().keys()];
      if (clients.length > 0) {
        const presence = encoding.createEncoder();
        encoding.writeVarUint(presence, CollabFrame.Awareness);
        encoding.writeVarUint8Array(presence, encodeAwarenessUpdate(this.awareness, clients));
        state.peer.send(encoding.toUint8Array(presence));
      }
    } else {
      for (const entry of this.log) {
        if (entry.sequence > from) state.peer.send(encodeJsonFrame(CollabFrame.Operation, entry));
      }
      const clients = [...this.awareness.getStates().keys()];
      if (clients.length > 0) {
        const presence = encoding.createEncoder();
        encoding.writeVarUint(presence, CollabFrame.Awareness);
        encoding.writeVarUint8Array(presence, encodeAwarenessUpdate(this.awareness, clients));
        state.peer.send(encoding.toUint8Array(presence));
      }
    }
    this.sendControl(state, { type: 'ready' });
  }

  private receiveOperation(state: PeerState, submitted: SubmittedOperation<Operation> | null): void {
    if (isSharedDocumentKind(this.kind) || state.sessionKey === null) return;
    if (!state.peer.canWrite) {
      this.sendControl(state, { type: 'error', code: 'READ_ONLY' });
      return;
    }
    if (
      !submitted ||
      typeof submitted.id !== 'string' ||
      submitted.id.length > 100 ||
      typeof submitted.base !== 'number' ||
      !Number.isInteger(submitted.base) ||
      submitted.base > this.sequence
    ) {
      this.sendControl(state, { type: 'error', code: 'INVALID_FRAME' });
      return;
    }
    const author = state.sessionKey;
    const known = this.log.find((entry) => entry.author === author && entry.id === submitted.id);
    if (known) {
      state.peer.send(encodeJsonFrame(CollabFrame.Operation, known));
      return;
    }
    const oldest = this.log.length > 0 ? this.log[0]!.sequence - 1 : this.sequence;
    if (submitted.base < oldest) {
      this.sendControl(state, { type: 'resync' });
      return;
    }
    let operation: Operation | null = submitted.operation;
    for (const entry of this.log) {
      if (operation === null) break;
      if (entry.sequence <= submitted.base || entry.author === author || entry.operation === null) continue;
      operation = this.transform ? this.transform(operation, entry.operation) : operation;
    }
    this.sequence += 1;
    const sequenced: SequencedOperation<Operation> = {
      sequence: this.sequence,
      author,
      id: submitted.id,
      operation: operation as Operation,
    };
    this.log.push(sequenced);
    this.pruneLog();
    this.events.operation?.(sequenced);
    if (operation !== null) this.dirty = true;
    this.events.change?.();
    this.broadcast(encodeJsonFrame(CollabFrame.Operation, sequenced), null);
  }

  private pruneLog(): void {
    const keepFrom = this.log.findIndex((entry) => entry.sequence > this.baseSequence);
    const older = keepFrom === -1 ? this.log.length : keepFrom;
    if (older > this.historyLimit) this.log = this.log.slice(older - this.historyLimit);
  }

  private chooseSeeder(): void {
    if (this.seeded || !isSharedDocumentKind(this.kind)) return;
    for (const state of this.peers.values()) {
      if (state.sessionKey !== null && state.peer.canWrite) {
        this.seederId = state.peer.id;
        this.sendControl(state, { type: 'seed' });
        return;
      }
    }
  }

  private freeSlot(state: PeerState): number {
    const taken = new Set([...this.peers.values()].filter((other) => other !== state).map((other) => other.slot));
    for (let slot = 0; slot < MAX_SLOTS; slot += 1) {
      if (!taken.has(slot)) return slot;
    }
    return this.peers.size % MAX_SLOTS;
  }

  private sendControl(state: PeerState, message: ServerControlMessage): void {
    state.peer.send(encodeJsonFrame(CollabFrame.Control, message));
  }

  private broadcast(frame: Uint8Array, except: string | null): void {
    for (const state of this.peers.values()) {
      if (state.peer.id !== except && state.sessionKey !== null) state.peer.send(frame);
    }
  }
}

/** The state vector a client claims to have saved, decoded, or undefined when it sent none. */
export function claimedStateVector(claim: SavedMessage): Uint8Array | undefined {
  if (typeof claim.stateVector !== 'string') return undefined;
  try {
    return fromBase64(claim.stateVector);
  } catch {
    return undefined;
  }
}
