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

  test('dragging the fill handle continues the series as one step', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('A1'));
    fireEvent.keyDown(grid, { key: 'ArrowDown', shiftKey: true });

    fireEvent.mouseDown(grid.querySelector('.rely-sheet-fill-handle')!);
    fireEvent.mouseEnter(cell('A5'));
    fireEvent.mouseUp(window);

    expect(sheet.input(0, 2, 0)).toBe('30');
    expect(sheet.input(0, 4, 0)).toBe('50');
    expect((screen.getByLabelText('Cell') as HTMLInputElement).value).toBe('A1:A5');
    sheet.undo();
    expect(sheet.input(0, 2, 0)).toBe('=SOMMA(A1:A2)');
  });

  test('Ctrl+D fills the first row of the selection down', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('B1'));
    fireEvent.keyDown(grid, { key: 'ArrowDown', shiftKey: true });
    fireEvent.keyDown(grid, { key: 'ArrowDown', shiftKey: true });

    fireEvent.keyDown(grid, { key: 'd', ctrlKey: true });

    expect(sheet.input(0, 2, 1)).toBe('Totale');
  });

  test('dragging the edge of a column header sets its width, and a double click fits it', () => {
    mount();
    const header = screen.getAllByRole('columnheader').find((element) => element.textContent === 'B')!;
    const edge = header.querySelector('.rely-sheet-resize-column')!;

    fireEvent.mouseDown(edge, { clientX: 100 });
    fireEvent.mouseMove(window, { clientX: 170 });
    fireEvent.mouseUp(window);

    expect(sheet.sheets()[0].columnWidths).toEqual([[1, 18.43]]);
    fireEvent.doubleClick(edge);
    expect(sheet.sheets()[0].columnWidths[0][1]).toBeLessThan(18);
  });

  test('dragging the edge of a row header sets its height', () => {
    mount();
    const header = screen.getAllByRole('rowheader').find((element) => element.textContent === '2')!;

    fireEvent.mouseDown(header.querySelector('.rely-sheet-resize-row')!, { clientY: 50 });
    fireEvent.mouseMove(window, { clientY: 80 });
    fireEvent.mouseUp(window);

    expect(sheet.sheets()[0].rowHeights).toEqual([[1, 37.5]]);
  });

  test('the filter buttons hide the rows whose values are left out, and sort', () => {
    sheet.setInputs(0, [
      [0, 2, 'Zona'],
      [1, 2, 'Nord'],
      [2, 2, 'Sud'],
    ]);
    const { cell } = mount();
    fireEvent.mouseDown(cell('C1'));
    fireEvent.click(screen.getByRole('button', { name: 'Filter' }));
    expect(sheet.sheets()[0].autoFilter).toMatchObject({ top: 0, left: 0, bottom: 2, right: 2 });

    fireEvent.click(screen.getByRole('button', { name: 'Filter column C' }));
    const menu = screen.getByRole('dialog', { name: 'Filter column' });
    fireEvent.click(within(menu).getByRole('checkbox', { name: 'Sud' }));
    fireEvent.click(within(menu).getByRole('button', { name: 'OK' }));

    expect(sheet.sheets()[0].autoFilter?.hiddenRows).toEqual([2]);
    expect(screen.getByRole('button', { name: 'Filter column C' }).getAttribute('aria-pressed')).toBe('true');

    fireEvent.click(screen.getByRole('button', { name: 'Filter' }));
    expect(sheet.sheets()[0].autoFilter).toBeNull();
    expect(sheet.sheets()[0].rowHeights).toEqual([]);
  });

  test('sorting the column of the active cell orders the rows below a bold heading', () => {
    sheet.setInputs(0, [
      [0, 3, 'Nome'],
      [1, 3, 'Verdi'],
      [2, 3, 'Bianchi'],
      [3, 3, 'Rossi'],
    ]);
    sheet.setStyle(0, { top: 0, left: 3, bottom: 0, right: 3 }, { bold: true });
    const { cell } = mount();
    fireEvent.mouseDown(cell('D1'));
    fireEvent.mouseEnter(cell('D4'));
    fireEvent.mouseUp(window);

    fireEvent.click(screen.getByRole('button', { name: 'Sort A to Z' }));

    expect([1, 2, 3].map((row) => sheet.input(0, row, 3))).toEqual(['Bianchi', 'Rossi', 'Verdi']);
  });

  test('freezing keeps the rows above the active cell in a pane of their own', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('B2'));

    fireEvent.click(screen.getByRole('button', { name: 'Freeze panes at the selection' }));

    expect(sheet.sheets()[0]).toMatchObject({ frozenRows: 1, frozenColumns: 1 });
    expect(document.querySelectorAll('.rely-sheet-pane')).toHaveLength(3);
    expect(within(grid).queryByRole('gridcell', { name: 'A1' })).toBeNull();
    expect(screen.getAllByRole('gridcell', { name: 'A1' })).toHaveLength(1);
    fireEvent.click(screen.getByRole('button', { name: 'Unfreeze panes' }));
    expect(document.querySelectorAll('.rely-sheet-pane')).toHaveLength(0);
  });

  test('typing a function name offers the functions, and Tab takes one', () => {
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('C1'));
    fireEvent.keyDown(grid, { key: '=' });
    const editor = screen.getByRole('textbox', { name: 'C1' }) as HTMLInputElement;

    fireEvent.change(editor, { target: { value: '=som', selectionStart: 4 } });
    const hints = screen.getByRole('listbox', { name: 'Functions' });
    expect(within(hints).getAllByRole('option')[0].textContent).toBe('SOMMA');
    fireEvent.keyDown(editor, { key: 'Tab' });

    expect(editor.value).toBe('=SOMMA(');
    expect(screen.queryByRole('listbox')).toBeNull();
    fireEvent.change(editor, { target: { value: '=SOMMA(A1:A2)', selectionStart: 13 } });
    fireEvent.keyDown(editor, { key: 'Enter' });
    expect(cell('C1').textContent).toBe('30');
  });

  test('the conditional format panel adds a rule to the selection', () => {
    const { cell } = mount();
    fireEvent.mouseDown(cell('A1'));
    fireEvent.click(screen.getByRole('button', { name: 'Conditional formatting' }));
    const panel = screen.getByRole('complementary', { name: 'Conditional formatting' });
    expect(within(panel).getByText('No rules on this sheet')).toBeTruthy();

    fireEvent.change(within(panel).getByRole('textbox', { name: 'Value' }), { target: { value: '15' } });
    fireEvent.click(within(panel).getByRole('button', { name: 'Add rule' }));

    expect(sheet.conditionalFormats(0)).toHaveLength(1);
    expect(sheet.conditionalFormats(0)[0].rules[0].condition).toEqual({ kind: 'cellIs', operator: 'greater', formulas: ['15'] });
    fireEvent.click(within(panel).getByRole('button', { name: 'Remove' }));
    expect(sheet.conditionalFormats(0)).toHaveLength(0);
  });

  test('the validation panel puts a list on the selection', () => {
    const { cell } = mount();
    fireEvent.mouseDown(cell('E1'));
    fireEvent.click(screen.getByRole('button', { name: 'Data validation' }));
    const panel = screen.getByRole('complementary', { name: 'Data validation' });

    fireEvent.change(within(panel).getByRole('textbox', { name: 'Items, separated by a semicolon' }), { target: { value: 'Sì;No' } });
    fireEvent.click(within(panel).getByRole('button', { name: 'Apply to the selection' }));

    expect(sheet.validations(0)).toMatchObject([{ kind: 'list', formulas: ['"Sì,No"'] }]);
    expect(sheet.validation(0, 0, 4).items).toEqual(['Sì', 'No']);
  });

  test('the comments panel writes a comment, marks its cell, and removes it', () => {
    const { cell } = mount({ author: 'Anna' });
    fireEvent.mouseDown(cell('B1'));
    fireEvent.click(screen.getByRole('button', { name: 'Comments' }));
    const panel = screen.getByRole('complementary', { name: 'Comments' });
    expect(within(panel).getByText('No comments on this sheet')).toBeTruthy();

    fireEvent.change(within(panel).getByRole('textbox', { name: 'Comments' }), { target: { value: 'Da rivedere' } });
    fireEvent.click(within(panel).getByRole('button', { name: 'Save comment' }));

    expect(sheet.comments(0)).toEqual([{ row: 0, column: 1, author: 'Anna', text: 'Da rivedere' }]);
    expect(cell('B1').className).toContain('rely-sheet-commented');
    expect(cell('B1').getAttribute('title')).toBe('Anna: Da rivedere');

    fireEvent.mouseDown(cell('A1'));
    expect((within(panel).getByRole('textbox', { name: 'Comments' }) as HTMLTextAreaElement).value).toBe('');
    fireEvent.click(within(panel).getByRole('button', { name: 'B1' }));
    expect((within(panel).getByRole('textbox', { name: 'Comments' }) as HTMLTextAreaElement).value).toBe('Da rivedere');

    fireEvent.click(within(panel).getByRole('button', { name: 'Remove' }));
    expect(sheet.comments(0)).toEqual([]);
    expect(cell('B1').className).not.toContain('rely-sheet-commented');
  });

  test('the names panel defines a name formulas read, and shows a refused one', () => {
    const { cell } = mount();
    fireEvent.click(screen.getByRole('button', { name: 'Names' }));
    const panel = screen.getByRole('complementary', { name: 'Names' });
    expect(within(panel).getByText('The workbook defines no names')).toBeTruthy();

    fireEvent.change(within(panel).getByRole('textbox', { name: 'Name' }), { target: { value: 'A1' } });
    fireEvent.change(within(panel).getByRole('textbox', { name: 'Refers to' }), { target: { value: '=1' } });
    fireEvent.click(within(panel).getByRole('button', { name: 'Define name' }));
    expect(within(panel).getByRole('alert').textContent).toContain('starts with a letter');

    fireEvent.change(within(panel).getByRole('textbox', { name: 'Name' }), { target: { value: 'Valori' } });
    fireEvent.change(within(panel).getByRole('textbox', { name: 'Refers to' }), { target: { value: '=A1:A2' } });
    fireEvent.click(within(panel).getByRole('button', { name: 'Define name' }));
    expect(sheet.definedNames()).toEqual([{ name: 'Valori', formula: '=Foglio1!A1:A2', hidden: false }]);
    expect(within(panel).queryByRole('alert')).toBeNull();

    sheet.setInput(0, 4, 0, '=SOMMA(Valori)');
    act(() => screen.getByRole('button', { name: 'Names' }).focus());
    expect(sheet.cells(0, { top: 4, left: 0, bottom: 4, right: 0 })[0].display).toBe('30');

    fireEvent.click(within(panel).getByRole('button', { name: 'Remove' }));
    expect(sheet.definedNames()).toEqual([]);
    expect(cell('A1').textContent).toBe('10');
  });

  test('the charts panel inserts a chart from the selection, changes it, moves it and removes it', () => {
    sheet.setInputs(0, [
      [0, 0, 'Mese'],
      [0, 1, 'Vendite'],
      [1, 0, 'Gen'],
      [1, 1, '10'],
      [2, 0, 'Feb'],
      [2, 1, '20'],
    ]);
    const { cell, grid } = mount();
    fireEvent.mouseDown(cell('A1'));
    fireEvent.mouseDown(cell('B3'), { shiftKey: true });
    fireEvent.click(screen.getByRole('button', { name: 'Charts' }));
    const panel = screen.getByRole('complementary', { name: 'Charts' });
    expect(within(panel).getByText('No charts on this sheet')).toBeTruthy();

    fireEvent.change(within(panel).getAllByRole('combobox', { name: 'Type' })[0], { target: { value: 'line' } });
    fireEvent.click(within(panel).getByRole('button', { name: 'Insert chart' }));
    const charts = sheet.charts(0);
    expect(charts).toHaveLength(1);
    expect(charts[0].spec).toMatchObject({
      chartType: 'line',
      categories: 'Foglio1!$A$2:$A$3',
      series: [{ name: 'Foglio1!$B$1', values: 'Foglio1!$B$2:$B$3' }],
    });
    expect(charts[0].placement).toMatchObject({ row: 0, column: 3 });
    const frame = within(grid).getByRole('button', { name: 'Chart 1' });
    expect(frame.getAttribute('aria-pressed')).toBe('true');

    const kinds = within(panel).getAllByRole('combobox', { name: 'Type' });
    fireEvent.change(kinds[kinds.length - 1], { target: { value: 'bar' } });
    expect(sheet.charts(0)[0].spec?.chartType).toBe('bar');
    const title = within(panel).getByRole('textbox', { name: 'Title' });
    fireEvent.change(title, { target: { value: 'Trimestre' } });
    fireEvent.blur(title);
    expect(sheet.charts(0)[0].spec?.title).toBe('Trimestre');
    fireEvent.click(within(panel).getByRole('checkbox', { name: 'Stacked' }));
    expect(sheet.charts(0)[0].spec?.stacked).toBe(true);

    const moved = within(grid).getByRole('button', { name: 'Chart Trimestre' });
    fireEvent.pointerDown(moved, { button: 0, clientX: 100, clientY: 100, pointerId: 1 });
    fireEvent.pointerMove(moved, { clientX: 300, clientY: 160, pointerId: 1 });
    fireEvent.pointerUp(moved, { clientX: 300, clientY: 160, pointerId: 1 });
    const placement = sheet.charts(0)[0].placement;
    expect(placement.column).toBeGreaterThan(3);
    expect(placement.row).toBeGreaterThan(0);

    fireEvent.keyDown(grid, { key: 'Delete' });
    expect(sheet.charts(0)).toEqual([]);
    expect(sheet.cells(0, { top: 0, left: 0, bottom: 0, right: 0 })[0].display).toBe('Mese');
  });

  test('a selection with nothing to plot makes no chart', () => {
    const { cell } = mount();
    fireEvent.mouseDown(cell('B1'));
    fireEvent.click(screen.getByRole('button', { name: 'Charts' }));
    const panel = screen.getByRole('complementary', { name: 'Charts' });
    fireEvent.click(within(panel).getByRole('button', { name: 'Insert chart' }));
    expect(within(panel).getByRole('alert').textContent).toBe('The selection holds no numbers to plot');
    expect(sheet.charts(0)).toEqual([]);
  });

  test('the borders menu puts a border on the selection', () => {
    const { cell } = mount();
    fireEvent.mouseDown(cell('B1'));
    fireEvent.click(screen.getByRole('button', { name: 'Borders' }));

    fireEvent.click(screen.getByRole('button', { name: 'All borders' }));

    const style = sheet.styles()[sheet.cells(0, { top: 0, left: 1, bottom: 0, right: 1 })[0].style];
    expect(style.borders.top).toMatchObject({ style: 'single', colour: '000000' });
    expect(screen.queryByRole('dialog', { name: 'Borders' })).toBeNull();
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
