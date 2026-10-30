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
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactElement,
  type Ref,
} from 'react';
import type {
  Border,
  CellComment,
  CellRange,
  CellStyle,
  CellWindow,
  ChartType,
  ConditionalLook,
  FilterChoice,
  SheetCell,
  SheetChart,
  SheetDocument,
  SheetPicture,
  SortKey,
  StylePatch,
} from '../model.js';
import { acceptFunction, functionTokenAt, suggestFunctions, type FunctionToken } from './formula-hints.js';
import {
  GridAxis,
  cellName,
  columnName,
  columnWidthCharacters,
  columnWidthPixels,
  coverMerges,
  fillTarget,
  parseCellName,
  parseTabSeparated,
  rowHeightPixels,
  rowHeightPoints,
  toTabSeparated,
  windowBetween,
  type CellPosition,
} from './geometry.js';
import { ENGLISH_SHEET_GRID_LABELS, type SheetGridLabels } from './labels.js';
import { PivotPanel } from './pivot-panel.js';
import { usePictureUrls } from './picture-urls.js';
import {
  BorderMenu,
  ChartPanel,
  CommentPanel,
  ConditionalFormatPanel,
  FilterMenu,
  NamesPanel,
  ValidationPanel,
} from './sheet-panels.js';
import { RelyOfficeError } from '../runtime/shared.js';

const DEFAULT_ROW_HEIGHT = rowHeightPixels(15);
const DEFAULT_COLUMN_WIDTH = columnWidthPixels(8.43);
const HEADER_HEIGHT = 24;
const HEADER_WIDTH = 48;
const EXTRA_ROWS = 50;
const EXTRA_COLUMNS = 10;
const MINIMUM_ROWS = 100;
const MINIMUM_COLUMNS = 26;
/** The smallest a dragged row or column gets; a size of its own hides it only from the menu. */
const MINIMUM_DRAGGED_SIZE = 6;
/** The rows an automatic column width looks at. */
const FIT_ROWS = 2_000;
const EMU_PER_PIXEL = 9_525;

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

interface Resizing {
  axis: 'row' | 'column';
  index: number;
  origin: number;
  base: number;
  size: number;
}

interface FunctionHints {
  token: FunctionToken;
  names: string[];
  active: number;
}

type Panel = 'conditional' | 'validation' | 'comments' | 'names' | 'charts' | 'pivots' | null;

/** A chart being dragged by its frame or its corner, and how far the pointer went. */
interface ChartDrag {
  index: number;
  mode: 'move' | 'resize';
  startX: number;
  startY: number;
  dx: number;
  dy: number;
}

/** A new chart's size: 4.8 by 2.9 inches, as Excel makes one. */
const NEW_CHART_EMU = { width: 4_389_120, height: 2_651_760 };

interface OpenFilter {
  column: number;
  choices: FilterChoice[];
  shown: string[] | null;
  position: { top: number; left: number };
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

/** A workbook edited together: a `SheetCollaboration`, whose `document` the grid is given as `sheet`. */
export interface SheetGridCollaboration {
  /** Called after another client changed the workbook. */
  subscribe: (listener: () => void) => () => void;
  /** Share the selection of this grid. */
  setSelection: (selection: { sheet: number; window: CellWindow } | null) => void;
  /** Where the others' selections are. */
  remoteSelections: () => { client: number; name: string; color: string; sheet: number; window: CellWindow }[];
  /** Called when the others' presence changes. */
  onPresence: (listener: () => void) => () => void;
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
  /** Who writes the comments added in the grid. */
  author?: string;
  /** Edit together: the grid redraws on the others' changes and shows where they are. */
  collaboration?: SheetGridCollaboration;
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

function contains(window: CellWindow, { row, column }: CellPosition): boolean {
  return row >= window.top && row <= window.bottom && column >= window.left && column <= window.right;
}

let measuring: CanvasRenderingContext2D | null | undefined;

/** The width a text takes in the grid's font, measured when the browser can, else estimated. */
function textWidth(text: string): number {
  if (measuring === undefined) {
    try {
      measuring = typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
    } catch {
      measuring = null;
    }
    if (measuring) measuring.font = '13px -apple-system, "Segoe UI", Helvetica, Arial, sans-serif';
  }
  return measuring ? measuring.measureText(text).width : text.length * 7;
}

/**
 * A spreadsheet grid over a `SheetDocument`: the sheets as tabs, a formula bar that suggests
 * functions, cells drawn with their styles, merges, conditional formats, validation and
 * pictures, frozen panes, filters, and the editing a spreadsheet user expects, from typing into
 * a cell to dragging its fill handle or the edge of a column.
 */
export const SheetGrid = forwardRef(function SheetGrid(
  { sheet, onChange, editable = true, labels, author, collaboration, className }: SheetGridProps,
  ref: Ref<SheetGridHandle>,
): ReactElement {
  const words = useMemo(() => ({ ...ENGLISH_SHEET_GRID_LABELS, ...labels }), [labels]);
  const [activeSheet, setActiveSheet] = useState(0);
  const [revision, refresh] = useReducer((value: number) => value + 1, 0);
  const [selection, setSelection] = useState<Selection>({ anchor: { row: 0, column: 0 }, focus: { row: 0, column: 0 } });
  const [presence, refreshPresence] = useReducer((value: number) => value + 1, 0);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const [editing, setEditing] = useState<Editing | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [scroll, setScroll] = useState({ top: 0, left: 0 });
  const [viewportSize, setViewportSize] = useState({ width: 960, height: 480 });
  const [renaming, setRenaming] = useState<{ index: number; name: string } | null>(null);
  const [resizing, setResizing] = useState<Resizing | null>(null);
  const [filling, setFilling] = useState<{ source: CellWindow; target: CellWindow } | null>(null);
  const [hints, setHints] = useState<FunctionHints | null>(null);
  const [panel, setPanel] = useState<Panel>(null);
  const [bordersOpen, setBordersOpen] = useState(false);
  const [openFilter, setOpenFilter] = useState<OpenFilter | null>(null);
  const [nameError, setNameError] = useState<string | null>(null);
  const [chosenChart, setChosenChart] = useState<number | null>(null);
  const [chartError, setChartError] = useState<string | null>(null);
  const [pivotError, setPivotError] = useState<string | null>(null);
  const [chartDrag, setChartDrag] = useState<ChartDrag | null>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const cellEditor = useRef<HTMLInputElement>(null);
  const barEditor = useRef<HTMLInputElement>(null);
  const pendingCaret = useRef<number | null>(null);
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
      comments: sheet.comments(index),
      charts: sheet.charts(index),
      commented: new Map(sheet.comments(index).map((comment) => [`${comment.row}:${comment.column}`, comment])),
    };
    // `revision` stands for every edit of the workbook, which changes in place.
  }, [sheet, activeSheet, revision]);

  const functionNames = useMemo(() => sheet.functionNames(), [sheet]);
  const frozenRows = workbook.summary.frozenRows ?? 0;
  const frozenColumns = workbook.summary.frozenColumns ?? 0;
  const filter = workbook.summary.autoFilter ?? null;

  const rows = useMemo(() => {
    const sizes: [number, number][] = workbook.summary.rowHeights.map(([row, points]) => [row, rowHeightPixels(points)]);
    if (resizing?.axis === 'row') sizes.push([resizing.index, resizing.size]);
    return new GridAxis(DEFAULT_ROW_HEIGHT, new Map(sizes));
  }, [workbook, resizing]);
  const columns = useMemo(() => {
    const sizes: [number, number][] = workbook.summary.columnWidths.map(([column, width]) => [column, columnWidthPixels(width)]);
    if (resizing?.axis === 'column') sizes.push([resizing.index, resizing.size]);
    return new GridAxis(DEFAULT_COLUMN_WIDTH, new Map(sizes));
  }, [workbook, resizing]);
  const frozenHeight = rows.offset(frozenRows);
  const frozenWidth = columns.offset(frozenColumns);

  const selected = useMemo(
    () => coverMerges(windowBetween(selection.anchor, selection.focus), workbook.merges),
    [selection, workbook.merges],
  );

  useEffect(() => {
    if (!collaboration) return;
    const stopChanges = collaboration.subscribe(() => {
      refresh();
      onChangeRef.current?.();
    });
    const stopPresence = collaboration.onPresence(refreshPresence);
    return () => {
      stopChanges();
      stopPresence();
    };
  }, [collaboration]);

  useEffect(() => {
    collaboration?.setSelection({ sheet: workbook.index, window: selected });
  }, [collaboration, workbook.index, selected]);

  const others = useMemo(
    () => (collaboration ? collaboration.remoteSelections().filter((other) => other.sheet === workbook.index) : []),
    // `presence` stands for every change of the others' selections.
    [collaboration, workbook.index, presence],
  );
  const rowCount = Math.max(MINIMUM_ROWS, Math.max(workbook.summary.rows, selected.bottom + 1) + EXTRA_ROWS);
  const columnCount = Math.max(MINIMUM_COLUMNS, Math.max(workbook.summary.columns, selected.right + 1) + EXTRA_COLUMNS);

  const visible: CellWindow = useMemo(() => {
    const top = Math.min(rows.indexAt(scroll.top + frozenHeight), rowCount - 1);
    const left = Math.min(columns.indexAt(scroll.left + frozenWidth), columnCount - 1);
    return {
      top: Math.max(top, frozenRows),
      left: Math.max(left, frozenColumns),
      bottom: Math.min(rowCount - 1, rows.indexAt(scroll.top + viewportSize.height) + 1),
      right: Math.min(columnCount - 1, columns.indexAt(scroll.left + viewportSize.width) + 1),
    };
  }, [rows, columns, scroll, viewportSize, rowCount, columnCount, frozenHeight, frozenWidth, frozenRows, frozenColumns]);

  const cells = useMemo(() => {
    const map = new Map<string, SheetCell>();
    const windows: CellWindow[] = [visible];
    if (frozenRows > 0) windows.push({ top: 0, bottom: frozenRows - 1, left: visible.left, right: visible.right });
    if (frozenColumns > 0) windows.push({ top: visible.top, bottom: visible.bottom, left: 0, right: frozenColumns - 1 });
    if (frozenRows > 0 && frozenColumns > 0) windows.push({ top: 0, bottom: frozenRows - 1, left: 0, right: frozenColumns - 1 });
    for (const window of windows) {
      for (const cell of sheet.cells(workbook.index, window)) map.set(`${cell.row}:${cell.column}`, cell);
    }
    return map;
  }, [sheet, workbook, visible, frozenRows, frozenColumns]);

  const cellAt = useCallback(
    ({ row, column }: CellPosition): SheetCell | undefined =>
      cells.get(`${row}:${column}`) ?? sheet.cells(workbook.index, { top: row, left: column, bottom: row, right: column })[0],
    [cells, sheet, workbook.index],
  );

  // A new `workbook` follows every edit, and a chart is drawn again from the values it changed.
  const pictures = useMemo(() => sheet.pictures(workbook.index), [sheet, workbook]);
  const pictureUrls = usePictureUrls(pictures, (position) => sheet.pictureBytes(workbook.index, position));
  // A chart is listed among the pictures, drawn, and among the charts, where it is named.
  const pictureName = (picture: SheetPicture): string => {
    const chart = workbook.charts.find(
      ({ placement }) =>
        placement.row === picture.row &&
        placement.column === picture.column &&
        placement.offsetX === picture.offsetXEmu &&
        placement.offsetY === picture.offsetYEmu &&
        placement.width === picture.widthEmu &&
        placement.height === picture.heightEmu,
    );
    if (!chart) return picture.description ?? '';
    return `${words.chartLabel} ${chart.spec?.title || picture.description || chart.index + 1}`;
  };

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

  useLayoutEffect(() => {
    if (pendingCaret.current === null) return;
    const input = editing?.source === 'bar' ? barEditor.current : cellEditor.current;
    input?.setSelectionRange(pendingCaret.current, pendingCaret.current);
    pendingCaret.current = null;
  });

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

  useEffect(() => {
    const release = () => {
      dragging.current = false;
      setFilling((current) => {
        if (current) {
          const { source, target } = current;
          const grew =
            source.top !== target.top || source.left !== target.left || source.bottom !== target.bottom || source.right !== target.right;
          if (grew && mutate(() => sheet.fillRange(workbook.index, source, target))) {
            setSelection({ anchor: { row: target.top, column: target.left }, focus: { row: target.bottom, column: target.right } });
          }
        }
        return null;
      });
    };
    window.addEventListener('mouseup', release);
    return () => window.removeEventListener('mouseup', release);
  }, [mutate, sheet, workbook.index]);

  useEffect(() => {
    if (!resizing) return;
    const move = (event: MouseEvent) => {
      const pointer = resizing.axis === 'column' ? event.clientX : event.clientY;
      setResizing((current) =>
        current ? { ...current, size: Math.max(MINIMUM_DRAGGED_SIZE, current.base + pointer - current.origin) } : current,
      );
    };
    const finish = () => {
      const { axis, index, size, base } = resizing;
      setResizing(null);
      if (size === base) return;
      mutate(() =>
        axis === 'column'
          ? sheet.setColumnWidth(workbook.index, index, columnWidthCharacters(size))
          : sheet.setRowHeight(workbook.index, index, rowHeightPoints(size)),
      );
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', finish);
    return () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', finish);
    };
  }, [resizing, mutate, sheet, workbook.index]);

  const scrollIntoView = useCallback(
    ({ row, column }: CellPosition) => {
      const element = viewport.current;
      if (!element) return;
      if (row >= frozenRows) {
        const top = rows.offset(row);
        const bottom = top + rows.size(row);
        if (top < element.scrollTop + frozenHeight) element.scrollTop = top - frozenHeight;
        else if (bottom > element.scrollTop + viewportSize.height) element.scrollTop = bottom - viewportSize.height;
      }
      if (column >= frozenColumns) {
        const left = columns.offset(column);
        const right = left + columns.size(column);
        if (left < element.scrollLeft + frozenWidth) element.scrollLeft = left - frozenWidth;
        else if (right > element.scrollLeft + viewportSize.width) element.scrollLeft = right - viewportSize.width;
      }
    },
    [rows, columns, viewportSize, frozenRows, frozenColumns, frozenHeight, frozenWidth],
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

  const updateHints = (text: string, caret: number | null) => {
    const token = caret === null ? null : functionTokenAt(text, caret);
    const names = token ? suggestFunctions(functionNames, token.prefix) : [];
    setHints(token && names.length > 0 ? { token, names, active: 0 } : null);
  };

  const typeInto = (text: string, caret: number | null) => {
    setEditing((current) => (current ? { ...current, text } : current));
    updateHints(text, caret);
  };

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
      setHints(null);
      if (move) moveTo({ row: row + move.rows, column: column + move.columns });
      focusGrid();
    },
    [editing, sheet, workbook.index, mutate, moveTo, words.invalidValue],
  );

  const startEditing = (text?: string) => {
    if (!editable) return;
    closing.current = false;
    const { anchor } = selection;
    const initial = text ?? sheet.input(workbook.index, anchor.row, anchor.column);
    setEditing({ ...anchor, text: initial, source: 'cell' });
    updateHints(initial, text === undefined ? null : initial.length);
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

  const filterWindow: CellWindow | null = filter ? { top: filter.top, left: filter.left, bottom: filter.bottom, right: filter.right } : null;
  const usedWindow: CellWindow = {
    top: 0,
    left: 0,
    bottom: Math.max(0, workbook.summary.rows - 1),
    right: Math.max(0, workbook.summary.columns - 1),
  };
  const single = selected.top === selected.bottom && selected.left === selected.right;

  /** Whether the first row of a window reads as its heading: text over other values, or bold. */
  const hasHeading = (window: CellWindow, column: number): boolean => {
    if (window.bottom <= window.top) return false;
    const heading = sheet.cells(workbook.index, { ...window, bottom: window.top });
    if (heading.length === 0 || heading.some((cell) => cell.kind !== 'text')) return false;
    if (heading.some((cell) => workbook.styles[cell.style]?.font.bold)) return true;
    const below = sheet.cells(workbook.index, { top: window.top + 1, bottom: window.bottom, left: column, right: column });
    return below.some((cell) => cell.kind !== 'text');
  };

  const sort = (descending: boolean, column = selection.anchor.column) => {
    const inFilter = filterWindow && contains(filterWindow, { row: selection.anchor.row, column });
    const window = inFilter ? filterWindow : single ? usedWindow : selected;
    const key: SortKey = { column: Math.min(Math.max(column, window.left), window.right), descending };
    mutate(() => sheet.sortRange(workbook.index, window, [key], inFilter ? true : hasHeading(window, key.column)));
  };

  const toggleFilter = () =>
    mutate(() => sheet.setAutoFilter(workbook.index, filter ? null : single ? usedWindow : selected));

  const toggleFreeze = () =>
    mutate(() => {
      if (frozenRows > 0 || frozenColumns > 0) sheet.setFrozen(workbook.index, 0, 0);
      else if (selection.anchor.row === 0 && selection.anchor.column === 0) sheet.setFrozen(workbook.index, 1, 0);
      else sheet.setFrozen(workbook.index, selection.anchor.row, selection.anchor.column);
    });

  const fitColumn = (column: number) => {
    const last = Math.min(Math.max(0, workbook.summary.rows - 1), FIT_ROWS);
    const found = sheet.cells(workbook.index, { top: 0, bottom: last, left: column, right: column });
    if (found.length === 0) {
      mutate(() => sheet.setColumnWidth(workbook.index, column, null));
      return;
    }
    const widest = Math.max(...found.map((cell) => textWidth(cell.display)));
    mutate(() => sheet.setColumnWidth(workbook.index, column, columnWidthCharacters(Math.ceil(widest) + 12)));
  };

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
    } else if (command && key === 'd' && editable && selected.bottom > selected.top) {
      const source = { ...selected, bottom: selected.top };
      mutate(() => sheet.fillRange(workbook.index, source, selected));
    } else if (command && key === 'r' && editable && selected.right > selected.left) {
      const source = { ...selected, right: selected.left };
      mutate(() => sheet.fillRange(workbook.index, source, selected));
    } else if (event.key in steps) {
      const [rowStep, columnStep] = steps[event.key];
      moveTo({ row: from.row + rowStep, column: from.column + columnStep }, event.shiftKey);
    } else if (event.key === 'Tab' || event.key === 'Enter') {
      const backwards = event.shiftKey;
      const step = event.key === 'Tab' ? { row: 0, column: backwards ? -1 : 1 } : { row: backwards ? -1 : 1, column: 0 };
      moveTo({ row: anchor.row + step.row, column: anchor.column + step.column });
    } else if (event.key === 'F2') {
      startEditing();
    } else if ((event.key === 'Delete' || event.key === 'Backspace') && editable && chosenChart !== null) {
      const index = chosenChart;
      setChosenChart(null);
      mutate(() => sheet.removeChart(workbook.index, index));
    } else if (event.key === 'Escape' && chosenChart !== null) {
      setChosenChart(null);
    } else if ((event.key === 'Delete' || event.key === 'Backspace') && editable) {
      mutate(() => sheet.clearRange(workbook.index, selected));
    } else if (command && ['b', 'i', 'u'].includes(key) && editable) {
      toggle(({ b: 'bold', i: 'italic', u: 'underline' } as const)[key as 'b' | 'i' | 'u']);
    } else if (event.key.length === 1 && !command && !event.altKey) {
      startEditing(event.key);
    } else {
      return;
    }
    event.preventDefault();
  };

  /** Run a change to the pivot tables, keeping why it was refused for the pivot panel. */
  const changePivot = (change: () => void) => {
    try {
      change();
    } catch (error) {
      setPivotError(error instanceof Error ? error.message : String(error));
      return;
    }
    setPivotError(null);
    refresh();
    onChange?.();
  };

  /** Run a change to the charts, keeping why it was refused for the chart panel. */
  const changeChart = (change: () => void): boolean => {
    try {
      change();
    } catch (error) {
      setChartError(error instanceof Error ? error.message : String(error));
      return false;
    }
    setChartError(null);
    refresh();
    onChange?.();
    return true;
  };

  const insertChart = (type: ChartType) => {
    const made = sheet.chartForRange(workbook.index, selected, type);
    if (!made) {
      setChartError(words.chartNothingToPlot);
      return;
    }
    const placement = { row: selected.top, column: selected.right + 2, offsetX: 0, offsetY: 0, ...NEW_CHART_EMU };
    let index = -1;
    if (changeChart(() => (index = sheet.addChart(workbook.index, made, placement)))) setChosenChart(index);
  };

  const startChartDrag = (event: ReactPointerEvent<HTMLElement>, index: number, mode: ChartDrag['mode']) => {
    if (event.button !== 0) return;
    event.stopPropagation();
    event.preventDefault();
    if (editing) commit();
    setChosenChart(index);
    event.currentTarget.setPointerCapture?.(event.pointerId);
    setChartDrag({ index, mode, startX: event.clientX, startY: event.clientY, dx: 0, dy: 0 });
  };

  const moveChartDrag = (event: ReactPointerEvent<HTMLElement>) => {
    setChartDrag((current) =>
      current ? { ...current, dx: event.clientX - current.startX, dy: event.clientY - current.startY } : current,
    );
  };

  const endChartDrag = (event: ReactPointerEvent<HTMLElement>, chart: SheetChart) => {
    const drag = chartDrag;
    setChartDrag(null);
    if (!drag || drag.index !== chart.index || (Math.abs(drag.dx) < 3 && Math.abs(drag.dy) < 3)) return;
    event.stopPropagation();
    const { placement } = chart;
    if (drag.mode === 'resize') {
      const width = Math.max(16, placement.width / EMU_PER_PIXEL + drag.dx);
      const height = Math.max(16, placement.height / EMU_PER_PIXEL + drag.dy);
      changeChart(() =>
        sheet.setChart(workbook.index, chart.index, {
          placement: { ...placement, width: Math.round(width * EMU_PER_PIXEL), height: Math.round(height * EMU_PER_PIXEL) },
        }),
      );
      return;
    }
    const left = Math.max(0, columns.offset(placement.column) + placement.offsetX / EMU_PER_PIXEL + drag.dx);
    const top = Math.max(0, rows.offset(placement.row) + placement.offsetY / EMU_PER_PIXEL + drag.dy);
    const column = columns.indexAt(left);
    const row = rows.indexAt(top);
    changeChart(() =>
      sheet.setChart(workbook.index, chart.index, {
        placement: {
          ...placement,
          row,
          column,
          offsetX: Math.round((left - columns.offset(column)) * EMU_PER_PIXEL),
          offsetY: Math.round((top - rows.offset(row)) * EMU_PER_PIXEL),
        },
      }),
    );
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

  const openFilterMenu = (event: ReactMouseEvent, column: number) => {
    event.stopPropagation();
    if (!filter) return;
    const offset = column - filter.left;
    const shown = filter.columns.find((entry) => entry.column === offset)?.shown ?? null;
    let choices: FilterChoice[] = [];
    try {
      choices = sheet.filterChoices(workbook.index, offset);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
      return;
    }
    const left = HEADER_WIDTH + columns.offset(column) - (column >= frozenColumns ? scroll.left : 0);
    const top = HEADER_HEIGHT + rows.offset(filter.top + 1) - (filter.top >= frozenRows ? scroll.top : 0);
    setOpenFilter({ column, choices, shown, position: { top, left: Math.max(0, left) } });
  };

  const renderCell = (row: number, column: number, span?: CellRange): ReactElement => {
    const key = `${row}:${column}`;
    const cell = cells.get(key) ?? (span ? cellAt({ row, column }) : undefined);
    const style = cell ? workbook.styles[cell.style] : undefined;
    const look = workbook.looks.get(key);
    const area = span
      ? rectangle({ top: span.firstRow, left: span.firstColumn, bottom: span.lastRow, right: span.lastColumn })
      : rectangle({ top: row, left: column, bottom: row, right: column });
    const showValue = !look?.bar && !look?.icon ? true : (look.bar?.showValue ?? look.icon?.showValue ?? true);
    const filterButton = filter !== null && row === filter.top && column >= filter.left && column <= filter.right;
    const filtered = filterButton && filter.columns.some((entry) => entry.column === column - filter.left);
    const comment = workbook.commented.get(key);
    return (
      <div
        key={key}
        role="gridcell"
        aria-label={cellName({ row, column })}
        aria-selected={contains(selected, { row, column })}
        aria-description={comment ? comment.text : undefined}
        title={comment ? commentTitle(comment) : undefined}
        className={[
          'rely-sheet-cell',
          workbook.invalid.has(key) ? 'rely-sheet-invalid' : '',
          comment ? 'rely-sheet-commented' : '',
        ]
          .filter(Boolean)
          .join(' ')}
        style={{ ...area, ...cellCss(cell, style, look) }}
        onMouseDown={(event) => {
          if (event.button !== 0) return;
          if (editing) commit();
          moveTo({ row, column }, event.shiftKey);
          dragging.current = true;
          focusGrid();
        }}
        onMouseEnter={() => {
          if (filling) setFilling((current) => (current ? { ...current, target: fillTarget(current.source, { row, column }) } : current));
          else if (dragging.current) setSelection((current) => ({ anchor: current.anchor, focus: { row, column } }));
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
        {filterButton ? (
          <button
            type="button"
            className="rely-sheet-filter-button"
            aria-label={`${words.filterColumn} ${columnName(column)}`}
            aria-pressed={filtered}
            onMouseDown={(event) => event.stopPropagation()}
            onClick={(event) => openFilterMenu(event, column)}
          >
            {filtered ? '⧩' : '▾'}
          </button>
        ) : null}
      </div>
    );
  };

  /** The cells of a window, merges drawn once from their top-left cell. */
  const renderCells = (window: CellWindow): ReactElement[] => {
    const drawn: ReactElement[] = [];
    for (let row = window.top; row <= window.bottom; row += 1) {
      for (let column = window.left; column <= window.right; column += 1) {
        if (coveredBy.has(`${row}:${column}`)) continue;
        drawn.push(renderCell(row, column));
      }
    }
    for (const merge of workbook.merges) {
      const overlaps =
        merge.firstRow <= window.bottom && merge.lastRow >= window.top && merge.firstColumn <= window.right && merge.lastColumn >= window.left;
      if (overlaps) drawn.push(renderCell(merge.firstRow, merge.firstColumn, merge));
    }
    return drawn;
  };

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
    single ? anchorName : `${cellName({ row: selected.top, column: selected.left })}:${cellName({ row: selected.bottom, column: selected.right })}`;

  const acceptHint = (name: string) => {
    if (!editing || !hints) return;
    const accepted = acceptFunction(editing.text, hints.token, name);
    pendingCaret.current = accepted.caret;
    setEditing({ ...editing, text: accepted.text });
    setHints(null);
  };

  const editorKeys = (event: KeyboardEvent<HTMLInputElement>) => {
    if (hints) {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        const delta = event.key === 'ArrowDown' ? 1 : -1;
        setHints({ ...hints, active: (hints.active + delta + hints.names.length) % hints.names.length });
        return;
      }
      if (event.key === 'Tab' || (event.key === 'Enter' && !event.shiftKey)) {
        event.preventDefault();
        acceptHint(hints.names[hints.active]);
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        setHints(null);
        return;
      }
    }
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
      setHints(null);
      setMessage(null);
      focusGrid();
    }
  };

  const hintList = (style: CSSProperties) =>
    hints ? (
      <ul className="rely-sheet-hints" role="listbox" aria-label={words.functions} style={style}>
        {hints.names.map((name, position) => (
          <li
            key={name}
            role="option"
            aria-selected={position === hints.active}
            onMouseDown={(event) => {
              event.preventDefault();
              acceptHint(name);
            }}
          >
            {name}
          </li>
        ))}
      </ul>
    ) : null;

  const activeArea = (() => {
    const merge = coveredBy.get(`${selection.anchor.row}:${selection.anchor.column}`);
    return merge
      ? { top: merge.firstRow, left: merge.firstColumn, bottom: merge.lastRow, right: merge.lastColumn }
      : { top: selection.anchor.row, left: selection.anchor.column, bottom: selection.anchor.row, right: selection.anchor.column };
  })();

  /** What every pane draws over its cells: the selection, the fill handle and the editor. */
  const overlays = (pane: 'main' | 'top' | 'left' | 'corner'): ReactElement => {
    const paneOf = ({ row, column }: CellPosition) =>
      row < frozenRows ? (column < frozenColumns ? 'corner' : 'top') : column < frozenColumns ? 'left' : 'main';
    const selectionRectangle = rectangle(selected);
    const handleHere = editable && !editing && paneOf({ row: selected.bottom, column: selected.right }) === pane;
    return (
      <>
        {others.map((other) => (
          <div
            key={other.client}
            className="rely-sheet-remote"
            style={{ ...rectangle(other.window), borderColor: other.color }}
            title={other.name}
          >
            <span className="rely-sheet-remote-name" style={{ background: other.color }}>
              {other.name}
            </span>
          </div>
        ))}
        <div className="rely-sheet-selection" style={selectionRectangle} />
        <div className="rely-sheet-active" style={rectangle(activeArea)} />
        {filling ? <div className="rely-sheet-fill-target" style={rectangle(filling.target)} /> : null}
        {handleHere ? (
          <div
            className="rely-sheet-fill-handle"
            title={words.fillHandle}
            style={{
              top: rows.offset(selected.bottom + 1) - 4,
              left: columns.offset(selected.right + 1) - 4,
            }}
            onMouseDown={(event) => {
              event.preventDefault();
              event.stopPropagation();
              setFilling({ source: selected, target: selected });
            }}
          />
        ) : null}
        {editingCell && editingArea && paneOf(editingCell) === pane ? (
          <>
            <input
              ref={cellEditor}
              className="rely-sheet-editor"
              aria-label={cellName(editingCell)}
              style={editingArea}
              value={editingCell.text}
              list={validationItems.length > 0 ? listId : undefined}
              autoFocus
              onChange={(event) => typeInto(event.target.value, event.target.selectionStart)}
              onSelect={(event) => updateHints(event.currentTarget.value, event.currentTarget.selectionStart)}
              onKeyDown={editorKeys}
              onBlur={() => {
                setHints(null);
                if (closing.current) closing.current = false;
                else commit();
              }}
            />
            {hintList({
              top: Number(editingArea.top) + Number(editingArea.height),
              left: editingArea.left,
            })}
          </>
        ) : null}
      </>
    );
  };

  const canvasSize = { width: columns.offset(columnCount), height: rows.offset(rowCount) };

  const columnHeader = (column: number) => (
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
      {editable ? (
        <span
          className="rely-sheet-resize-column"
          title={words.resizeColumn}
          onMouseDown={(event) => {
            event.preventDefault();
            event.stopPropagation();
            const size = columns.size(column);
            setResizing({ axis: 'column', index: column, origin: event.clientX, base: size, size });
          }}
          onDoubleClick={(event) => {
            event.stopPropagation();
            fitColumn(column);
          }}
        />
      ) : null}
    </div>
  );

  const rowHeader = (row: number) => (
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
      {editable ? (
        <span
          className="rely-sheet-resize-row"
          title={words.resizeRow}
          onMouseDown={(event) => {
            event.preventDefault();
            event.stopPropagation();
            const size = rows.size(row);
            setResizing({ axis: 'row', index: row, origin: event.clientY, base: size, size });
          }}
          onDoubleClick={(event) => {
            event.stopPropagation();
            mutate(() => sheet.setRowHeight(workbook.index, row, null));
          }}
        />
      ) : null}
    </div>
  );

  const range = (from: number, to: number) => Array.from({ length: Math.max(0, to - from + 1) }, (_, offset) => from + offset);
  const frozen = frozenRows > 0 || frozenColumns > 0;

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
            <span className="rely-sheet-menu-anchor">
              <button
                type="button"
                className="rely-tool"
                aria-label={words.borders}
                title={words.borders}
                aria-expanded={bordersOpen}
                onClick={() => setBordersOpen((open) => !open)}
              >
                {'▦'}
              </button>
              {bordersOpen ? (
                <BorderMenu
                  words={words}
                  onApply={(placement, border) => {
                    mutate(() => sheet.setBorders(workbook.index, selected, placement, border));
                    setBordersOpen(false);
                  }}
                  onClose={() => setBordersOpen(false)}
                />
              ) : null}
            </span>
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
            <button type="button" className="rely-tool" aria-label={words.sortAscending} title={words.sortAscending} onClick={() => sort(false)}>
              {'A↓'}
            </button>
            <button type="button" className="rely-tool" aria-label={words.sortDescending} title={words.sortDescending} onClick={() => sort(true)}>
              {'Z↓'}
            </button>
            <button
              type="button"
              className="rely-tool"
              aria-label={words.filter}
              title={words.filter}
              aria-pressed={filter !== null}
              onClick={toggleFilter}
            >
              {'⧩'}
            </button>
            <button
              type="button"
              className="rely-tool"
              aria-label={frozen ? words.unfreeze : words.freeze}
              title={frozen ? words.unfreeze : words.freeze}
              aria-pressed={frozen}
              onClick={toggleFreeze}
            >
              {'⊞'}
            </button>
            <button
              type="button"
              className="rely-tool"
              aria-label={words.conditionalFormats}
              title={words.conditionalFormats}
              aria-pressed={panel === 'conditional'}
              onClick={() => setPanel((current) => (current === 'conditional' ? null : 'conditional'))}
            >
              {'◐'}
            </button>
            <button
              type="button"
              className="rely-tool"
              aria-label={words.validation}
              title={words.validation}
              aria-pressed={panel === 'validation'}
              onClick={() => setPanel((current) => (current === 'validation' ? null : 'validation'))}
            >
              {'✓'}
            </button>
            <button
              type="button"
              className="rely-tool"
              aria-label={words.comments}
              title={words.comments}
              aria-pressed={panel === 'comments'}
              onClick={() => setPanel((current) => (current === 'comments' ? null : 'comments'))}
            >
              {'✎'}
            </button>
            <button
              type="button"
              className="rely-tool"
              aria-label={words.charts}
              title={words.charts}
              aria-pressed={panel === 'charts'}
              onClick={() => {
                setChartError(null);
                setPanel((current) => (current === 'charts' ? null : 'charts'));
              }}
            >
              {'▥'}
            </button>
            <button
              type="button"
              className="rely-tool"
              aria-label={words.pivotTables}
              title={words.pivotTables}
              aria-pressed={panel === 'pivots'}
              onClick={() => {
                setPivotError(null);
                setPanel((current) => (current === 'pivots' ? null : 'pivots'));
              }}
            >
              {'⊞'}
            </button>
            <button
              type="button"
              className="rely-tool"
              aria-label={words.definedNames}
              title={words.definedNames}
              aria-pressed={panel === 'names'}
              onClick={() => {
                setNameError(null);
                setPanel((current) => (current === 'names' ? null : 'names'));
              }}
            >
              {'N'}
            </button>
          </div>
          <div className="rely-tool-group">
            <button type="button" className="rely-tool" aria-label={words.merge} title={words.merge} onClick={() => mutate(() => sheet.mergeCells(workbook.index, selected))}>
              {'⬚'}
            </button>
            <button type="button" className="rely-tool" aria-label={words.unmerge} title={words.unmerge} onClick={() => mutate(() => sheet.unmergeCells(workbook.index, selected))}>
              {'⊟'}
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
        <span className="rely-sheet-formula-wrap">
          <input
            ref={barEditor}
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
            onChange={(event) => typeInto(event.target.value, event.target.selectionStart)}
            onSelect={(event) => {
              if (editing?.source === 'bar') updateHints(event.currentTarget.value, event.currentTarget.selectionStart);
            }}
            onBlur={() => {
              if (editing?.source === 'bar') setHints(null);
            }}
            onKeyDown={editorKeys}
          />
          {editing?.source === 'bar' ? hintList({ top: '100%', left: 0 }) : null}
        </span>
      </div>
      {message ? (
        <div className="rely-sheet-message" role="alert">
          {message}
        </div>
      ) : null}

      <div className="rely-sheet-body">
        <div className="rely-sheet-frame">
          <div className="rely-sheet-corner" style={{ width: HEADER_WIDTH, height: HEADER_HEIGHT }} />
          <div className="rely-sheet-column-headers" style={{ left: HEADER_WIDTH, height: HEADER_HEIGHT }}>
            <div style={{ transform: `translateX(${-scroll.left}px)` }}>{range(visible.left, visible.right).map(columnHeader)}</div>
            {frozenColumns > 0 ? (
              <div className="rely-sheet-frozen-headers" style={{ width: frozenWidth, height: HEADER_HEIGHT }}>
                {range(0, frozenColumns - 1).map(columnHeader)}
              </div>
            ) : null}
          </div>
          <div className="rely-sheet-row-headers" style={{ top: HEADER_HEIGHT, width: HEADER_WIDTH }}>
            <div style={{ transform: `translateY(${-scroll.top}px)` }}>{range(visible.top, visible.bottom).map(rowHeader)}</div>
            {frozenRows > 0 ? (
              <div className="rely-sheet-frozen-headers" style={{ width: HEADER_WIDTH, height: frozenHeight }}>
                {range(0, frozenRows - 1).map(rowHeader)}
              </div>
            ) : null}
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
            onScroll={(event) => {
              setScroll({ top: event.currentTarget.scrollTop, left: event.currentTarget.scrollLeft });
              setOpenFilter(null);
            }}
            onKeyDown={onKeyDown}
            onCopy={copySelection}
            onCut={(event) => {
              copySelection(event);
              if (editable) mutate(() => sheet.clearRange(workbook.index, selected));
            }}
            onPaste={paste}
          >
            <div className="rely-sheet-canvas" style={canvasSize}>
              {renderCells(visible)}
              {pictures.map((picture, position) => (
                <img
                  key={position}
                  className="rely-sheet-picture"
                  alt={pictureName(picture)}
                  src={pictureUrls[position] || undefined}
                  style={{
                    top: rows.offset(picture.row) + picture.offsetYEmu / EMU_PER_PIXEL,
                    left: columns.offset(picture.column) + picture.offsetXEmu / EMU_PER_PIXEL,
                    width: picture.widthEmu / EMU_PER_PIXEL,
                    height: picture.heightEmu / EMU_PER_PIXEL,
                  }}
                />
              ))}
              {editable
                ? workbook.charts.map((chart) => {
                    const moving = chartDrag?.index === chart.index ? chartDrag : null;
                    const width = chart.placement.width / EMU_PER_PIXEL;
                    const height = chart.placement.height / EMU_PER_PIXEL;
                    return (
                      <div
                        key={chart.index}
                        role="button"
                        tabIndex={-1}
                        aria-label={`${words.chartLabel} ${chart.spec?.title || chart.index + 1}`}
                        aria-pressed={chosenChart === chart.index}
                        className={['rely-sheet-chart-frame', chosenChart === chart.index ? 'rely-sheet-chart-chosen' : '']
                          .filter(Boolean)
                          .join(' ')}
                        style={{
                          top: rows.offset(chart.placement.row) + chart.placement.offsetY / EMU_PER_PIXEL + (moving?.mode === 'move' ? moving.dy : 0),
                          left:
                            columns.offset(chart.placement.column) +
                            chart.placement.offsetX / EMU_PER_PIXEL +
                            (moving?.mode === 'move' ? moving.dx : 0),
                          width: Math.max(16, width + (moving?.mode === 'resize' ? moving.dx : 0)),
                          height: Math.max(16, height + (moving?.mode === 'resize' ? moving.dy : 0)),
                        }}
                        onPointerDown={(event) => startChartDrag(event, chart.index, 'move')}
                        onPointerMove={moveChartDrag}
                        onPointerUp={(event) => endChartDrag(event, chart)}
                        onDoubleClick={() => {
                          setChosenChart(chart.index);
                          setPanel('charts');
                        }}
                      >
                        {chosenChart === chart.index ? (
                          <span
                            className="rely-sheet-chart-resize"
                            aria-label={words.resizeChart}
                            title={words.resizeChart}
                            onPointerDown={(event) => startChartDrag(event, chart.index, 'resize')}
                          />
                        ) : null}
                      </div>
                    );
                  })
                : null}
              {overlays('main')}
              {validationItems.length > 0 ? (
                <datalist id={listId}>
                  {validationItems.map((item) => (
                    <option key={item} value={item} />
                  ))}
                </datalist>
              ) : null}
            </div>
          </div>
          {frozenRows > 0 ? (
            <div
              className="rely-sheet-pane rely-sheet-pane-top"
              style={{ top: HEADER_HEIGHT, left: HEADER_WIDTH, width: viewportSize.width, height: frozenHeight }}
            >
              <div className="rely-sheet-canvas" style={{ ...canvasSize, transform: `translateX(${-scroll.left}px)` }}>
                {renderCells({ top: 0, bottom: frozenRows - 1, left: visible.left, right: visible.right })}
                {overlays('top')}
              </div>
            </div>
          ) : null}
          {frozenColumns > 0 ? (
            <div
              className="rely-sheet-pane rely-sheet-pane-left"
              style={{ top: HEADER_HEIGHT, left: HEADER_WIDTH, width: frozenWidth, height: viewportSize.height }}
            >
              <div className="rely-sheet-canvas" style={{ ...canvasSize, transform: `translateY(${-scroll.top}px)` }}>
                {renderCells({ top: visible.top, bottom: visible.bottom, left: 0, right: frozenColumns - 1 })}
                {overlays('left')}
              </div>
            </div>
          ) : null}
          {frozenRows > 0 && frozenColumns > 0 ? (
            <div
              className="rely-sheet-pane rely-sheet-pane-top rely-sheet-pane-left"
              style={{ top: HEADER_HEIGHT, left: HEADER_WIDTH, width: frozenWidth, height: frozenHeight }}
            >
              <div className="rely-sheet-canvas" style={canvasSize}>
                {renderCells({ top: 0, bottom: frozenRows - 1, left: 0, right: frozenColumns - 1 })}
                {overlays('corner')}
              </div>
            </div>
          ) : null}
          {openFilter ? (
            <FilterMenu
              key={openFilter.column}
              words={words}
              choices={openFilter.choices}
              shown={openFilter.shown}
              position={openFilter.position}
              onClose={() => setOpenFilter(null)}
              onApply={(shown) => {
                if (filter) mutate(() => sheet.setFilterColumn(workbook.index, openFilter.column - filter.left, shown));
                setOpenFilter(null);
              }}
              onSort={(descending) => {
                if (filterWindow) {
                  mutate(() => sheet.sortRange(workbook.index, filterWindow, [{ column: openFilter.column, descending }], true));
                }
                setOpenFilter(null);
              }}
            />
          ) : null}
        </div>
        {panel === 'conditional' ? (
          <ConditionalFormatPanel
            words={words}
            selection={selected}
            formats={sheet.conditionalFormats(workbook.index)}
            onChange={(formats) => mutate(() => sheet.setConditionalFormats(workbook.index, formats))}
            onClose={() => setPanel(null)}
          />
        ) : null}
        {panel === 'validation' ? (
          <ValidationPanel
            words={words}
            selection={selected}
            validations={sheet.validations(workbook.index)}
            onChange={(validations) => mutate(() => sheet.setValidations(workbook.index, validations))}
            onClose={() => setPanel(null)}
          />
        ) : null}
        {panel === 'comments' ? (
          <CommentPanel
            words={words}
            selection={selected}
            comments={workbook.comments}
            onChange={(row, column, text) => mutate(() => sheet.setComment(workbook.index, row, column, text, author ?? ''))}
            onStart={(row, column, text) =>
              mutate(() =>
                sheet.startCommentThread(workbook.index, row, column, text, author ?? '', new Date().toISOString()),
              )
            }
            onReply={(row, column, text) =>
              mutate(() =>
                sheet.replyToComment(workbook.index, row, column, text, author ?? '', new Date().toISOString()),
              )
            }
            onResolve={(row, column, resolved) =>
              mutate(() => sheet.setCommentResolved(workbook.index, row, column, resolved))
            }
            onRemoveReply={(row, column, reply) =>
              mutate(() => sheet.removeCommentReply(workbook.index, row, column, reply))
            }
            onSelect={(row, column) => moveTo({ row, column }, false)}
            onClose={() => setPanel(null)}
          />
        ) : null}
        {panel === 'charts' ? (
          <ChartPanel
            words={words}
            charts={workbook.charts}
            chosen={chosenChart !== null && chosenChart < workbook.charts.length ? chosenChart : null}
            error={chartError}
            onChoose={setChosenChart}
            onInsert={insertChart}
            onChange={(index, chart) => changeChart(() => sheet.setChart(workbook.index, index, { chart }))}
            onUseSelection={(index) => {
              const current = workbook.charts[index]?.spec;
              const made = sheet.chartForRange(workbook.index, selected, current?.chartType ?? 'column');
              if (!made) {
                setChartError(words.chartNothingToPlot);
                return;
              }
              changeChart(() =>
                sheet.setChart(workbook.index, index, {
                  chart: { ...made, title: current?.title ?? null, legend: current?.legend ?? made.legend, stacked: current?.stacked ?? false },
                }),
              );
            }}
            onRemove={(index) => {
              setChosenChart(null);
              changeChart(() => sheet.removeChart(workbook.index, index));
            }}
            onClose={() => setPanel(null)}
          />
        ) : null}
        {panel === 'pivots' ? (
          <PivotPanel
            words={words}
            selection={selected}
            sheetIndex={workbook.index}
            tables={sheet.pivotTables(workbook.index)}
            fieldsOf={(source, window) => {
              if (window.bottom <= window.top) return null;
              try {
                return sheet.pivotSourceFields(source, window);
              } catch {
                return null;
              }
            }}
            error={pivotError}
            onCreate={(definition) => changePivot(() => sheet.addPivotTable(workbook.index, definition))}
            onChange={(index, definition) => changePivot(() => sheet.setPivotTable(workbook.index, index, definition))}
            onRefresh={(index) => changePivot(() => sheet.refreshPivotTable(workbook.index, index))}
            onRefreshAll={() => changePivot(() => sheet.refreshPivotTables())}
            onRemove={(index) => changePivot(() => sheet.removePivotTable(workbook.index, index))}
            onSelect={(row, column) => moveTo({ row, column }, false)}
            onClose={() => setPanel(null)}
          />
        ) : null}
        {panel === 'names' ? (
          <NamesPanel
            words={words}
            sheets={workbook.sheets.map((summary) => summary.name)}
            names={sheet.definedNames()}
            error={nameError}
            onDefine={(name, formula, scope) => {
              try {
                sheet.setDefinedName(name, formula, scope);
                setNameError(null);
              } catch (error) {
                if (error instanceof RelyOfficeError && error.code === 'INVALID_NAME') setNameError(words.invalidName);
                else setNameError(error instanceof Error ? error.message : String(error));
                return;
              }
              refresh();
              onChange?.();
            }}
            onRemove={(name, scope) => mutate(() => sheet.removeDefinedName(name, scope))}
            onClose={() => setPanel(null)}
          />
        ) : null}
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
                setOpenFilter(null);
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

/** What pointing at a commented cell shows: the comment and each reply under its author. */
function commentTitle(comment: CellComment): string {
  return [comment, ...comment.replies]
    .map((entry) => (entry.author ? `${entry.author}: ${entry.text}` : entry.text))
    .join('\n');
}
