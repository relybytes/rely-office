// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { Editor } from '@tiptap/core';
import { createRelyDocumentExtensions, openDocument, saveDocument, saveDocumentAs } from '../src/index.js';
import { readDocumentTree, writeDocx, writeHtml, writeMarkdown, type Block } from '../src/runtime/index.js';
import { loadWasm } from './wasm.js';

const PNG = Uint8Array.from([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
  0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
  0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
  0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
  0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
]);

const BLOCKS: Block[] = [
  { type: 'heading', level: 2, content: [{ type: 'text', text: 'Offerta' }] },
  {
    type: 'paragraph',
    align: 'center',
    content: [
      { type: 'text', text: 'Totale ', marks: [{ type: 'bold' }] },
      { type: 'text', text: '1.200 euro', marks: [{ type: 'textStyle', color: '#C00000' }] },
      { type: 'image', media: 0, width: 24, height: 24 },
    ],
  },
  { type: 'listItem', ordered: true, level: 0, listId: 1, content: [{ type: 'text', text: 'Analisi' }] },
  { type: 'listItem', ordered: true, level: 1, listId: 1, content: [{ type: 'text', text: 'Interviste' }] },
  {
    type: 'table',
    rows: [
      { header: true, cells: [{ colSpan: 1, rowSpan: 1, blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Voce' }] }] }, { colSpan: 1, rowSpan: 1, blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Importo' }] }] }] },
      { header: false, cells: [{ colSpan: 2, rowSpan: 1, blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Incluso' }] }] }] },
    ],
  },
];

beforeAll(loadWasm);

test('a docx opened in the editor and saved comes back with the same blocks and image', () => {
  const original = writeDocx({ blocks: BLOCKS, media: [PNG] });
  const { document, report } = openDocument(original);
  expect(report).toEqual({ marksDropped: 0, imagesLeftOut: 0 });

  const editor = new Editor({ extensions: createRelyDocumentExtensions(), content: document.content });
  const edited = { ...document, content: editor.getJSON() };
  editor.destroy();

  const reread = readDocumentTree(saveDocument(edited));
  expect(reread.blocks).toEqual(readDocumentTree(original).blocks);
  expect(reread.media[0].bytes).toEqual(PNG);
});

test('a Markdown file and an HTML page open in the editor and are saved in their own format', () => {
  const markdown = new TextEncoder().encode('# Titolo\n\nUn **testo** con [link](https://relybytes.com).\n\n- uno\n- due\n');
  const opened = openDocument(markdown, { fileName: 'nota.md' });
  const saved = new TextDecoder().decode(saveDocumentAs(opened.document, 'markdown'));
  expect(saved).toBe('# Titolo\n\nUn **testo** con [link](https://relybytes.com).\n\n- uno\n- due\n');

  const page = new TextDecoder().decode(saveDocumentAs(opened.document, 'html', 'Nota'));
  expect(page).toContain('<title>Nota</title>');
  expect(page).toContain('<h1>Titolo</h1>');
  const again = openDocument(new TextEncoder().encode(page), { fileName: 'nota.html' });
  expect(readDocumentTree(saveDocumentAs(again.document, 'docx')).blocks).toEqual(
    readDocumentTree(saveDocumentAs(opened.document, 'docx')).blocks,
  );
  expect(writeMarkdown({ blocks: BLOCKS, media: [PNG] })).toContain('| Voce | Importo |');
  expect(writeHtml({ blocks: BLOCKS, media: [PNG] }, 'Offerta')).toContain('colspan="2"');
});
