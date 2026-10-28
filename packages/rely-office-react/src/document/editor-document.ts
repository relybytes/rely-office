// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type { JSONContent } from '@tiptap/core';
import type {
  Comment,
  EmbeddedObject,
  HeadersFooters,
  ImportReport,
  MediaItem,
  Note,
  PageSize,
  Section,
} from '../model.js';

/**
 * What a document holds that the editor does not show as text: the notes and comments its
 * references and marks point at, the page, the headers and footers, the sections, the embedded
 * objects. It rides along unchanged and is written back with the edited text.
 */
export interface DocumentAside {
  report: ImportReport;
  notes: Note[];
  headersFooters: HeadersFooters;
  page: PageSize;
  sections?: Section[];
  comments?: Comment[];
  objects?: EmbeddedObject[];
}

/**
 * A document as the editor holds it: the ProseMirror content, every image by index (an image
 * node names its item with the `media` attribute) and what rides along.
 */
export interface EditorDocument {
  content: JSONContent;
  media: MediaItem[];
  aside: DocumentAside;
}

/** What the mapping onto the editor had to leave out, counted by kind. */
export interface EditorMappingReport {
  /** Marks a code run cannot carry, since code excludes every other mark. */
  marksDropped: number;
  /** Images whose `media` names no item of the tree. */
  imagesLeftOut: number;
}

/** How a tree becomes editor content. */
export interface TreeToEditorOptions {
  /** The `src` an image is shown with; by default a `data:` URL of its bytes. */
  imageSource?: (item: MediaItem) => string;
}
