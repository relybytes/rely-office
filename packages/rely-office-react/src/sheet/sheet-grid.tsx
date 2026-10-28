// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
  type ClipboardEvent,
  type CSSProperties,
  type KeyboardEvent,
  type ReactElement,
  type Ref,
} from 'react';
import type {
  Border,
  CellRange,
  CellStyle,
  CellWindow,
  ConditionalLook,
  SheetCell,
  SheetDocument,
  StylePatch,
} from '../model.js';
import {
  GridAxis,
  cellName,
  columnName,
  columnWidthPixels,
  coverMerges,
  parseCellName,
  parseTabSeparated,
  rowHeightPixels,
  toTabSeparated,
  windowBetween,
  type CellPosition,
} from './geometry.js';
import { ENGLISH_SHEET_GRID_LABELS, type SheetGridLabels } from './labels.js';

const DEFAULT_ROW_HEIGHT = rowHeightPixels(15);
const DEFAULT_COLUMN_WIDTH = columnWidthPixels(8.43);
const HEADER_HEIGHT = 24;
const HEADER_WIDTH = 48;
const EXTRA_ROWS = 50;
const EXTRA_COLUMNS = 10;
const MINIMUM_ROWS = 100;
const MINIMUM_COLUMNS = 26;

const NUMBER_FORMATS: [keyof SheetGridLabels, string][] = [
  ['formatGeneral', ''],
  ['formatNumber', '#,##0.00'],
  ['formatPercent', '0%'],
  ['formatDate', 'dd/mm/yyyy'],
  ['formatCurrency', '#,##0.00 "€"'],
];

const ICON_COLOURS = ['#c92a2a', '#f08c00', '#2f9e44', '#1971c2', '#6741d9'];

interface Selection {
  anchor: CellPosition;
  focus: CellPosition;
}

interface Editing extends CellPosition {
  text: string;
  source: 'cell' | 'bar';
}

/** What a host can ask of a mounted grid. */
export interface SheetGridHandle {
  activeSheet: number;
  /** The selected window of the active sheet. */
  selection: CellWindow;
  /** Draw the workbook again, after the host changed it itself. */
  refresh: () => void;
  focus: () => void;
}

export interface SheetGridProps {
  /** The workbook to edit; the grid changes it in place. */
  sheet: SheetDocument;
  /** Called after every change to the workbook. */
  onChange?: () => void;
  /** False shows the workbook without letting it change. */
  editable?: boolean;
  /** The words of the grid, in the host's language; English by default. */
  labels?: Partial<SheetGridLabels>;
  className?: string;
}

function borderCss(border: Border | null | undefined): string | undefined {
  if (!border || border.style === 'none') return undefined;
  const width = Math.max(1, Math.round(((border.widthEighths / 8) * 96) / 72));
  const style = border.style === 'single' ? 'solid' : border.style;
  return `${border.style === 'double' ? Math.max(3, width) : width}px ${style} #${border.colour ?? '000000'}`;
}

function cellCss(cell: SheetCell | undefined, style: CellStyle | undefined, look: ConditionalLook | undefined): CSSProperties {
  const css: CSSProperties = {};
  if (style) {
    if (style.font.family) css.fontFamily = style.font.family;
    if (style.font.sizeTenths) css.fontSize = `${style.font.sizeTenths / 10}pt`;
    if (style.font.colour) css.color = `#${style.font.colour}`;
    if (style.font.bold) css.fontWeight = 700;
    if (style.font.italic) css.fontStyle = 'italic';
    const decorations = [style.font.underline ? 'underline' : '', style.font.strike ? 'line-through' : ''].filter(Boolean);
    if (decorations.length > 0) css.textDecoration = decorations.join(' ');
    if (style.fill) css.background = `#${style.fill}`;
    css.borderTop = borderCss(style.borders.top);
    css.borderRight = borderCss(style.borders.right);
    css.borderBottom = borderCss(style.borders.bottom);
    css.borderLeft = borderCss(style.borders.left);
    if (style.wrap) css.whiteSpace = 'normal';
    if (style.vertical) css.alignItems = style.vertical === 'top' ? 'flex-start' : style.vertical === 'center' ? 'center' : 'flex-end';
  }
  const horizontal =
    style?.horizontal ??
    (cell?.kind === 'number' ? 'right' : cell?.kind === 'bool' || cell?.kind === 'error' ? 'center' : null);
  if (horizontal) css.justifyContent = horizontal === 'center' ? 'center' : horizontal === 'right' ? 'flex-end' : 'flex-start';
  if (horizontal) css.textAlign = horizontal;
  if (look) {
    if (look.fill) css.background = `#${look.fill}`;
    if (look.fontColour) css.color = `#${look.fontColour}`;
    if (look.bold !== null) css.fontWeight = look.bold ? 700 : 400;
    if (look.italic !== null) css.fontStyle = look.italic ? 'italic' : 'normal';
  }
  return css;
}

/**
 * A spreadsheet grid over a `SheetDocument`: the sheets as tabs, a formula bar, cells drawn with
 * their styles, merges, conditional formats and validation, and the editing a spreadsheet user
 * expects, from typing into a cell to copying a range with its formulas.
 */
export const SheetGrid = forwardRef(function SheetGrid(
  { sheet, onChange, editable = true, labels, className }: SheetGridProps,
  ref: Ref<SheetGridHandle>,
): ReactElement {
  const words = useMemo(() => ({ ...ENGLISH_SHEET_GRID_LABELS, ...labels }), [labels]);
  const [activeSheet, setActiveSheet] = useState(0);
  const [revision, refresh] = useReducer((value: number) => value + 1, 0);
  const [selection, setSelection] = useState<Selection>({ anchor: { row: 0, column: 0 }, focus: { row: 0, column: 0 } });
  const [editing, setEditing] = useState<Editing | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [scroll, setScroll] = useState({ top: 0, left: 0 });
  const [viewportSize, setViewportSize] = useState({ width: 960, height: 480 });
  const [renaming, setRenaming] = useState<{ index: number; name: string } | null>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  const copied = useRef<{ text: string; sheet: number; window: CellWindow } | null>(null);
  // Set while an edit is being closed on purpose, so the blur that follows does not commit it.
  const closing = useRef(false);

  const workbook = useMemo(() => {
    const sheets = sheet.sheets();
    const index = Math.min(activeSheet, sheets.length - 1);
    const looks = new Map<string, ConditionalLook>();
    for (const look of sheet.conditionalLooks(index)) looks.set(`${look.row}:${look.column}`, look);
    return {
      sheets,
      index,
      summary: sheets[index],
      styles: sheet.styles(),
      merges: sheet.merges(index),
      looks,
      invalid: new Set(sheet.invalidCells(index).map(([row, column]) => `${row}:${column}`)),
    };
    // `revision` stands for every edit of the workbook, which changes in place.
  }, [sheet, activeSheet, revision]);

  const rows = useMemo(
    () => new GridAxis(DEFAULT_ROW_HEIGHT, workbook.summary.rowHeights.map(([row, points]) => [row, rowHeightPixels(points)])),
    [workbook],
  );
  const columns = useMemo(
    () => new GridAxis(DEFAULT_COLUMN_WIDTH, workbook.summary.columnWidths.map(([column, width]) => [column, columnWidthPixels(width)])),
    [workbook],
  );

  const selected = useMemo(
    () => coverMerges(windowBetween(selection.anchor, selection.focus), workbook.merges),
    [selection, workbook.merges],
  );
  const rowCount = Math.max(MINIMUM_ROWS, Math.max(workbook.summary.rows, selected.bottom + 1) + EXTRA_ROWS);
  const columnCount = Math.max(MINIMUM_COLUMNS, Math.max(workbook.summary.columns, selected.right + 1) + EXTRA_COLUMNS);

  const visible: CellWindow = useMemo(() => {
    const top = Math.min(rows.indexAt(scroll.top), rowCount - 1);
    const left = Math.min(columns.indexAt(scroll.left), columnCount - 1);
    return {
      top,
      left,
      bottom: Math.min(rowCount - 1, rows.indexAt(scroll.top + viewportSize.height) + 1),
      right: Math.min(columnCount - 1, columns.indexAt(scroll.left + viewportSize.width) + 1),
    };
  }, [rows, columns, scroll, viewportSize, rowCount, columnCount]);

  const cells = useMemo(() => {
    const map = new Map<string, SheetCell>();
    for (const cell of sheet.cells(workbook.index, visible)) map.set(`${cell.row}:${cell.column}`, cell);
    return map;
  }, [sheet, workbook, visible]);

  const cellAt = useCallback(
    ({ row, column }: CellPosition): SheetCell | undefined =>
      cells.get(`${row}:${column}`) ?? sheet.cells(workbook.index, { top: row, left: column, bottom: row, right: column })[0],
    [cells, sheet, workbook.index],
  );

  useLayoutEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const measure = () => {
      if (element.clientWidth > 0 && element.clientHeight > 0) {
        setViewportSize({ width: element.clientWidth, height: element.clientHeight });
      }
    };
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const release = () => {
      dragging.current = false;
    };
    window.addEventListener('mouseup', release);
    return () => window.removeEventListener('mouseup', release);
  }, []);

  /** Run an edit of the workbook; a refusal of rely-office is shown and changes nothing. */
  const mutate = useCallback(
    (edit: () => void): boolean => {
      try {
        edit();
        setMessage(null);
        refresh();
        onChange?.();
        return true;
      } catch (error) {
        setMessage(error instanceof Error ? error.message : String(error));
        return false;
      }
    },
    [onChange],
  );

  const scrollIntoView = useCallback(
    ({ row, column }: CellPosition) => {
      const element = viewport.current;
      if (!element) return;
      const top = rows.offset(row);
      const bottom = top + rows.size(row);
      const left = columns.offset(column);
      const right = left + columns.size(column);
      if (top < element.scrollTop) element.scrollTop = top;
      else if (bottom > element.scrollTop + viewportSize.height) element.scrollTop = bottom - viewportSize.height;
      if (left < element.scrollLeft) element.scrollLeft = left;
      else if (right > element.scrollLeft + viewportSize.width) element.scrollLeft = right - viewportSize.width;
    },
    [rows, columns, viewportSize],
  );

  const moveTo = useCallback(
    (position: CellPosition, extend = false) => {
      const target = { row: Math.max(0, position.row), column: Math.max(0, position.column) };
      setSelection((current) => (extend ? { anchor: current.anchor, focus: target } : { anchor: target, focus: target }));
      scrollIntoView(target);
    },
    [scrollIntoView],
  );

  const focusGrid = () => viewport.current?.focus();

  const commit = useCallback(
    (move?: { rows: number; columns: number }) => {
      if (!editing) return;
      const { row, column, text } = editing;
      const done = mutate(() => {
        sheet.setInput(workbook.index, row, column, text);
        const check = sheet.validation(workbook.index, row, column);
        const rule = check.validation;
        if (!check.valid && rule && rule.showError !== false && (rule.errorStyle ?? 'stop') === 'stop') {
          // Undo rather than type the old value back, so the refused value is no step of the history.
          sheet.undo();
          throw new Error(rule.error ?? words.invalidValue);
        }
      });
      if (!done) return;
      closing.current = true;
      setEditing(null);
      if (move) moveTo({ row: row + move.rows, column: column + move.columns });
      focusGrid();
    },
    [editing, sheet, workbook.index, mutate, moveTo, words.invalidValue],
  );

  const startEditing = (text?: string) => {
    if (!editable) return;
    closing.current = false;
    const { anchor } = selection;
    setEditing({ ...anchor, text: text ?? sheet.input(workbook.index, anchor.row, anchor.column), source: 'cell' });
  };

  const step = (direction: 'undo' | 'redo') => {
    if (!editable) return;
    const moved = direction === 'undo' ? sheet.undo() : sheet.redo();
    if (!moved) return;
    setEditing(null);
    setMessage(null);
    refresh();
    onChange?.();
  };

  const activeStyle = workbook.styles[cellAt(selection.anchor)?.style ?? 0] ?? workbook.styles[0];
  const toggle = (key: 'bold' | 'italic' | 'underline' | 'strike') =>
    mutate(() => sheet.setStyle(workbook.index, selected, { [key]: !(activeStyle?.font[key] ?? false) }));
  const applyStyle = (patch: StylePatch) => mutate(() => sheet.setStyle(workbook.index, selected, patch));

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (editing) return;
    const { anchor, focus } = selection;
    const from = event.shiftKey ? focus : anchor;
    const command = event.ctrlKey || event.metaKey;
    const steps: Record<string, [number, number]> = {
      ArrowUp: [-1, 0],
      ArrowDown: [1, 0],
      ArrowLeft: [0, -1],
      ArrowRight: [0, 1],
    };
    const key = event.key.toLowerCase();
    if (command && key === 'z' && !event.shiftKey) {
      step('undo');
    } else if (command && (key === 'y' || (key === 'z' && event.shiftKey))) {
      step('redo');
    } else if (event.key in steps) {
      const [rowStep, columnStep] = steps[event.key];
      moveTo({ row: from.row + rowStep, column: from.column + columnStep }, event.shiftKey);
    } else if (event.key === 'Tab' || event.key === 'Enter') {
      const backwards = event.shiftKey;
      const step = event.key === 'Tab' ? { row: 0, column: backwards ? -1 : 1 } : { row: backwards ? -1 : 1, column: 0 };
      moveTo({ row: anchor.row + step.row, column: anchor.column + step.column });
    } else if (event.key === 'F2') {
      startEditing();
    } else if ((event.key === 'Delete' || event.key === 'Backspace') && editable) {
      mutate(() => sheet.clearRange(workbook.index, selected));
    } else if (command && ['b', 'i', 'u'].includes(event.key.toLowerCase()) && editable) {
      toggle(({ b: 'bold', i: 'italic', u: 'underline' } as const)[event.key.toLowerCase() as 'b' | 'i' | 'u']);
    } else if (event.key.length === 1 && !command && !event.altKey) {
      startEditing(event.key);
    } else {
      return;
    }
    event.preventDefault();
  };

  const copySelection = (event: ClipboardEvent<HTMLDivElement>) => {
    const grid: string[][] = [];
    const found = new Map(sheet.cells(workbook.index, selected).map((cell) => [`${cell.row}:${cell.column}`, cell.display]));
    for (let row = selected.top; row <= selected.bottom; row += 1) {
      const line: string[] = [];
      for (let column = selected.left; column <= selected.right; column += 1) line.push(found.get(`${row}:${column}`) ?? '');
      grid.push(line);
    }
    const text = toTabSeparated(grid);
    event.clipboardData.setData('text/plain', text);
    copied.current = { text, sheet: workbook.index, window: selected };
    event.preventDefault();
  };

  const paste = (event: ClipboardEvent<HTMLDivElement>) => {
    if (!editable || editing) return;
    const text = event.clipboardData.getData('text/plain');
    event.preventDefault();
    const { anchor } = selection;
    const own = copied.current;
    if (own !== null && own.text === text) {
      // From this workbook: the formulas move with their relative references, as a spreadsheet does.
      mutate(() => sheet.copyRange(own.sheet, own.window, workbook.index, anchor.row, anchor.column));
      return;
    }
    const inputs: [number, number, string][] = [];
    parseTabSeparated(text).forEach((line, rowOffset) =>
      line.forEach((value, columnOffset) => inputs.push([anchor.row + rowOffset, anchor.column + columnOffset, value])),
    );
    mutate(() => sheet.setInputs(workbook.index, inputs));
  };

  useImperativeHandle(
    ref,
    () => ({ activeSheet: workbook.index, selection: selected, refresh, focus: focusGrid }),
    [workbook.index, selected],
  );

  const merges = workbook.merges.filter(
    (merge) =>
      merge.firstRow <= visible.bottom && merge.lastRow >= visible.top && merge.firstColumn <= visible.right && merge.lastColumn >= visible.left,
  );
  const coveredBy = new Map<string, CellRange>();
  for (const merge of workbook.merges) {
    for (let row = merge.firstRow; row <= merge.lastRow; row += 1) {
      for (let column = merge.firstColumn; column <= merge.lastColumn; column += 1) {
        coveredBy.set(`${row}:${column}`, merge);
      }
    }
  }

  const rectangle = (window: CellWindow): CSSProperties => ({
    top: rows.offset(window.top),
    left: columns.offset(window.left),
    height: rows.offset(window.bottom + 1) - rows.offset(window.top),
    width: columns.offset(window.right + 1) - columns.offset(window.left),
  });

  const renderCell = (row: number, column: number, span?: CellRange): ReactElement => {
    const key = `${row}:${column}`;
    const cell = cells.get(key) ?? (span ? cellAt({ row, column }) : undefined);
    const style = cell ? workbook.styles[cell.style] : undefined;
    const look = workbook.looks.get(key);
    const area = span
      ? rectangle({ top: span.firstRow, left: span.firstColumn, bottom: span.lastRow, right: span.lastColumn })
      : rectangle({ top: row, left: column, bottom: row, right: column });
    const showValue = !look?.bar && !look?.icon ? true : (look.bar?.showValue ?? look.icon?.showValue ?? true);
    return (
      <div
        key={key}
        role="gridcell"
        aria-label={cellName({ row, column })}
        aria-selected={row >= selected.top && row <= selected.bottom && column >= selected.left && column <= selected.right}
        className={['rely-sheet-cell', workbook.invalid.has(key) ? 'rely-sheet-invalid' : ''].filter(Boolean).join(' ')}
        style={{ ...area, ...cellCss(cell, style, look) }}
        onMouseDown={(event) => {
          if (event.button !== 0) return;
          if (editing) commit();
          moveTo({ row, column }, event.shiftKey);
          dragging.current = true;
          focusGrid();
        }}
        onMouseEnter={() => {
          if (dragging.current) setSelection((current) => ({ anchor: current.anchor, focus: { row, column } }));
        }}
        onDoubleClick={() => startEditing()}
      >
        {look?.bar ? (
          <span className="rely-sheet-bar" style={{ width: `${look.bar.length * 100}%`, background: `#${look.bar.colour}` }} />
        ) : null}
        {look?.icon ? (
          <span className="rely-sheet-icon" style={{ color: ICON_COLOURS[look.icon.index % ICON_COLOURS.length] }}>
            {'●'}
          </span>
        ) : null}
        {showValue ? <span className="rely-sheet-value">{cell?.display ?? ''}</span> : null}
      </div>
    );
  };

  const body: ReactElement[] = [];
  for (let row = visible.top; row <= visible.bottom; row += 1) {
    for (let column = visible.left; column <= visible.right; column += 1) {
      if (coveredBy.has(`${row}:${column}`)) continue;
      body.push(renderCell(row, column));
    }
  }
  for (const merge of merges) body.push(renderCell(merge.firstRow, merge.firstColumn, merge));

  const editingCell = editing && editing.source === 'cell' ? editing : null;
  const editingArea = editingCell
    ? (() => {
        const merge = coveredBy.get(`${editingCell.row}:${editingCell.column}`);
        return merge
          ? rectangle({ top: merge.firstRow, left: merge.firstColumn, bottom: merge.lastRow, right: merge.lastColumn })
          : rectangle({ top: editingCell.row, left: editingCell.column, bottom: editingCell.row, right: editingCell.column });
      })()
    : null;
  const validationItems = editing ? sheet.validation(workbook.index, editing.row, editing.column).items : [];
  const listId = `rely-sheet-list-${workbook.index}`;
  const anchorName = cellName(selection.anchor);
  const rangeName =
    selected.top === selected.bottom && selected.left === selected.right
      ? anchorName
      : `${cellName({ row: selected.top, column: selected.left })}:${cellName({ row: selected.bottom, column: selected.right })}`;

  const editorKeys = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      commit({ rows: event.shiftKey ? -1 : 1, columns: 0 });
    } else if (event.key === 'Tab') {
      event.preventDefault();
      commit({ rows: 0, columns: event.shiftKey ? -1 : 1 });
    } else if (event.key === 'Escape') {
      event.preventDefault();
      closing.current = true;
      setEditing(null);
      setMessage(null);
      focusGrid();
    }
  };

  return (
    <div className={['rely-sheet-grid', className].filter(Boolean).join(' ')}>
      {editable ? (
        <div className="rely-toolbar" role="toolbar" aria-label={words.toolbar}>
          <div className="rely-tool-group">
            <button
              type="button"
              className="rely-tool"
              aria-label={words.undo}
              title={words.undo}
              disabled={!sheet.canUndo()}
              onClick={() => step('undo')}
            >
              {'↶'}
            </button>
            <button
              type="button"
              className="rely-tool"
              aria-label={words.redo}
              title={words.redo}
              disabled={!sheet.canRedo()}
              onClick={() => step('redo')}
            >
              {'↷'}
            </button>
          </div>
          <div className="rely-tool-group">
            {(['bold', 'italic', 'underline', 'strike'] as const).map((key) => (
              <button
                key={key}
                type="button"
                className="rely-tool"
                aria-label={words[key]}
                title={words[key]}
                aria-pressed={activeStyle?.font[key] ?? false}
                onClick={() => toggle(key)}
              >
                {{ bold: <b>B</b>, italic: <i>I</i>, underline: <u>U</u>, strike: <s>S</s> }[key]}
              </button>
            ))}
            <label className="rely-tool rely-tool-colour" title={words.fontColour}>
              <span aria-hidden="true">A</span>
              <input type="color" aria-label={words.fontColour} onChange={(event) => applyStyle({ fontColour: event.target.value })} />
            </label>
            <label className="rely-tool rely-tool-colour" title={words.fill}>
              <span aria-hidden="true">{'▧'}</span>
              <input type="color" aria-label={words.fill} onChange={(event) => applyStyle({ fill: event.target.value })} />
            </label>
          </div>
          <div className="rely-tool-group">
            {(
              [
                ['left', 'alignLeft', '≡'],
                ['center', 'alignCenter', '≣'],
                ['right', 'alignRight', '≢'],
              ] as const
            ).map(([align, key, glyph]) => (
              <button
                key={align}
                type="button"
                className="rely-tool"
                aria-label={words[key]}
                title={words[key]}
                aria-pressed={activeStyle?.horizontal === align}
                onClick={() => applyStyle({ horizontal: activeStyle?.horizontal === align ? 'general' : align })}
              >
                {glyph}
              </button>
            ))}
            <button
              type="button"
              className="rely-tool"
              aria-label={words.wrap}
              title={words.wrap}
              aria-pressed={activeStyle?.wrap ?? false}
              onClick={() => applyStyle({ wrap: !(activeStyle?.wrap ?? false) })}
            >
              {'↵'}
            </button>
            <select
              className="rely-tool-select"
              aria-label={words.numberFormat}
              value={NUMBER_FORMATS.find(([, code]) => code === (activeStyle?.numberFormat ?? ''))?.[1] ?? ''}
              onChange={(event) => applyStyle({ numberFormat: event.target.value })}
            >
              {NUMBER_FORMATS.map(([key, code]) => (
                <option key={key} value={code}>
                  {words[key] as string}
                </option>
              ))}
            </select>
          </div>
          <div className="rely-tool-group">
            <button type="button" className="rely-tool" aria-label={words.merge} title={words.merge} onClick={() => mutate(() => sheet.mergeCells(workbook.index, selected))}>
              {'⬚'}
            </button>
            <button type="button" className="rely-tool" aria-label={words.unmerge} title={words.unmerge} onClick={() => mutate(() => sheet.unmergeCells(workbook.index, selected))}>
              {'⊞'}
            </button>
            <button type="button" className="rely-tool" aria-label={words.insertRow} title={words.insertRow} onClick={() => mutate(() => sheet.insertRows(workbook.index, selected.top, selected.bottom - selected.top + 1))}>
              {'+━'}
            </button>
            <button type="button" className="rely-tool" aria-label={words.deleteRow} title={words.deleteRow} onClick={() => mutate(() => sheet.deleteRows(workbook.index, selected.top, selected.bottom - selected.top + 1))}>
              {'−━'}
            </button>
            <button type="button" className="rely-tool" aria-label={words.insertColumn} title={words.insertColumn} onClick={() => mutate(() => sheet.insertColumns(workbook.index, selected.left, selected.right - selected.left + 1))}>
              {'+┃'}
            </button>
            <button type="button" className="rely-tool" aria-label={words.deleteColumn} title={words.deleteColumn} onClick={() => mutate(() => sheet.deleteColumns(workbook.index, selected.left, selected.right - selected.left + 1))}>
              {'−┃'}
            </button>
            <button type="button" className="rely-tool" aria-label={words.clear} title={words.clear} onClick={() => mutate(() => sheet.clearRange(workbook.index, selected))}>
              {'✖'}
            </button>
          </div>
        </div>
      ) : null}

      <div className="rely-sheet-formula-bar">
        <input
          className="rely-sheet-name"
          aria-label={words.cellName}
          key={rangeName}
          defaultValue={rangeName}
          onKeyDown={(event) => {
            if (event.key !== 'Enter') return;
            const target = parseCellName(event.currentTarget.value);
            if (target) moveTo(target);
            focusGrid();
          }}
        />
        <input
          className="rely-sheet-formula"
          aria-label={words.formula}
          readOnly={!editable}
          value={editing ? editing.text : sheet.input(workbook.index, selection.anchor.row, selection.anchor.column)}
          onFocus={() => {
            if (editable && !editing) {
              closing.current = false;
              setEditing({ ...selection.anchor, text: sheet.input(workbook.index, selection.anchor.row, selection.anchor.column), source: 'bar' });
            }
          }}
          onChange={(event) => setEditing((current) => (current ? { ...current, text: event.target.value } : current))}
          onKeyDown={editorKeys}
        />
      </div>
      {message ? (
        <div className="rely-sheet-message" role="alert">
          {message}
        </div>
      ) : null}

      <div className="rely-sheet-frame">
        <div className="rely-sheet-corner" style={{ width: HEADER_WIDTH, height: HEADER_HEIGHT }} />
        <div className="rely-sheet-column-headers" style={{ left: HEADER_WIDTH, height: HEADER_HEIGHT }}>
          <div style={{ transform: `translateX(${-scroll.left}px)` }}>
            {Array.from({ length: visible.right - visible.left + 1 }, (_, offset) => visible.left + offset).map((column) => (
              <div
                key={column}
                role="columnheader"
                className={column >= selected.left && column <= selected.right ? 'rely-sheet-header rely-sheet-header-selected' : 'rely-sheet-header'}
                style={{ left: columns.offset(column), width: columns.size(column), height: HEADER_HEIGHT }}
                onMouseDown={(event) => {
                  setSelection((current) => ({ anchor: event.shiftKey ? current.anchor : { row: 0, column }, focus: { row: rowCount - 1, column } }));
                  focusGrid();
                }}
              >
                {columnName(column)}
              </div>
            ))}
          </div>
        </div>
        <div className="rely-sheet-row-headers" style={{ top: HEADER_HEIGHT, width: HEADER_WIDTH }}>
          <div style={{ transform: `translateY(${-scroll.top}px)` }}>
            {Array.from({ length: visible.bottom - visible.top + 1 }, (_, offset) => visible.top + offset).map((row) => (
              <div
                key={row}
                role="rowheader"
                className={row >= selected.top && row <= selected.bottom ? 'rely-sheet-header rely-sheet-header-selected' : 'rely-sheet-header'}
                style={{ top: rows.offset(row), height: rows.size(row), width: HEADER_WIDTH }}
                onMouseDown={(event) => {
                  setSelection((current) => ({
                    anchor: event.shiftKey ? current.anchor : { row, column: 0 },
                    focus: { row, column: columnCount - 1 },
                  }));
                  focusGrid();
                }}
              >
                {row + 1}
              </div>
            ))}
          </div>
        </div>
        <div
          ref={viewport}
          className="rely-sheet-viewport"
          role="grid"
          aria-label={words.grid}
          aria-rowcount={rowCount}
          aria-colcount={columnCount}
          tabIndex={0}
          style={{ top: HEADER_HEIGHT, left: HEADER_WIDTH }}
          onScroll={(event) => setScroll({ top: event.currentTarget.scrollTop, left: event.currentTarget.scrollLeft })}
          onKeyDown={onKeyDown}
          onCopy={copySelection}
          onCut={(event) => {
            copySelection(event);
            if (editable) mutate(() => sheet.clearRange(workbook.index, selected));
          }}
          onPaste={paste}
        >
          <div className="rely-sheet-canvas" style={{ width: columns.offset(columnCount), height: rows.offset(rowCount) }}>
            {body}
            <div className="rely-sheet-selection" style={rectangle(selected)} />
            <div
              className="rely-sheet-active"
              style={rectangle(
                (() => {
                  const merge = coveredBy.get(`${selection.anchor.row}:${selection.anchor.column}`);
                  return merge
                    ? { top: merge.firstRow, left: merge.firstColumn, bottom: merge.lastRow, right: merge.lastColumn }
                    : { top: selection.anchor.row, left: selection.anchor.column, bottom: selection.anchor.row, right: selection.anchor.column };
                })(),
              )}
            />
            {editingCell && editingArea ? (
              <input
                className="rely-sheet-editor"
                aria-label={cellName(editingCell)}
                style={editingArea}
                value={editingCell.text}
                list={validationItems.length > 0 ? listId : undefined}
                autoFocus
                onChange={(event) => setEditing((current) => (current ? { ...current, text: event.target.value } : current))}
                onKeyDown={editorKeys}
                onBlur={() => {
                  if (closing.current) closing.current = false;
                  else commit();
                }}
              />
            ) : null}
            {validationItems.length > 0 ? (
              <datalist id={listId}>
                {validationItems.map((item) => (
                  <option key={item} value={item} />
                ))}
              </datalist>
            ) : null}
          </div>
        </div>
      </div>

      <div className="rely-sheet-tabs" role="tablist" aria-label={words.sheets}>
        {workbook.sheets.map((summary, index) =>
          renaming?.index === index ? (
            <input
              key={index}
              className="rely-sheet-tab-name"
              aria-label={words.renameSheet}
              value={renaming.name}
              autoFocus
              onChange={(event) => setRenaming({ index, name: event.target.value })}
              onKeyDown={(event) => {
                if (event.key === 'Escape') setRenaming(null);
                if (event.key !== 'Enter') return;
                if (mutate(() => sheet.renameSheet(index, renaming.name.trim()))) setRenaming(null);
              }}
              onBlur={() => setRenaming(null)}
            />
          ) : (
            <button
              key={index}
              type="button"
              role="tab"
              aria-selected={index === workbook.index}
              className="rely-sheet-tab"
              onClick={() => {
                setActiveSheet(index);
                setSelection({ anchor: { row: 0, column: 0 }, focus: { row: 0, column: 0 } });
                setEditing(null);
              }}
              onDoubleClick={() => editable && setRenaming({ index, name: summary.name })}
            >
              {summary.name}
            </button>
          ),
        )}
        {editable ? (
          <>
            <button
              type="button"
              className="rely-tool"
              aria-label={words.addSheet}
              title={words.addSheet}
              onClick={() => {
                const names = new Set(workbook.sheets.map((summary) => summary.name));
                let number = workbook.sheets.length + 1;
                while (names.has(words.newSheetName(number))) number += 1;
                let added = -1;
                if (mutate(() => (added = sheet.addSheet(words.newSheetName(number))))) setActiveSheet(added);
              }}
            >
              +
            </button>
            {workbook.sheets.length > 1 ? (
              <button
                type="button"
                className="rely-tool"
                aria-label={words.removeSheet}
                title={words.removeSheet}
                onClick={() => {
                  if (mutate(() => sheet.removeSheet(workbook.index))) setActiveSheet(Math.max(0, workbook.index - 1));
                }}
              >
                {'−'}
              </button>
            ) : null}
          </>
        ) : null}
      </div>
    </div>
  );
});
