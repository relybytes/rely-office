// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';
import * as Y from 'yjs';

/**
 * The frames of a collaboration channel. Each starts with its kind as a variable-length integer:
 * the Yjs sync and awareness messages of `y-protocols`, a control message as JSON text, and a
 * spreadsheet operation as JSON text.
 */
export const CollabFrame = {
  Sync: 0,
  Awareness: 1,
  Control: 2,
  Operation: 3,
} as const;

/** The most clients a room gives a slot of its own; past it, slots repeat. */
export const MAX_SLOTS = 64;

/** What a room edits: the three editors share a Yjs document, a spreadsheet an operation log. */
export type CollabKind = 'document' | 'presentation' | 'text' | 'spreadsheet';

/** Whether a kind is kept as a Yjs document rather than as a log of operations. */
export function isSharedDocumentKind(kind: CollabKind): boolean {
  return kind !== 'spreadsheet';
}

/** What a client says first, so a reconnection is recognised and sent only what it missed. */
export interface HelloMessage {
  type: 'hello';
  /** Random and kept for the life of the editor page, across reconnections. */
  sessionKey: string;
  /** The last operation the client applied; -1 before any. */
  lastSequence: number;
}

/** What the room answers a hello with. */
export interface WelcomeMessage {
  type: 'welcome';
  kind: CollabKind;
  /** A number no other client of the room holds, for indexes two clients could pick together. */
  slot: number;
  canWrite: boolean;
  /** The version of the file the room's state starts from. */
  baseVersionId: string;
  /** The last operation the base version holds. */
  baseSequence: number;
  /** The last operation the room has. */
  sequence: number;
  /** Whether the shared document already holds the file. */
  seeded: boolean;
  /** Whether this client is to put the file into the shared document. */
  seed: boolean;
  /**
   * Whether the client must open the base version again: it applied operations the base version
   * does not hold, or missed some the room no longer has.
   */
  reload: boolean;
  /** Whether the room holds changes no version of the file has. */
  dirty: boolean;
}

/** The end of the operations a welcome is followed by. */
export interface ReadyMessage {
  type: 'ready';
}

/** This client is now to put the file into the shared document, since the one before left. */
export interface SeedMessage {
  type: 'seed';
}

/** The shared document holds the file. */
export interface SeededMessage {
  type: 'seeded';
}

/** A client saved the room as a version of the file. */
export interface SavedMessage {
  type: 'saved';
  versionId: string;
  /** For a spreadsheet, the last operation the version holds. */
  sequence?: number;
  /** For a shared document, its state vector when it was written, in base64. */
  stateVector?: string;
  dirty?: boolean;
  by?: string;
}

/** An operation of this client cannot be placed and the client must open the file again. */
export interface ResyncMessage {
  type: 'resync';
}

/** Something the room refused, by code. */
export interface ErrorMessage {
  type: 'error';
  code: 'READ_ONLY' | 'INVALID_FRAME' | 'NOT_SEEDER' | 'SAVE_REFUSED';
}

export type ClientControlMessage = HelloMessage | SeededMessage | SavedMessage;
export type ServerControlMessage =
  | WelcomeMessage
  | ReadyMessage
  | SeedMessage
  | SeededMessage
  | SavedMessage
  | ResyncMessage
  | ErrorMessage;

/** A spreadsheet operation as a client sends it. */
export interface SubmittedOperation<Operation = unknown> {
  /** Unique within the client's session key. */
  id: string;
  /** The last operation the client had applied when it made this one. */
  base: number;
  operation: Operation;
}

/** A spreadsheet operation as the room orders it, already moved past what came before it. */
export interface SequencedOperation<Operation = unknown> {
  sequence: number;
  /** The session key of the client that made it. */
  author: string;
  id: string;
  operation: Operation;
}

/** A frame of JSON text. */
export function encodeJsonFrame(kind: number, value: unknown): Uint8Array {
  const encoder = encoding.createEncoder();
  encoding.writeVarUint(encoder, kind);
  encoding.writeVarString(encoder, JSON.stringify(value));
  return encoding.toUint8Array(encoder);
}

/** The JSON value of a frame whose kind was read; null when it is not JSON. */
export function readJson(decoder: decoding.Decoder): unknown {
  try {
    return JSON.parse(decoding.readVarString(decoder)) as unknown;
  } catch {
    return null;
  }
}

/** A Uint8Array as base64, in the browser and in Node. */
export function toBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let index = 0; index < bytes.length; index += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(index, index + 0x8000));
  }
  return btoa(binary);
}

/** Base64 as bytes. */
export function fromBase64(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
  return bytes;
}

/** Whether two encoded state vectors name the same updates, whatever order their clients are in. */
export function sameStateVector(left: Uint8Array, right: Uint8Array): boolean {
  const first = Y.decodeStateVector(left);
  const second = Y.decodeStateVector(right);
  if (first.size !== second.size) return false;
  for (const [client, clock] of first) {
    if (second.get(client) !== clock) return false;
  }
  return true;
}
