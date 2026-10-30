// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { render, screen, waitFor } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { DocumentEditor, openDocument, saveDocument, saveDocumentAs } from '../src/index.js';
import { readDocumentTree, writeDocx, writeHtml, writeOdt, type Block } from '../src/runtime/index.js';
import { loadWasm } from './wasm.js';

const CARLITO = new Uint8Array(
  readFileSync(join(resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', '..'), 'fonts', 'Carlito-Regular.ttf')),
);

const BLOCKS: Block[] = [
  { type: 'paragraph', content: [{ type: 'text', text: 'Titolo', marks: [{ type: 'textStyle', fontFamily: 'Garamond' }] }] },
  { type: 'paragraph', content: [{ type: 'text', text: 'Corpo' }] },
];

beforeAll(loadWasm);

describe('the fonts of a document', () => {
  const input = {
    blocks: BLOCKS,
    fonts: [{ name: 'Carlito', generic: 'swiss' as const, faces: [{ style: 'regular' as const, file: 0 }] }],
    fontFiles: [CARLITO],
    defaultFont: { family: 'Carlito', sizeHalfPoints: 24 },
  };

  it('travel through a docx with the faces it embeds', () => {
    const tree = readDocumentTree(writeDocx(input));
    expect(tree.defaultFont).toEqual({ family: 'Carlito', sizeHalfPoints: 24 });
    const carlito = tree.fonts?.find((font) => font.name === 'Carlito');
    expect(carlito?.faces).toEqual([{ style: 'regular', file: 0 }]);
    expect(tree.fontFiles?.[0]).toEqual(CARLITO);
    expect(tree.fonts?.find((font) => font.name === 'Garamond')?.generic).toBe('roman');
  });

  it('drop a face whose file is not a font', () => {
    const tree = readDocumentTree(writeDocx({ ...input, fontFiles: [new Uint8Array([1, 2, 3])] }));
    expect(tree.fontFiles).toBeUndefined();
    expect(tree.fonts?.find((font) => font.name === 'Carlito')?.faces).toBeUndefined();
  });

  it('travel through an odt and into an HTML page', () => {
    const tree = readDocumentTree(writeOdt(input));
    expect(tree.fontFiles?.[0]).toEqual(CARLITO);
    const html = writeHtml(input, 'Pagina');
    expect(html).toContain("font-family:'Garamond', 'Liberation Serif', serif;");
    expect(html).toContain("font-family:'Carlito', sans-serif;font-size:12pt;");
  });

  it('ride along the editor and are saved again', () => {
    const { document } = openDocument(writeDocx(input));
    expect(document.aside.fontFiles?.length).toBe(1);
    const again = readDocumentTree(saveDocument(document));
    expect(again.fontFiles?.[0]).toEqual(CARLITO);
    expect(again.defaultFont?.family).toBe('Carlito');
    const odt = readDocumentTree(saveDocumentAs(document, 'odt'));
    expect(odt.fontFiles?.[0]).toEqual(CARLITO);
  });

  it('set the editor text and lead its font picker', async () => {
    const { document } = openDocument(writeDocx(input));
    const { container } = render(<DocumentEditor document={document} />);
    const root = container.querySelector('.rely-document-editor') as HTMLElement;
    expect(root.style.getPropertyValue('--rely-document-font')).toBe("'Carlito', sans-serif");
    expect(root.style.getPropertyValue('--rely-document-size')).toBe('12pt');
    const picker = (await screen.findByLabelText('Font')) as HTMLSelectElement;
    const options = [...picker.options].map((option) => option.value);
    expect(options[0]).toBe('');
    expect(options.slice(1, 3).sort()).toEqual(['Carlito', 'Garamond']);
    await waitFor(() => expect(container.querySelector('span[style*="Garamond"]')?.getAttribute('style')).toContain(
      '"Garamond", "Liberation Serif", serif',
    ));
  });
});
