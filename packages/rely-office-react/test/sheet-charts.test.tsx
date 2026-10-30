// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { StrictMode } from 'react';
import { SheetGrid } from '../src/index.js';
import { createSheet, type SheetDocument } from '../src/runtime/index.js';
import { dataUrl } from '../src/sheet/picture-urls.js';
import { loadWasm } from './wasm.js';

beforeAll(loadWasm);

let sheet: SheetDocument;

beforeEach(async () => {
  sheet = await createSheet({ locale: 'en' });
  sheet.setInputs(0, [
    [0, 0, 'Month'],
    [0, 1, 'Sales'],
    [1, 0, 'Jan'],
    [1, 1, '10'],
    [2, 0, 'Feb'],
    [2, 1, '=B2*3'],
  ]);
  const chart = sheet.chartForRange(0, { top: 0, left: 0, bottom: 2, right: 1 }, 'column')!;
  sheet.addChart(0, { ...chart, title: 'Quarter' }, { row: 0, column: 4, width: 3_810_000, height: 2_286_000 });
});

afterEach(() => {
  sheet.free();
  vi.unstubAllGlobals();
});

/** The SVG an `<img>` shows, read back from its `data:` address. */
function svgOf(image: HTMLImageElement): string {
  const [, encoded = ''] = image.src.split(',');
  return new TextDecoder().decode(Uint8Array.from(atob(encoded), (character) => character.charCodeAt(0)));
}

describe('charts in the grid', () => {
  test('a chart is drawn in its frame, named, and redrawn when the cells it plots change', async () => {
    vi.stubGlobal('URL', { ...URL, createObjectURL: undefined, revokeObjectURL: undefined });
    render(<SheetGrid sheet={sheet} />);
    const grid = screen.getByRole('grid');

    const chart = await waitFor(() => within(grid).getByRole('img', { name: 'Chart Quarter' }) as HTMLImageElement);
    await waitFor(() => expect(chart.src).toMatch(/^data:image\/svg\+xml;base64,/));
    const before = svgOf(chart);
    expect(before).toContain('<svg');
    expect(before).toContain('>Jan<');
    expect(before).toContain('>30<');
    expect(before).not.toContain('>80<');
    // The frame the editor moves it by sits over the picture.
    expect(within(grid).getByRole('button', { name: 'Chart Quarter' })).toBeTruthy();

    fireEvent.mouseDown(within(grid).getByRole('gridcell', { name: 'B2' }));
    fireEvent.keyDown(grid, { key: '5' });
    const editor = screen.getByRole('textbox', { name: 'B2' });
    fireEvent.change(editor, { target: { value: '25' } });
    fireEvent.keyDown(editor, { key: 'Enter' });

    await waitFor(() => expect(svgOf(within(grid).getByRole('img', { name: 'Chart Quarter' }) as HTMLImageElement)).not.toBe(before));
    const after = svgOf(within(grid).getByRole('img', { name: 'Chart Quarter' }) as HTMLImageElement);
    // 25 and 75 now: the value axis reaches 80.
    expect(after).toContain('>80<');
  });

  test('a read-only grid draws the chart too, with no frame to move it', async () => {
    vi.stubGlobal('URL', { ...URL, createObjectURL: undefined, revokeObjectURL: undefined });
    render(<SheetGrid sheet={sheet} editable={false} />);
    const grid = screen.getByRole('grid');

    const chart = await waitFor(() => within(grid).getByRole('img', { name: 'Chart Quarter' }) as HTMLImageElement);
    await waitFor(() => expect(svgOf(chart)).toContain('<rect'));
    expect(within(grid).queryByRole('button', { name: 'Chart Quarter' })).toBeNull();
  });

  test('in strict mode the picture is never left on an address already released', async () => {
    let next = 0;
    const revoked = new Set<string>();
    vi.stubGlobal('URL', {
      ...URL,
      createObjectURL: () => `blob:chart-${(next += 1)}`,
      revokeObjectURL: (url: string) => revoked.add(url),
    });
    const { unmount } = render(
      <StrictMode>
        <SheetGrid sheet={sheet} />
      </StrictMode>,
    );

    const chart = await waitFor(() => screen.getByRole('img', { name: 'Chart Quarter' }) as HTMLImageElement);
    await waitFor(() => expect(chart.getAttribute('src')).toMatch(/^blob:chart-/));
    expect(revoked.has(chart.getAttribute('src')!)).toBe(false);

    unmount();
    expect(revoked.has(chart.getAttribute('src')!)).toBe(true);
  });

  test('a data address carries the bytes as they are', () => {
    const bytes = new TextEncoder().encode('<svg>è</svg>');
    const url = dataUrl(bytes, 'image/svg+xml');
    expect(url.startsWith('data:image/svg+xml;base64,')).toBe(true);
    expect(new TextDecoder().decode(Uint8Array.from(atob(url.split(',')[1]!), (character) => character.charCodeAt(0)))).toBe('<svg>è</svg>');
  });
});
