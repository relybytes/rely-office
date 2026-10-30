// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import { CollabRoom, type CollabRoomOptions, type CollabRoomState } from '../src/collab/room.js';
import { CollabSession, type CollabSocket, type CollabUser } from '../src/collab/session.js';
import type { CollabKind } from '../src/collab/protocol.js';

/**
 * A room and its clients in memory. Nothing is delivered until `flush`, so two clients can act
 * before either hears of the other, as on a real network.
 */
export class CollabNetwork<Operation = unknown> {
  readonly room: CollabRoom<Operation>;
  private readonly queue: (() => void)[] = [];
  private peers = 0;
  readonly sockets = new Map<CollabSession, FakeSocket>();

  constructor(state: CollabRoomState<Operation>, options: CollabRoomOptions<Operation> = {}) {
    this.room = new CollabRoom(state, options);
  }

  /** A session of this room, connected; `canWrite` false for a reader. */
  session(kind: CollabKind, user: CollabUser, canWrite = true): CollabSession {
    const session = new CollabSession({
      kind,
      user,
      address: () => Promise.resolve('memory'),
      openSocket: () => {
        const socket = new FakeSocket(this as unknown as Wire, `peer-${(this.peers += 1)}`, canWrite);
        this.sockets.set(session, socket);
        return socket;
      },
      reconnectDelays: [0],
    });
    session.connect();
    return session;
  }

  /** Cut a session's connection, as a network drop does. */
  drop(session: CollabSession): void {
    this.sockets.get(session)?.drop();
  }

  enqueue(task: () => void): void {
    this.queue.push(task);
  }

  /** Deliver everything in flight, and what that sends in turn. */
  async flush(): Promise<void> {
    for (let round = 0; round < 1_000; round += 1) {
      await new Promise((resolve) => setTimeout(resolve, 0));
      if (this.queue.length === 0) {
        await new Promise((resolve) => setTimeout(resolve, 0));
        if (this.queue.length === 0) return;
      }
      while (this.queue.length > 0) this.queue.shift()!();
    }
    throw new Error('the network did not settle');
  }
}

/** What a fake socket needs of its network. */
interface Wire {
  room: Pick<CollabRoom<never>, 'join' | 'receive' | 'leave'>;
  enqueue: (task: () => void) => void;
}

class FakeSocket implements CollabSocket {
  binaryType = 'arraybuffer';
  readyState = 0;
  onopen: ((event: unknown) => void) | null = null;
  onclose: ((event: unknown) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;

  constructor(
    private readonly network: Wire,
    private readonly id: string,
    canWrite: boolean,
  ) {
    network.enqueue(() => {
      network.room.join({
        id,
        canWrite,
        send: (frame) =>
          network.enqueue(() => {
            if (this.readyState === 1) this.onmessage?.({ data: frame.slice().buffer });
          }),
      });
      this.readyState = 1;
      this.onopen?.({});
    });
  }

  send(data: Uint8Array): void {
    const frame = data.slice();
    this.network.enqueue(() => {
      if (this.readyState === 1) this.network.room.receive(this.id, frame);
    });
  }

  close(): void {
    this.drop();
  }

  drop(): void {
    if (this.readyState === 3) return;
    this.readyState = 3;
    this.network.room.leave(this.id);
    this.network.enqueue(() => this.onclose?.({}));
  }
}
