// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { Editor, type JSONContent } from '@tiptap/core';
import type { DocumentTree, MediaItem } from '../model.js';
import { editorToDocumentTree } from './editor-to-tree.js';
import { createRelyDocumentExtensions } from './extensions.js';
import { documentTreeToEditor } from './tree-to-editor.js';

const PNG: MediaItem = { index: 0, contentType: 'image/png', bytes: new Uint8Array([0x89, 0x50, 0x4e, 0x47]) };
const JPEG: MediaItem = { index: 1, contentType: 'image/jpeg', bytes: new Uint8Array([0xff, 0xd8, 0xff]) };
const UNUSED: MediaItem = { index: 2, contentType: 'image/png', bytes: new Uint8Array([0x89, 0x50]) };

const EMPTY_REPORT: DocumentTree['report'] = {
  truncated: false,
  textBoxes: 0,
  footnotes: 1,
  endnotes: 0,
  comments: 1,
  headersAndFooters: 1,
  fieldsKeptAsText: 0,
  charts: 0,
  smartArt: 0,
  equations: 1,
  shapes: 0,
  embeddedObjects: 0,
  unsupportedLinks: 0,
  oversizedImages: 0,
  imagesOverLimit: 0,
  missingImages: 0,
  trackedInsertions: 0,
  trackedDeletions: 0,
  hiddenRuns: 0,
};

const PAGE: DocumentTree['page'] = {
  widthTwips: 11906,
  heightTwips: 16838,
  marginsTwips: [1440, 1440, 1440, 1440],
};
const LANDSCAPE: DocumentTree['page'] = { ...PAGE, widthTwips: 16838, heightTwips: 11906 };
/** The headers of the last section, which are the document's. */
const HEADERS: DocumentTree['headersFooters'] = {
  header: [{ type: 'paragraph', content: [{ type: 'image', media: 0 }] }],
  footer: [],
};

/** A tree that uses every block, inline and mark the tree has. */
function richTree(): DocumentTree {
  return {
    blocks: [
      { type: 'heading', level: 1, align: 'center', content: [{ type: 'text', text: 'Contratto' }] },
      {
        type: 'paragraph',
        align: 'justify',
        content: [
          { type: 'text', text: 'Grassetto', marks: [{ type: 'bold' }, { type: 'italic' }] },
          { type: 'text', text: ' sottolineato', marks: [{ type: 'underline' }, { type: 'strike' }] },
          { type: 'text', text: ' x', marks: [{ type: 'superscript' }] },
          { type: 'text', text: '2', marks: [{ type: 'subscript' }] },
          {
            type: 'text',
            text: ' rosso',
            marks: [{ type: 'textStyle', fontFamily: 'Arial', fontSize: '14pt', color: '#C00000' }],
          },
          { type: 'text', text: ' evidenziato', marks: [{ type: 'highlight', color: '#FFFF00' }] },
          { type: 'text', text: ' sito', marks: [{ type: 'link', href: 'https://relybytes.com' }] },
          { type: 'text', text: ' commentato', marks: [{ type: 'comment', comment: 0 }] },
          { type: 'commentAnchor', comment: 0 },
          { type: 'text', text: ' aggiunto', marks: [{ type: 'insertion', author: 'Anna', date: '2026-10-01T10:00:00Z' }] },
          { type: 'text', text: ' tolto', marks: [{ type: 'deletion', author: 'Marco' }] },
          { type: 'noteReference', note: 0 },
          { type: 'hardBreak' },
          { type: 'math', latex: '\\frac{a}{b}', display: false },
          { type: 'image', media: 0, alt: 'logo', width: 120, height: 40 },
          { type: 'text', text: 'codice', marks: [{ type: 'code' }] },
        ],
      },
      { type: 'listItem', ordered: false, level: 0, listId: 3, content: [{ type: 'text', text: 'uno' }] },
      { type: 'listItem', ordered: true, level: 1, listId: 3, content: [{ type: 'text', text: 'uno.uno' }] },
      { type: 'listItem', ordered: false, level: 0, listId: 3, content: [{ type: 'text', text: 'due' }] },
      { type: 'listItem', ordered: true, level: 0, listId: 4, content: [{ type: 'text', text: 'altra lista' }] },
      { type: 'blockquote', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'citazione' }] }] },
      { type: 'codeBlock', text: 'let a = 1;\nlet b = 2;' },
      { type: 'horizontalRule' },
      {
        type: 'table',
        rows: [
          {
            header: true,
            cells: [
              { colSpan: 2, rowSpan: 1, blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Voce' }] }] },
            ],
          },
          {
            header: false,
            cells: [
              { colSpan: 1, rowSpan: 1, blocks: [{ type: 'paragraph', content: [{ type: 'image', media: 1 }] }] },
              {
                colSpan: 1,
                rowSpan: 1,
                blocks: [
                  { type: 'listItem', ordered: false, level: 0, listId: 9, content: [{ type: 'text', text: 'in cella' }] },
                ],
              },
            ],
          },
        ],
      },
      { type: 'paragraph', content: [] },
      { type: 'paragraph', content: [{ type: 'text', text: 'Allegato' }] },
    ],
    media: [PNG, JPEG, UNUSED],
    report: EMPTY_REPORT,
    notes: [{ kind: 'footnote', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'nota' }] }] }],
    headersFooters: HEADERS,
    page: LANDSCAPE,
    sections: [
      { endBlock: 11, page: PAGE, headersFooters: { header: [], footer: [] }, continuous: false },
      { endBlock: 12, page: LANDSCAPE, headersFooters: HEADERS, continuous: false },
    ],
    comments: [
      { author: 'Anna', date: '2026-10-01T10:00:00Z', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Rivedere' }] }], resolved: false },
    ],
  };
}

function throughEditor(content: JSONContent): JSONContent {
  const editor = new Editor({ extensions: createRelyDocumentExtensions(), content });
  try {
    return editor.getJSON();
  } finally {
    editor.destroy();
  }
}

describe('the document tree in the editor', () => {
  test('every block, inline and mark comes back as it went in, the unused image dropped', () => {
    const tree = richTree();
    const { document, report } = documentTreeToEditor(tree);
    expect(report).toEqual({ marksDropped: 0, imagesLeftOut: 0 });

    const back = editorToDocumentTree({ ...document, content: throughEditor(document.content) });

    expect(back.blocks).toEqual(tree.blocks);
    expect(back.media).toEqual([PNG, JPEG]);
    expect(back.notes).toEqual(tree.notes);
    expect(back.comments).toEqual(tree.comments);
    expect(back.headersFooters).toEqual(tree.headersFooters);
    expect(back.sections).toEqual(tree.sections);
    expect(back.page).toEqual(LANDSCAPE);
    expect(back.report).toBe(tree.report);
  });

  test('a section whose break was deleted merges into the next one', () => {
    const { document } = documentTreeToEditor(richTree());
    const content = {
      ...document.content,
      content: document.content.content!.filter((node) => node.type !== 'sectionBreak'),
    };

    const back = editorToDocumentTree({ ...document, content });

    expect(back.sections).toBeUndefined();
    expect(back.page).toEqual(LANDSCAPE);
  });

  test('a code run keeps only its code mark, and an image with no media is counted', () => {
    const tree: DocumentTree = {
      ...richTree(),
      sections: undefined,
      blocks: [
        {
          type: 'paragraph',
          content: [
            { type: 'text', text: 'x', marks: [{ type: 'code' }, { type: 'bold' }, { type: 'link', href: 'https://a.it' }] },
            { type: 'image', media: 7 },
          ],
        },
      ],
    };

    const { document, report } = documentTreeToEditor(tree);

    expect(report).toEqual({ marksDropped: 2, imagesLeftOut: 1 });
    expect(editorToDocumentTree(document).blocks).toEqual([
      { type: 'paragraph', content: [{ type: 'text', text: 'x', marks: [{ type: 'code' }] }] },
    ]);
  });

  test('an empty tree is one empty paragraph, and comes back as one', () => {
    const { document } = documentTreeToEditor({ ...richTree(), blocks: [], sections: undefined });

    expect(document.content).toEqual({ type: 'doc', content: [{ type: 'paragraph' }] });
    expect(editorToDocumentTree(document).blocks).toEqual([{ type: 'paragraph', content: [] }]);
  });

  test('a list level that skips one goes a level down, and a second paragraph joins its item', () => {
    const tree: DocumentTree = {
      ...richTree(),
      sections: undefined,
      blocks: [
        { type: 'listItem', ordered: false, level: 2, listId: 1, content: [{ type: 'text', text: 'a' }] },
        { type: 'listItem', ordered: false, level: 3, listId: 1, content: [{ type: 'text', text: 'b' }] },
      ],
    };
    const { document } = documentTreeToEditor(tree);
    const list = document.content.content![0];
    list.content![0].content!.splice(1, 0, { type: 'paragraph', content: [{ type: 'text', text: 'segue' }] });

    expect(editorToDocumentTree(document).blocks).toEqual([
      {
        type: 'listItem',
        ordered: false,
        level: 0,
        listId: 1,
        content: [{ type: 'text', text: 'a' }, { type: 'hardBreak' }, { type: 'text', text: 'segue' }],
      },
      { type: 'listItem', ordered: false, level: 1, listId: 1, content: [{ type: 'text', text: 'b' }] },
    ]);
  });

  test('a list with no id, or a second list with a taken one, gets a new id', () => {
    const tree = editorToDocumentTree({
      ...documentTreeToEditor({ ...richTree(), sections: undefined, blocks: [] }).document,
      content: {
        type: 'doc',
        content: [
          { type: 'bulletList', attrs: { listId: 5 }, content: [{ type: 'listItem', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'a' }] }] }] },
          { type: 'paragraph', content: [{ type: 'text', text: 'tra' }] },
          { type: 'bulletList', attrs: { listId: 5 }, content: [{ type: 'listItem', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'b' }] }] }] },
          { type: 'orderedList', content: [{ type: 'listItem', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'c' }] }] }] },
        ],
      },
    });

    expect(tree.blocks.filter((block) => block.type === 'listItem').map((block) => block.type === 'listItem' && block.listId)).toEqual([5, 6, 7]);
  });

  test('a link to something other than the web or mail, and empty styles, are dropped', () => {
    const tree = editorToDocumentTree({
      ...documentTreeToEditor({ ...richTree(), sections: undefined, blocks: [] }).document,
      content: {
        type: 'doc',
        content: [
          {
            type: 'paragraph',
            content: [
              { type: 'text', text: 'a', marks: [{ type: 'link', attrs: { href: 'javascript:alert(1)' } }] },
              { type: 'text', text: 'b', marks: [{ type: 'textStyle', attrs: { fontFamily: null, fontSize: null, color: null } }] },
              { type: 'text', text: 'c', marks: [{ type: 'unknownMark' }] },
            ],
          },
          { type: 'taskList', content: [{ type: 'taskItem', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'compito' }] }] }] },
          { type: 'image', attrs: { media: 0, width: 'auto' } },
        ],
      },
    });

    expect(tree.blocks).toEqual([
      { type: 'paragraph', content: [{ type: 'text', text: 'a' }, { type: 'text', text: 'b' }, { type: 'text', text: 'c' }] },
      { type: 'paragraph', content: [{ type: 'text', text: 'compito' }] },
      { type: 'paragraph', content: [{ type: 'image', media: 0 }] },
    ]);
  });
});
