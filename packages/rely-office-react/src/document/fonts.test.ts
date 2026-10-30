// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import {
  fontGeneric,
  fontStack,
  fontSubstitute,
  isFontAvailable,
  loadDocumentFonts,
  pickerFamilies,
  stackFamily,
  usedFamilies,
} from './fonts.js';

describe('font stacks', () => {
  it('end a family with its bundled substitute and generic family', () => {
    expect(fontStack('Calibri')).toBe("'Calibri', 'Carlito', sans-serif");
    expect(fontStack('Aptos Display')).toBe("'Aptos Display', 'Carlito', sans-serif");
    expect(fontStack('Cambria')).toBe("'Cambria', 'Caladea', serif");
    expect(fontStack('Consolas')).toBe("'Consolas', 'Liberation Mono', monospace");
    expect(fontStack('Mystery', 'roman')).toBe("'Mystery', 'Liberation Serif', serif");
    expect(fontStack('Mystery', 'script')).toBe("'Mystery', 'Carlito', cursive");
    expect(fontStack('Mystery', 'decorative')).toBe("'Mystery', 'Carlito', fantasy");
    expect(fontStack('Carlito')).toBe("'Carlito', sans-serif");
    expect(fontStack(" ';")).toBe("'Carlito', sans-serif");
  });

  it('pick a substitute by metrics, then by kind', () => {
    expect(fontSubstitute('Segoe UI')).toBe('Liberation Sans');
    expect(fontSubstitute('Source Code Pro')).toBe('Liberation Mono');
    expect(fontSubstitute('Noto Serif')).toBe('Liberation Serif');
    expect(fontSubstitute('Mystery', 'swiss')).toBe('Liberation Sans');
    expect(fontSubstitute('Mystery', 'modern')).toBe('Liberation Mono');
    expect(fontSubstitute('Mystery')).toBe('Carlito');
    expect(fontGeneric('Georgia')).toBe('roman');
    expect(fontGeneric('Mystery')).toBe('auto');
  });

  it('read the first family of a stack', () => {
    expect(stackFamily("'Calibri', 'Carlito', sans-serif")).toBe('Calibri');
    expect(stackFamily('"Times New Roman", serif')).toBe('Times New Roman');
    expect(stackFamily('')).toBeNull();
    expect(stackFamily(undefined)).toBeNull();
  });
});

describe('font pickers', () => {
  it('list the document families first, each once', () => {
    expect(pickerFamilies(['Aptos', 'calibri', ' '], ['Calibri', 'Arial'])).toEqual(['Aptos', 'calibri', 'Arial']);
  });

  it('collect the families the runs of editor content set', () => {
    const content = {
      type: 'doc',
      content: [
        { type: 'paragraph', content: [{ type: 'text', text: 'a', marks: [{ type: 'textStyle', attrs: { fontFamily: 'Aptos' } }] }] },
        {
          type: 'paragraph',
          content: [
            { type: 'text', text: 'b', marks: [{ type: 'textStyle', attrs: { fontFamily: "'aptos', sans-serif" } }, { type: 'bold' }] },
            { type: 'text', text: 'c', marks: [{ type: 'textStyle', attrs: { color: '#ff0000' } }] },
          ],
        },
      ],
    };
    expect(usedFamilies(content)).toEqual(['Aptos']);
  });

  it('take a family as available where nothing can be measured', () => {
    expect(isFontAvailable('Calibri')).toBe(true);
  });
});

describe('embedded faces', () => {
  it('load into the page and leave it on release', async () => {
    const added: unknown[] = [];
    const removed: unknown[] = [];
    class FakeFace {
      constructor(
        readonly family: string,
        readonly source: ArrayBuffer,
        readonly descriptors: { weight: string; style: string },
      ) {}
      load(): Promise<this> {
        return this.family === 'Broken' ? Promise.reject(new Error('bad')) : Promise.resolve(this);
      }
    }
    const fonts = { add: (face: unknown) => added.push(face), delete: (face: unknown) => removed.push(face) };
    Object.defineProperty(document, 'fonts', { value: fonts, configurable: true });
    vi.stubGlobal('FontFace', FakeFace);
    const loaded = await loadDocumentFonts(
      [
        { name: 'Aptos', faces: [{ style: 'boldItalic', file: 0 }, { style: 'regular', file: 9 }] },
        { name: 'Broken', faces: [{ style: 'regular', file: 0 }] },
        { name: 'Plain' },
      ],
      [new Uint8Array([0, 1, 0, 0])],
    );
    expect(loaded.families).toEqual(['Aptos']);
    expect(added).toHaveLength(1);
    expect((added[0] as FakeFace).descriptors).toEqual({ weight: '700', style: 'italic' });
    loaded.release();
    expect(removed).toEqual(added);
    vi.unstubAllGlobals();
    Reflect.deleteProperty(document, 'fonts');
  });

  it('load nothing where the page has no FontFace', async () => {
    const loaded = await loadDocumentFonts([{ name: 'Aptos', faces: [{ style: 'regular', file: 0 }] }], [new Uint8Array(4)]);
    expect(loaded.families).toEqual([]);
    loaded.release();
  });
});
