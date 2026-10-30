// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { useState, type ReactElement } from 'react';
import type {
  CellWindow,
  PivotFunction,
  PivotSourceField,
  PivotTableDefinition,
  SheetPivotTable,
} from '../model.js';
import { cellName } from './geometry.js';
import type { SheetGridLabels } from './labels.js';

/** Where a field of the source goes in a pivot table. */
export type PivotRole = 'none' | 'rows' | 'columns' | 'values' | 'filters';

const ROLES: [PivotRole, keyof SheetGridLabels][] = [
  ['none', 'pivotRoleNone'],
  ['rows', 'pivotRoleRows'],
  ['columns', 'pivotRoleColumns'],
  ['values', 'pivotRoleValues'],
  ['filters', 'pivotRoleFilters'],
];

export const PIVOT_FUNCTIONS: [PivotFunction, keyof SheetGridLabels][] = [
  ['sum', 'pivotSum'],
  ['count', 'pivotCount'],
  ['average', 'pivotAverage'],
  ['max', 'pivotMax'],
  ['min', 'pivotMin'],
  ['product', 'pivotProduct'],
  ['countNumbers', 'pivotCountNumbers'],
  ['stdDev', 'pivotStdDev'],
  ['stdDevP', 'pivotStdDevP'],
  ['var', 'pivotVar'],
  ['varP', 'pivotVarP'],
];

/** What the panel's form holds: the role and function of each source field. */
export interface PivotDraft {
  roles: PivotRole[];
  functions: PivotFunction[];
  rowGrandTotals: boolean;
  columnGrandTotals: boolean;
  anchor: string;
}

/** The draft of an existing table, or of a new one over `fields` with nothing placed. */
export function pivotDraft(fields: number, table?: SheetPivotTable, anchor = ''): PivotDraft {
  const roles: PivotRole[] = Array.from({ length: fields }, () => 'none');
  const functions: PivotFunction[] = Array.from({ length: fields }, () => 'sum');
  if (table) {
    table.rows.forEach(({ field }) => (roles[field] = 'rows'));
    table.columns.forEach(({ field }) => (roles[field] = 'columns'));
    table.filters.forEach(({ field }) => (roles[field] = 'filters'));
    table.values.forEach(({ field, function: summary }) => {
      if (roles[field] === 'none') roles[field] = 'values';
      functions[field] = summary ?? 'sum';
    });
  }
  return {
    roles,
    functions,
    rowGrandTotals: table?.rowGrandTotals ?? true,
    columnGrandTotals: table?.columnGrandTotals ?? true,
    anchor: table ? cellName(table.anchor) : anchor,
  };
}

/** A1 notation as a cell, or null for anything else. */
export function parseCellName(text: string): { row: number; column: number } | null {
  const match = /^\s*\$?([A-Za-z]{1,3})\$?(\d{1,7})\s*$/.exec(text);
  if (!match) return null;
  const column = [...match[1].toUpperCase()].reduce((total, letter) => total * 26 + letter.charCodeAt(0) - 64, 0) - 1;
  const row = Number(match[2]) - 1;
  return row >= 0 && column >= 0 ? { row, column } : null;
}

/**
 * The definition a draft makes, the hidden items of a table being changed kept; null when the
 * anchor is no cell.
 */
export function pivotDefinition(
  draft: PivotDraft,
  sourceSheet: number,
  source: CellWindow,
  table?: SheetPivotTable,
): PivotTableDefinition | null {
  const anchor = parseCellName(draft.anchor);
  if (!anchor) return null;
  const hidden = (field: number) =>
    [...(table?.rows ?? []), ...(table?.columns ?? []), ...(table?.filters ?? [])].find((placed) => placed.field === field)
      ?.hidden ?? [];
  const placed = (role: PivotRole) =>
    draft.roles.flatMap((each, field) => (each === role ? [{ field, hidden: hidden(field) }] : []));
  return {
    ...(table ? { name: table.name } : {}),
    sourceSheet,
    source,
    anchor,
    rows: placed('rows'),
    columns: placed('columns'),
    filters: placed('filters'),
    values: draft.roles.flatMap((role, field) => (role === 'values' ? [{ field, function: draft.functions[field] }] : [])),
    rowGrandTotals: draft.rowGrandTotals,
    columnGrandTotals: draft.columnGrandTotals,
  };
}

/**
 * The pivot tables of a sheet: a table made from the selection, the fields of the chosen one
 * placed on its rows, columns, values and filters, and each refreshed or removed.
 */
export function PivotPanel({
  words,
  selection,
  sheetIndex,
  tables,
  fieldsOf,
  error,
  onCreate,
  onChange,
  onRefresh,
  onRefreshAll,
  onRemove,
  onSelect,
  onClose,
}: {
  words: SheetGridLabels;
  selection: CellWindow;
  sheetIndex: number;
  tables: SheetPivotTable[];
  /** The fields of a source; null for one no table can read. */
  fieldsOf: (sheet: number, source: CellWindow) => PivotSourceField[] | null;
  /** Why the last change was refused, if it was. */
  error: string | null;
  onCreate: (definition: PivotTableDefinition) => void;
  onChange: (index: number, definition: PivotTableDefinition) => void;
  onRefresh: (index: number) => void;
  onRefreshAll: () => void;
  onRemove: (index: number) => void;
  onSelect: (row: number, column: number) => void;
  onClose: () => void;
}): ReactElement {
  const [chosen, setChosen] = useState<number | null>(null);
  const table = chosen === null ? undefined : tables[chosen];
  const sourceSheet = table ? (table.sourceSheet ?? sheetIndex) : sheetIndex;
  const source = table ? table.source : selection;
  const fields = fieldsOf(sourceSheet, source);
  const suggested = cellName({ row: selection.top, column: selection.right + 2 });
  const key = `${chosen ?? 'new'}:${source.top}:${source.left}:${source.bottom}:${source.right}:${fields?.length ?? 0}`;
  const [shown, setShown] = useState(key);
  const [draft, setDraft] = useState<PivotDraft>(() => pivotDraft(fields?.length ?? 0, table, suggested));
  // Another table chosen, or a new selection, starts the form again.
  if (key !== shown) {
    setShown(key);
    setDraft(pivotDraft(fields?.length ?? 0, table, suggested));
  }
  const [incomplete, setIncomplete] = useState(false);
  const sourceName = (each: SheetPivotTable) =>
    `${each.sourceSheetName}!${cellName({ row: each.source.top, column: each.source.left })}:${cellName({ row: each.source.bottom, column: each.source.right })}`;
  return (
    <aside className="rely-sheet-panel" aria-label={words.pivotTables}>
      <header>
        <h2>{words.pivotTables}</h2>
        <button type="button" className="rely-tool" aria-label={words.close} title={words.close} onClick={onClose}>
          {'×'}
        </button>
      </header>
      {tables.length === 0 ? <p className="rely-sheet-panel-empty">{words.noPivotTables}</p> : null}
      <ol className="rely-sheet-rules">
        {tables.map((each) => (
          <li key={each.index} className={each.index === chosen ? 'rely-sheet-rule-here' : undefined}>
            <button
              type="button"
              className="rely-sheet-rule-range"
              onClick={() => {
                setChosen(each.index);
                onSelect(each.anchor.row, each.anchor.column);
              }}
            >
              {each.name}
            </button>
            <span className="rely-sheet-rule-text">{sourceName(each)}</span>
            <span className="rely-sheet-panel-actions">
              <button type="button" onClick={() => onRefresh(each.index)}>
                {words.pivotRefresh}
              </button>
              <button
                type="button"
                onClick={() => {
                  setChosen(null);
                  onRemove(each.index);
                }}
              >
                {words.remove}
              </button>
            </span>
          </li>
        ))}
      </ol>
      {tables.length > 0 ? (
        <button type="button" onClick={onRefreshAll}>
          {words.pivotRefreshAll}
        </button>
      ) : null}
      {chosen !== null ? (
        <button type="button" onClick={() => setChosen(null)}>
          {words.pivotNew}
        </button>
      ) : null}
      {fields === null ? (
        <p className="rely-sheet-panel-empty">{words.pivotSelectSource}</p>
      ) : (
        <form
          className="rely-sheet-panel-form"
          onSubmit={(event) => {
            event.preventDefault();
            const definition = pivotDefinition(draft, sourceSheet, source, table);
            if (!definition) {
              setIncomplete(true);
              return;
            }
            setIncomplete(false);
            if (table && chosen !== null) onChange(chosen, definition);
            else onCreate(definition);
          }}
        >
          <p className="rely-sheet-panel-target">
            {words.pivotSource} <strong>{cellName({ row: source.top, column: source.left })}:{cellName({ row: source.bottom, column: source.right })}</strong>
          </p>
          <label>
            <span>{words.pivotAnchor}</span>
            <input value={draft.anchor} onChange={(event) => setDraft({ ...draft, anchor: event.target.value })} />
          </label>
          {fields.map((field, index) => (
            <div className="rely-sheet-pivot-field" key={index}>
              <span>{field.name || `${words.pivotField} ${index + 1}`}</span>
              <select
                aria-label={`${words.pivotRole} ${field.name}`}
                value={draft.roles[index] ?? 'none'}
                onChange={(event) => {
                  const roles = [...draft.roles];
                  roles[index] = event.target.value as PivotRole;
                  setDraft({ ...draft, roles });
                }}
              >
                {ROLES.map(([role, label]) => (
                  <option key={role} value={role}>
                    {words[label] as string}
                  </option>
                ))}
              </select>
              {draft.roles[index] === 'values' ? (
                <select
                  aria-label={`${words.pivotFunction} ${field.name}`}
                  value={draft.functions[index] ?? 'sum'}
                  onChange={(event) => {
                    const functions = [...draft.functions];
                    functions[index] = event.target.value as PivotFunction;
                    setDraft({ ...draft, functions });
                  }}
                >
                  {PIVOT_FUNCTIONS.map(([summary, label]) => (
                    <option key={summary} value={summary}>
                      {words[label] as string}
                    </option>
                  ))}
                </select>
              ) : null}
            </div>
          ))}
          <label className="rely-sheet-panel-check">
            <input
              type="checkbox"
              checked={draft.rowGrandTotals}
              onChange={(event) => setDraft({ ...draft, rowGrandTotals: event.target.checked })}
            />
            <span>{words.pivotRowTotals}</span>
          </label>
          <label className="rely-sheet-panel-check">
            <input
              type="checkbox"
              checked={draft.columnGrandTotals}
              onChange={(event) => setDraft({ ...draft, columnGrandTotals: event.target.checked })}
            />
            <span>{words.pivotColumnTotals}</span>
          </label>
          {incomplete ? (
            <p className="rely-sheet-panel-error" role="alert">
              {words.pivotBadAnchor}
            </p>
          ) : null}
          {error ? (
            <p className="rely-sheet-panel-error" role="alert">
              {error}
            </p>
          ) : null}
          <button type="submit" className="rely-sheet-panel-submit">
            {table ? words.pivotApply : words.pivotCreate}
          </button>
        </form>
      )}
    </aside>
  );
}
