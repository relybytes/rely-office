// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { createRef } from 'react';
import { ITALIAN_SHEET_GRID_LABELS, SheetGrid, type SheetGridHandle } from '../src/index.js';
import { createSheet, type SheetDocument } from '../src/runtime/index.js';
import { loadWasm } from './wasm.js';

beforeAll(loadWasm);

let sheet: SheetDocument;

beforeEach(async () => {
  sheet = await createSheet({ locale: 'it' });
  sheet.setInputs(0, [
    [0, 0, '10'],
    [1, 0, '20'],
    [2, 0, '=SOMMA(A1:A2)'],
    [0, 1, 'Totale'],
  ]);
});

afterEach(() => sheet.free());

function mount(props: Partial<Parameters<typeof SheetGrid>[0]> = {}) {
  const ref = createRef<SheetGridHandle>();
  const onChange = vi.fn();
  render(<SheetGrid ref={ref} sheet={sheet} onChange={onChange} {...props} />);
  const grid = screen.getByRole('grid');
  const cell = (name: string) => within(grid).getByRole('gridcell', { name });
  return { ref, onChange, grid, cell };
}

describe('SheetGrid', () => {
  test('draws the values, a formula computed, and the formula in the bar', () => {
    const { cell } = mount();

    expect(cell('A3').textContent).toBe('30');
    expect(cell('B1').textContent).toBe('Totale');
    fireEvent.mouseDown(cell('A3'));
    expect((screen.getByLabelText('Contents') as HTMLInputElement).value).toBe('=SOMMA(A1:A2)');
    expect((screen.getByLabelText('Cell') as HTMLInputElement).value).toBe('A3');
  });

  test('typing into a cell and pressing Enter writes it, recalculates and moves down', () => {
    const { cell, grid, onChange } = mount();
    fireEvent.mouseDown(cell('A1'));

    fireEvent.keyDown(grid, { key: '5' });
    const editor = screen.getByRole('textbox', { name: 'A1' }) as HTMLInputElement;
    fireEvent.change(editor, { target: { value: '50' } });
    fireEvent.keyDown(editor, { key: 'Enter' });

    expect(cell('A3').textContent).toBe('70');
    expect(sheet.input(0, 0, 0)).toBe('50');
    expect((screen.getByLabelText('Cell') as HTMLInputElement).value).toBe('A2');
    expect(onChange).toHaveBeenCalled();
  });

  test('a formula rely-office refuses is shown and the cell is left as it was', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('C1'));
    fireEvent.keyDown(grid, { key: 'F2' });
    const editor = screen.getByRole('textbox', { name: 'C1' });
    fireEvent.change(editor, { target: { value: '=SOMMA(' } });
    fireEvent.keyDown(editor, { key: 'Enter' });

    expect(screen.getByRole('alert')).toBeTruthy();
    expect(sheet.input(0, 0, 2)).toBe('');
    fireEvent.keyDown(screen.getByRole('textbox', { name: 'C1' }), { key: 'Escape' });
    expect(screen.queryByRole('alert')).toBeNull();
  });

  test('an edit left by clicking another cell is written, after one closed from the bar', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('C1'));
    const bar = screen.getByLabelText('Contents');
    fireEvent.focus(bar);
    fireEvent.change(bar, { target: { value: 'dal bar' } });
    fireEvent.keyDown(bar, { key: 'Enter' });
    expect(sheet.input(0, 0, 2)).toBe('dal bar');

    fireEvent.keyDown(grid, { key: 'F2' });
    const editor = screen.getByRole('textbox', { name: 'C2' });
    fireEvent.change(editor, { target: { value: 'scritto' } });
    fireEvent.blur(editor);

    expect(sheet.input(0, 1, 2)).toBe('scritto');
  });

  test('arrows move, Shift extends, and Delete clears the selection', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('A1'));

    fireEvent.keyDown(grid, { key: 'ArrowDown', shiftKey: true });
    expect((screen.getByLabelText('Cell') as HTMLInputElement).value).toBe('A1:A2');
    fireEvent.keyDown(grid, { key: 'Delete' });

    expect(sheet.input(0, 0, 0)).toBe('');
    expect(sheet.input(0, 1, 0)).toBe('');
    expect(cell('A3').textContent).toBe('0');
  });

  test('the toolbar styles the selection, and Ctrl+I does too', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('B1'));

    fireEvent.click(screen.getByRole('button', { name: 'Bold' }));
    fireEvent.keyDown(grid, { key: 'i', ctrlKey: true });

    const style = sheet.styles()[sheet.cells(0, { top: 0, left: 1, bottom: 0, right: 1 })[0].style];
    expect(style.font.bold).toBe(true);
    expect(style.font.italic).toBe(true);
    expect(screen.getByRole('button', { name: 'Bold' }).getAttribute('aria-pressed')).toBe('true');
  });

  test('merging draws one cell over the range', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('B1'));
    fireEvent.keyDown(grid, { key: 'ArrowRight', shiftKey: true });

    fireEvent.click(screen.getByRole('button', { name: 'Merge cells' }));

    expect(sheet.merges(0)).toEqual([{ firstRow: 0, firstColumn: 1, lastRow: 0, lastColumn: 2 }]);
    expect(within(grid).queryByRole('gridcell', { name: 'C1' })).toBeNull();
  });

  test('rows go in above the selection and the formulas follow', () => {
    const { cell } = mount();
    fireEvent.mouseDown(cell('A1'));

    fireEvent.click(screen.getByRole('button', { name: 'Insert row above' }));

    expect(sheet.input(0, 3, 0)).toBe('=SOMMA(A2:A3)');
  });

  test('a copy within the workbook pastes the formula with its references moved', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('A3'));
    const data = new Map<string, string>();
    const clipboardData = { setData: (type: string, value: string) => data.set(type, value), getData: (type: string) => data.get(type) ?? '' };

    fireEvent.copy(grid, { clipboardData });
    expect(data.get('text/plain')).toBe('30');
    fireEvent.mouseDown(cell('B3'));
    fireEvent.paste(grid, { clipboardData });

    expect(sheet.input(0, 2, 1)).toBe('=SOMMA(B1:B2)');
  });

  test('text from elsewhere is pasted as typed cells', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('D1'));

    fireEvent.paste(grid, { clipboardData: { getData: () => '1\t2\n3\t4' } });

    expect(sheet.cells(0, { top: 0, left: 3, bottom: 1, right: 4 }).map((found) => found.display)).toEqual(['1', '2', '3', '4']);
  });

  test('sheets are added, renamed and removed from the tabs, in the language given', () => {
    mount({ labels: ITALIAN_SHEET_GRID_LABELS });

    fireEvent.click(screen.getByRole('button', { name: 'Aggiungi foglio' }));
    expect(screen.getByRole('tab', { name: 'Foglio2' }).getAttribute('aria-selected')).toBe('true');
    fireEvent.doubleClick(screen.getByRole('tab', { name: 'Foglio2' }));
    const name = screen.getByLabelText('Nome del foglio');
    fireEvent.change(name, { target: { value: 'Costi' } });
    fireEvent.keyDown(name, { key: 'Enter' });
    expect(sheet.sheets().map((summary) => summary.name)).toEqual(['Foglio1', 'Costi']);

    fireEvent.click(screen.getByRole('button', { name: 'Elimina foglio' }));
    expect(sheet.sheets().map((summary) => summary.name)).toEqual(['Foglio1']);
    expect(screen.queryByRole('button', { name: 'Elimina foglio' })).toBeNull();
  });

  test('a list validation refuses a value outside it, and offers its items', () => {
    sheet.setValidations(0, [
      {
        ranges: [{ firstRow: 4, firstColumn: 0, lastRow: 4, lastColumn: 0 }],
        kind: 'list',
        formulas: ['"Sì,No"'],
        dropdown: true,
        errorStyle: 'stop',
        showError: true,
        error: 'Scegli Sì o No',
      },
    ]);
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('A5'));
    fireEvent.keyDown(grid, { key: 'F2' });
    const editor = screen.getByRole('combobox', { name: 'A5' }) as HTMLInputElement;
    expect(editor.getAttribute('list')).toBeTruthy();

    fireEvent.change(editor, { target: { value: 'Forse' } });
    fireEvent.keyDown(editor, { key: 'Enter' });

    expect(screen.getByRole('alert').textContent).toBe('Scegli Sì o No');
    expect(sheet.input(0, 4, 0)).toBe('');
  });

  test('Ctrl+Z undoes an edit and Ctrl+Y redoes it, and so do the toolbar buttons', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('A1'));
    fireEvent.keyDown(grid, { key: 'Delete' });
    expect(cell('A3').textContent).toBe('20');

    fireEvent.keyDown(grid, { key: 'z', ctrlKey: true });
    expect(cell('A3').textContent).toBe('30');
    expect(screen.getByRole('button', { name: 'Redo' }).hasAttribute('disabled')).toBe(false);
    fireEvent.keyDown(grid, { key: 'y', ctrlKey: true });
    expect(cell('A3').textContent).toBe('20');
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(sheet.input(0, 0, 0)).toBe('10');
  });

  test('a formula typed in Italian shows in Italian and decimals take a comma', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('C1'));
    fireEvent.keyDown(grid, { key: 'F2' });
    const editor = screen.getByRole('textbox', { name: 'C1' });
    fireEvent.change(editor, { target: { value: '=se(A1>5;A1*1,5;0)' } });
    fireEvent.keyDown(editor, { key: 'Enter' });

    fireEvent.mouseDown(cell('C1'));
    expect((screen.getByLabelText('Contents') as HTMLInputElement).value).toBe('=SE(A1>5;A1*1,5;0)');
    expect(cell('C1').textContent).toBe('15');
  });

  test('read only, nothing can be typed and there is no toolbar', () => {
    const { cell, grid, ref } = mount({ editable: false });
    fireEvent.mouseDown(cell('A1'));

    fireEvent.keyDown(grid, { key: '9' });

    expect(screen.queryByRole('toolbar')).toBeNull();
    expect(screen.queryByRole('textbox', { name: 'A1' })).toBeNull();
    act(() => ref.current!.refresh());
    expect(ref.current!.selection).toEqual({ top: 0, left: 0, bottom: 0, right: 0 });
  });
});
