// @vitest-environment node
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import {
  RelyOfficeError,
  convertToDocx,
  createSheet,
  detectFormat,
  extractText,
  openSheet,
  readDocumentTree,
  readPresentation,
  readSheets,
  writeDocx,
  writePptx,
  type Block,
  type Slide,
} from '../src/runtime/index.js';
import { loadWasm } from './wasm.js';

const PNG = Uint8Array.from([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
  0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
  0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
  0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
  0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
]);

const BLOCKS: Block[] = [
  { type: 'heading', level: 1, content: [{ type: 'text', text: 'Verbale' }] },
  {
    type: 'paragraph',
    content: [
      { type: 'text', text: 'Presenti: ' },
      { type: 'text', text: 'tutti', marks: [{ type: 'bold' }] },
      { type: 'image', media: 0, alt: 'logo', width: 32, height: 32 },
    ],
  },
  { type: 'listItem', ordered: true, level: 0, listId: 1, content: [{ type: 'text', text: 'uno' }] },
];

beforeAll(loadWasm);

describe('documents', () => {
  test('a tree written as a docx reads back to the same blocks and image', () => {
    const docx = writeDocx({ blocks: BLOCKS, media: [PNG] });

    expect(detectFormat(docx)).toBe('docx');
    expect(extractText(docx).text).toContain('Verbale');
    const tree = readDocumentTree(docx);
    expect(tree.blocks.map((block) => block.type)).toEqual(['heading', 'paragraph', 'listItem']);
    expect(tree.media).toHaveLength(1);
    expect(tree.media[0].contentType).toBe('image/png');
    expect(tree.media[0].bytes).toEqual(PNG);
    expect(readDocumentTree(convertToDocx(docx)).blocks).toHaveLength(3);
  });

  test('a file that is not a document fails with the code of rely-office', () => {
    expect(() => readDocumentTree(new TextEncoder().encode('not a document'))).toThrow(
      RelyOfficeError,
    );
    try {
      readDocumentTree(new Uint8Array([1, 2, 3]));
    } catch (error) {
      expect((error as RelyOfficeError).code).toBe('UNSUPPORTED_FORMAT');
    }
  });
});

describe('presentations', () => {
  test('slides written as a pptx read back with their text and picture', () => {
    const slide: Slide = {
      shapes: [
        {
          frame: { x: 914400, y: 914400, width: 4572000, height: 914400 },
          rotation: 0,
          geometry: { type: 'rectangle' },
          fill: { type: 'solid', colour: 'DDEEFF' },
          line: null,
          content: {
            type: 'text',
            anchor: 'top',
            insetsEmu: [91440, 45720, 91440, 45720],
            wrap: true,
            paragraphs: [
              {
                level: 0,
                align: 'left',
                bullet: null,
                marginLeftEmu: 0,
                indentEmu: 0,
                lineSpacingPercent: 100,
                spaceBefore: 0,
                spaceAfter: 0,
                runs: [
                  {
                    text: 'Ordine del giorno',
                    size: 2400,
                    bold: true,
                    italic: false,
                    underline: false,
                    strike: false,
                    colour: '000000',
                    family: 'Calibri',
                    lineBreak: false,
                  },
                ],
              },
            ],
          },
        },
        {
          frame: { x: 914400, y: 2743200, width: 914400, height: 914400 },
          rotation: 0,
          geometry: { type: 'rectangle' },
          fill: null,
          line: null,
          content: { type: 'picture', media: 0 },
        },
      ],
    };
    const pptx = writePptx({ widthEmu: 12192000, heightEmu: 6858000, slides: [slide], media: [PNG] });

    expect(detectFormat(pptx)).toBe('pptx');
    const presentation = readPresentation(pptx);
    expect(presentation.slides).toHaveLength(1);
    const own = (presentation.slides[0].shapes ?? []).filter((shape) => !shape.inherited);
    expect(own.map((shape) => shape.content.type)).toEqual(['text', 'picture']);
    expect(presentation.media[0].bytes).toEqual(PNG);
  });
});

describe('sheets', () => {
  test('a workbook recalculates, saves and opens again', async () => {
    const sheet = await createSheet({ locale: 'it' });
    sheet.setInputs(0, [
      [0, 0, '10'],
      [0, 1, '=A1*3'],
    ]);
    expect(sheet.cells(0, { top: 0, left: 0, bottom: 0, right: 1 })[1].display).toBe('30');
    expect(() => sheet.removeSheet(0)).toThrow(RelyOfficeError);

    const xlsx = sheet.save();
    sheet.free();
    expect(readSheets(xlsx, 'conti.xlsx').sheets[0].rows[0]).toEqual(['10', '30']);
    const reopened = await openSheet(xlsx, 'conti.xlsx');
    expect(reopened.input(0, 0, 1)).toBe('=A1*3');
    reopened.free();
  });
});
