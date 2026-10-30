// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// The TipTap side of a shared document, in its own entry point: the collaboration extensions are
// optional peers, needed only by a host that edits together.

import type { AnyExtension } from '@tiptap/core';
import Collaboration from '@tiptap/extension-collaboration';
import CollaborationCaret from '@tiptap/extension-collaboration-caret';
import { DOCUMENT_CONTENT_FIELD } from './document-collab.js';
import type { CollabSession, CollabUser } from './session.js';

/**
 * The extensions that bind a document editor to a session's shared document and show where the
 * others are writing; pass them as `collaboration.extensions` of `DocumentEditor`.
 */
export function documentCollaborationExtensions(session: CollabSession, user: CollabUser): AnyExtension[] {
  return [
    Collaboration.configure({ document: session.doc, field: DOCUMENT_CONTENT_FIELD }),
    CollaborationCaret.configure({ provider: { awareness: session.awareness }, user }),
  ];
}
