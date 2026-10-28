// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// The rules the side panels of the grid write: what a person picks in a form, as the
// conditional formats and the validations of the workbook model.

import type {
  CellRange,
  CellWindow,
  ComparisonOperator,
  Condition,
  ConditionalFormat,
  DataValidation,
  DifferentialLook,
} from '../model.js';
import { cellName } from './geometry.js';

/** The rules the conditional format form offers. */
export type RuleKind =
  | 'greater'
  | 'less'
  | 'between'
  | 'equal'
  | 'textContains'
  | 'duplicates'
  | 'top'
  | 'blanks'
  | 'colorScale'
  | 'dataBar'
  | 'expression';

export interface RuleDraft {
  kind: RuleKind;
  first: string;
  second: string;
  fill: string;
  fontColour: string;
  bold: boolean;
  /** The colours of a scale or a bar, `#rrggbb`. */
  low: string;
  high: string;
}

export const EMPTY_RULE: RuleDraft = {
  kind: 'greater',
  first: '',
  second: '',
  fill: '#ffc9c9',
  fontColour: '#9c0006',
  bold: false,
  low: '#f8696b',
  high: '#63be7b',
};

/** The kinds of validation the form offers; `any` takes the validation away. */
export type ValidationKind = DataValidation['kind'];

export interface ValidationDraft {
  kind: ValidationKind;
  operator: ComparisonOperator;
  first: string;
  second: string;
  allowBlank: boolean;
  errorStyle: 'stop' | 'warning' | 'information';
  error: string;
  prompt: string;
}

export const EMPTY_VALIDATION: ValidationDraft = {
  kind: 'list',
  operator: 'between',
  first: '',
  second: '',
  allowBlank: true,
  errorStyle: 'stop',
  error: '',
  prompt: '',
};

export function windowRange(window: CellWindow): CellRange {
  return { firstRow: window.top, firstColumn: window.left, lastRow: window.bottom, lastColumn: window.right };
}

/** A range as a formula writes it: `B2`, or `A1:C3`. */
export function rangeName(range: CellRange): string {
  const first = cellName({ row: range.firstRow, column: range.firstColumn });
  if (range.firstRow === range.lastRow && range.firstColumn === range.lastColumn) return first;
  return `${first}:${cellName({ row: range.lastRow, column: range.lastColumn })}`;
}

/**
 * A value typed into a rule form as a formula operand: a number with either decimal mark, a
 * formula after `=`, a date as `dd/mm/yyyy`, or else text in quotes.
 */
export function operand(typed: string): string {
  const text = typed.trim();
  if (/^-?\d+([.,]\d+)?$/.test(text)) return text.replace(',', '.');
  if (text.startsWith('=')) return text.slice(1);
  const date = /^(\d{1,2})\/(\d{1,2})\/(\d{4})$/.exec(text);
  if (date) return `DATE(${Number(date[3])},${Number(date[2])},${Number(date[1])})`;
  return `"${text.replace(/"/g, '""')}"`;
}

function hex(colour: string): string {
  return colour.replace(/^#/, '').toUpperCase();
}

/** The condition a draft describes, or null when a value it needs is missing. */
export function conditionOf(draft: RuleDraft): Condition | null {
  const needsFirst = ['greater', 'less', 'between', 'equal', 'textContains', 'top', 'expression'];
  if (needsFirst.includes(draft.kind) && draft.first.trim() === '') return null;
  switch (draft.kind) {
    case 'greater':
    case 'less':
    case 'equal':
      return { kind: 'cellIs', operator: draft.kind, formulas: [operand(draft.first)] };
    case 'between':
      if (draft.second.trim() === '') return null;
      return { kind: 'cellIs', operator: 'between', formulas: [operand(draft.first), operand(draft.second)] };
    case 'textContains':
      return { kind: 'text', operator: 'contains', text: draft.first.trim() };
    case 'duplicates':
      return { kind: 'duplicates', unique: false };
    case 'top': {
      const rank = Number.parseInt(draft.first, 10);
      return Number.isFinite(rank) && rank > 0 ? { kind: 'top', rank, percent: false, bottom: false } : null;
    }
    case 'blanks':
      return { kind: 'blanks', blank: true };
    case 'colorScale':
      return {
        kind: 'colorScale',
        points: [
          { value: { kind: 'min' }, colour: hex(draft.low) },
          { value: { kind: 'max' }, colour: hex(draft.high) },
        ],
      };
    case 'dataBar':
      return { kind: 'dataBar', min: { kind: 'min' }, max: { kind: 'max' }, colour: hex(draft.high), showValue: true };
    case 'expression':
      return { kind: 'expression', formula: draft.first.trim().replace(/^=/, '') };
  }
}

/** The formats with a new rule for a window, after every rule already there. */
export function addConditionalFormat(
  formats: ConditionalFormat[],
  window: CellWindow,
  draft: RuleDraft,
): ConditionalFormat[] | null {
  const condition = conditionOf(draft);
  if (!condition) return null;
  const priorities = formats.flatMap((format) => format.rules.map((rule) => rule.priority ?? 0));
  const priority = Math.max(0, ...priorities) + 1;
  const look: DifferentialLook | null =
    condition.kind === 'colorScale' || condition.kind === 'dataBar'
      ? null
      : { fill: hex(draft.fill), fontColour: hex(draft.fontColour), bold: draft.bold ? true : null };
  return [...formats, { ranges: [windowRange(window)], rules: [{ condition, look, priority, stopIfTrue: false }] }];
}

/** A short description of a condition, for the list of rules. */
export function describeCondition(condition: Condition): string {
  switch (condition.kind) {
    case 'cellIs':
      return `${condition.operator} ${condition.formulas.join(' – ')}`;
    case 'expression':
      return `=${condition.formula}`;
    case 'text':
      return `${condition.operator} "${condition.text}"`;
    case 'top':
      return `${condition.bottom ? 'bottom' : 'top'} ${condition.rank}${condition.percent ? '%' : ''}`;
    case 'iconSet':
      return condition.icons;
    case 'timePeriod':
      return condition.period;
    default:
      return condition.kind;
  }
}

function overlaps(range: CellRange, window: CellWindow): boolean {
  return range.firstRow <= window.bottom && range.lastRow >= window.top && range.firstColumn <= window.right && range.lastColumn >= window.left;
}

function inside(range: CellRange, window: CellWindow): boolean {
  return range.firstRow >= window.top && range.lastRow <= window.bottom && range.firstColumn >= window.left && range.lastColumn <= window.right;
}

/** The validations that apply to some cell of a window. */
export function validationsIn(validations: DataValidation[], window: CellWindow): DataValidation[] {
  return validations.filter((validation) => validation.ranges.some((range) => overlaps(range, window)));
}

/** The validation a draft describes, or null when a value it needs is missing. */
export function validationOf(draft: ValidationDraft, window: CellWindow): DataValidation | null {
  if (draft.kind === 'any') return null;
  const base = {
    ranges: [windowRange(window)],
    allowBlank: draft.allowBlank,
    errorStyle: draft.errorStyle,
    showError: true,
    error: draft.error.trim() === '' ? null : draft.error.trim(),
    showPrompt: draft.prompt.trim() !== '',
    prompt: draft.prompt.trim() === '' ? null : draft.prompt.trim(),
  };
  if (draft.kind === 'list') {
    const items = draft.first
      .split(/[;,\n]/)
      .map((item) => item.trim())
      .filter(Boolean);
    if (items.length === 0) return null;
    return { ...base, kind: 'list', dropdown: true, formulas: [`"${items.join(',').replace(/"/g, '""')}"`] };
  }
  if (draft.kind === 'custom') {
    if (draft.first.trim() === '') return null;
    return { ...base, kind: 'custom', formulas: [draft.first.trim().replace(/^=/, '')] };
  }
  const two = draft.operator === 'between' || draft.operator === 'notBetween';
  if (draft.first.trim() === '' || (two && draft.second.trim() === '')) return null;
  const formulas = two ? [operand(draft.first), operand(draft.second)] : [operand(draft.first)];
  return { ...base, kind: draft.kind, operator: draft.operator, formulas };
}

/** The validations with the ones wholly inside a window taken away and the new one added. */
export function replaceValidation(
  validations: DataValidation[],
  window: CellWindow,
  validation: DataValidation | null,
): DataValidation[] {
  const kept = validations
    .map((existing) => ({ ...existing, ranges: existing.ranges.filter((range) => !inside(range, window)) }))
    .filter((existing) => existing.ranges.length > 0);
  return validation ? [...kept, validation] : kept;
}
