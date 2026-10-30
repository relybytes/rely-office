// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

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
  isSharedDocumentKind,
  readJson,
  toBase64,
  type CollabKind,
  type SavedMessage,
  type SequencedOperation,
  type ServerControlMessage,
  type SubmittedOperation,
  type WelcomeMessage,
} from './protocol.js';

/** Where a collaboration session stands. */
export type CollabStatus = 'connecting' | 'connected' | 'disconnected' | 'closed';

/** Who a client is to the others: shown beside their cursor and in the list of who is here. */
export interface CollabUser {
  name: string;
  /** A CSS colour for their cursor and selection. */
  color: string;
}

/** The part of a WebSocket the session uses, so a host or a test can give its own. */
export interface CollabSocket {
  binaryType: string;
  readyState: number;
  send: (data: Uint8Array) => void;
  close: (code?: number, reason?: string) => void;
  onopen: ((event: unknown) => void) | null;
  onclose: ((event: unknown) => void) | null;
  onerror: ((event: unknown) => void) | null;
  onmessage: ((event: { data: unknown }) => void) | null;
}

export interface CollabSessionOptions {
  kind: CollabKind;
  /**
   * The address to connect to, asked again before every connection: a host hands out a ticket
   * that is good for one connection.
   */
  address: () => Promise<string>;
  user: CollabUser;
  /** The shared document; a new one by default. */
  doc?: Y.Doc;
  /** Opens a socket; the browser's WebSocket by default. */
  openSocket?: (address: string) => CollabSocket;
  /** The waits between attempts to reconnect, in milliseconds; the last one repeats. */
  reconnectDelays?: number[];
}

interface SessionEvents {
  status: CollabStatus;
  welcome: WelcomeMessage;
  ready: undefined;
  control: ServerControlMessage;
  operation: SequencedOperation;
}

type Listener<T> = (value: T) => void;

const OPEN = 1;

/** A random key for this page's session, kept across reconnections. */
function randomKey(): string {
  const bytes = new Uint8Array(12);
  globalThis.crypto.getRandomValues(bytes);
  return [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

/**
 * The browser side of a collaboration channel: it keeps a shared document in sync with the room,
 * relays presence, carries spreadsheet operations and reconnects on its own.
 */
export class CollabSession {
  readonly doc: Y.Doc;
  readonly awareness: Awareness;
  readonly kind: CollabKind;
  readonly sessionKey = randomKey();
  private status: CollabStatus = 'disconnected';
  private socket: CollabSocket | null = null;
  private welcomeMessage: WelcomeMessage | null = null;
  private lastSequenceValue = -1;
  private readonly listeners = new Map<keyof SessionEvents, Set<Listener<never>>>();
  private readonly pendingOperations: SequencedOperation[] = [];
  private readonly options: CollabSessionOptions;
  private attempt = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private closed = false;

  constructor(options: CollabSessionOptions) {
    this.options = options;
    this.kind = options.kind;
    this.doc = options.doc ?? new Y.Doc();
    this.awareness = new Awareness(this.doc);
    this.awareness.setLocalStateField('user', options.user);
    this.doc.on('update', (update: Uint8Array, origin: unknown) => {
      if (origin === this || !this.isOpen()) return;
      const encoder = encoding.createEncoder();
      encoding.writeVarUint(encoder, CollabFrame.Sync);
      sync.writeUpdate(encoder, update);
      this.send(encoding.toUint8Array(encoder));
    });
    this.awareness.on(
      'update',
      ({ added, updated, removed }: { added: number[]; updated: number[]; removed: number[] }, origin: unknown) => {
        if (origin === this || !this.isOpen()) return;
        const changed = [...added, ...updated, ...removed];
        const encoder = encoding.createEncoder();
        encoding.writeVarUint(encoder, CollabFrame.Awareness);
        encoding.writeVarUint8Array(encoder, encodeAwarenessUpdate(this.awareness, changed));
        this.send(encoding.toUint8Array(encoder));
      },
    );
  }

  /** Connect, and keep reconnecting until `destroy`. */
  connect(): void {
    if (this.closed || this.socket) return;
    this.setStatus('connecting');
    this.options
      .address()
      .then((address) => {
        if (this.closed) return;
        const socket = (this.options.openSocket ?? ((url: string) => new WebSocket(url) as unknown as CollabSocket))(address);
        socket.binaryType = 'arraybuffer';
        this.socket = socket;
        socket.onopen = () => this.onOpen();
        socket.onmessage = (event) => this.onMessage(event.data);
        socket.onclose = () => this.onClose(socket);
        socket.onerror = () => undefined;
      })
      .catch(() => this.scheduleReconnect());
  }

  /** Close for good: presence goes and no reconnection follows. */
  destroy(): void {
    this.closed = true;
    if (this.timer) clearTimeout(this.timer);
    removeAwarenessStates(this.awareness, [this.doc.clientID], 'local');
    this.socket?.close(1000, 'closed');
    this.socket = null;
    this.awareness.destroy();
    this.setStatus('closed');
  }

  get currentStatus(): CollabStatus {
    return this.status;
  }

  /** The last welcome, or null before the first. */
  get welcome(): WelcomeMessage | null {
    return this.welcomeMessage;
  }

  /** The last operation applied; the spreadsheet binding keeps it current. */
  get lastSequence(): number {
    return this.lastSequenceValue;
  }

  set lastSequence(sequence: number) {
    this.lastSequenceValue = sequence;
  }

  /** Listen to the session; returns a function that stops listening. */
  on<K extends keyof SessionEvents>(event: K, listener: Listener<SessionEvents[K]>): () => void {
    let set = this.listeners.get(event);
    if (!set) {
      set = new Set();
      this.listeners.set(event, set);
    }
    set.add(listener as Listener<never>);
    if (event === 'operation' && this.pendingOperations.length > 0) {
      for (const operation of this.pendingOperations.splice(0)) (listener as Listener<SequencedOperation>)(operation);
    }
    return () => {
      set.delete(listener as Listener<never>);
    };
  }

  /** Send a spreadsheet operation; false when the channel is down, and the binding sends it again on `ready`. */
  sendOperation(submitted: SubmittedOperation): boolean {
    if (!this.isOpen() || !this.welcomeMessage) return false;
    this.send(encodeJsonFrame(CollabFrame.Operation, submitted));
    return true;
  }

  /** Tell the room the shared document now holds the file. */
  markSeeded(): void {
    this.send(encodeJsonFrame(CollabFrame.Control, { type: 'seeded' }));
  }

  /**
   * Tell the room this client saved it as a version: with the state vector of the shared
   * document as it was written, or the last operation the version holds.
   */
  claimSaved(versionId: string, holds: { stateVector?: Uint8Array; sequence?: number }): void {
    const claim: SavedMessage = { type: 'saved', versionId };
    if (holds.stateVector) claim.stateVector = toBase64(holds.stateVector);
    if (holds.sequence !== undefined) claim.sequence = holds.sequence;
    this.send(encodeJsonFrame(CollabFrame.Control, claim));
  }

  private isOpen(): boolean {
    return this.socket !== null && this.socket.readyState === OPEN;
  }

  private send(frame: Uint8Array): void {
    if (this.isOpen()) this.socket!.send(frame);
  }

  private emit<K extends keyof SessionEvents>(event: K, value: SessionEvents[K]): void {
    const set = this.listeners.get(event);
    if (event === 'operation' && (!set || set.size === 0)) {
      this.pendingOperations.push(value as SequencedOperation);
      return;
    }
    for (const listener of set ?? []) (listener as Listener<SessionEvents[K]>)(value);
  }

  private setStatus(status: CollabStatus): void {
    if (this.status === status) return;
    this.status = status;
    this.emit('status', status);
  }

  private onOpen(): void {
    this.attempt = 0;
    this.send(
      encodeJsonFrame(CollabFrame.Control, {
        type: 'hello',
        sessionKey: this.sessionKey,
        lastSequence: this.lastSequenceValue,
      }),
    );
  }

  private onMessage(data: unknown): void {
    const bytes = data instanceof ArrayBuffer ? new Uint8Array(data) : data instanceof Uint8Array ? data : null;
    if (!bytes) return;
    const decoder = decoding.createDecoder(bytes);
    const kind = decoding.readVarUint(decoder);
    switch (kind) {
      case CollabFrame.Sync: {
        const encoder = encoding.createEncoder();
        encoding.writeVarUint(encoder, CollabFrame.Sync);
        sync.readSyncMessage(decoder, encoder, this.doc, this);
        if (encoding.length(encoder) > 1) this.send(encoding.toUint8Array(encoder));
        break;
      }
      case CollabFrame.Awareness:
        applyAwarenessUpdate(this.awareness, decoding.readVarUint8Array(decoder), this);
        break;
      case CollabFrame.Control:
        this.onControl(readJson(decoder) as ServerControlMessage | null);
        break;
      case CollabFrame.Operation: {
        const operation = readJson(decoder) as SequencedOperation | null;
        if (operation) this.emit('operation', operation);
        break;
      }
      default:
        break;
    }
  }

  private onControl(message: ServerControlMessage | null): void {
    if (!message) return;
    if (message.type === 'welcome') {
      this.welcomeMessage = message;
      this.setStatus('connected');
      this.emit('welcome', message);
      if (isSharedDocumentKind(this.kind)) {
        const encoder = encoding.createEncoder();
        encoding.writeVarUint(encoder, CollabFrame.Sync);
        sync.writeSyncStep1(encoder, this.doc);
        this.send(encoding.toUint8Array(encoder));
      }
      const presence = encoding.createEncoder();
      encoding.writeVarUint(presence, CollabFrame.Awareness);
      encoding.writeVarUint8Array(presence, encodeAwarenessUpdate(this.awareness, [this.doc.clientID]));
      this.send(encoding.toUint8Array(presence));
      return;
    }
    if (message.type === 'ready') {
      this.emit('ready', undefined);
      return;
    }
    if (message.type === 'saved' && this.welcomeMessage) {
      this.welcomeMessage = { ...this.welcomeMessage, baseVersionId: message.versionId, dirty: message.dirty ?? false };
      if (message.sequence !== undefined) this.welcomeMessage.baseSequence = message.sequence;
    }
    if (message.type === 'seeded' && this.welcomeMessage) {
      this.welcomeMessage = { ...this.welcomeMessage, seeded: true };
    }
    this.emit('control', message);
  }

  private onClose(socket: CollabSocket): void {
    if (this.socket !== socket) return;
    this.socket = null;
    const others = [...this.awareness.getStates().keys()].filter((client) => client !== this.doc.clientID);
    if (others.length > 0) removeAwarenessStates(this.awareness, others, this);
    if (this.closed) return;
    this.setStatus('disconnected');
    this.scheduleReconnect();
  }

  private scheduleReconnect(): void {
    if (this.closed) return;
    this.socket = null;
    this.setStatus('disconnected');
    const delays = this.options.reconnectDelays ?? [500, 1_000, 2_000, 5_000, 10_000];
    const delay = delays[Math.min(this.attempt, delays.length - 1)] ?? 5_000;
    this.attempt += 1;
    this.timer = setTimeout(() => {
      this.timer = null;
      this.connect();
    }, delay);
  }
}

/** Who else is here, from the session's presence: one entry per client, without this one. */
export function collabPeers(awareness: Awareness): (CollabUser & { client: number })[] {
  const peers: (CollabUser & { client: number })[] = [];
  for (const [client, state] of awareness.getStates()) {
    if (client === awareness.clientID) continue;
    const user = (state as { user?: CollabUser }).user;
    if (user && typeof user.name === 'string') peers.push({ client, name: user.name, color: String(user.color) });
  }
  return peers;
}
