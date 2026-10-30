// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// Editing together, without React: the protocol, the room a server runs, the session a page
// opens, and the bindings of the three kinds of file to a session.

export * from './protocol.js';
export { CollabRoom, claimedStateVector, type CollabPeer, type CollabRoomEvents, type CollabRoomOptions, type CollabRoomState, type OperationTransform } from './room.js';
export { CollabSession, collabPeers, type CollabSessionOptions, type CollabSocket, type CollabStatus, type CollabUser } from './session.js';
export {
  SHEET_MUTATORS,
  isSheetMutator,
  shiftIndex,
  shiftSpan,
  transformSheetOperation,
  type SheetMutator,
  type SheetOperation,
} from './sheet-operations.js';
export {
  MAX_SHARED_UNDO,
  SheetCollaboration,
  type RemoteSelection,
  type SharedSelection,
  type SheetCollaborationOptions,
} from './sheet-collab.js';
export { DOCUMENT_CONTENT_FIELD, SharedDocumentAside, slotIndex } from './document-collab.js';
export { SharedPresentation, applyKeyChanges, matchKeys } from './slides-collab.js';
