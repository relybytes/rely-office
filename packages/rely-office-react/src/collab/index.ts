// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

// Editing together, without React: the protocol, the room a server runs, the session a page
// opens, and the bindings of the three kinds of file to a session.

export * from './protocol.js';
export { CollabRoom, claimedStateVector, type CollabPeer, type CollabRoomEvents, type CollabRoomOptions, type CollabRoomState, type OperationTransform } from './room.js';
export { CollabSession, collabPeers, type CollabSessionOptions, type CollabSocket, type CollabStatus, type CollabUser } from './session.js';
export {
  removeFormulaSheet,
  renameFormulaSheet,
  shiftFormula,
  type ReferenceShift,
  type ReferenceTarget,
} from './formula-references.js';
export {
  SHEET_MUTATORS,
  SHEET_NAMED_MUTATORS,
  isSheetMutator,
  shiftIndex,
  shiftSpan,
  transformSheetOperation,
  type SheetMutator,
  type RestoredBand,
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
