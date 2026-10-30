// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import { describe, expect, it } from 'vitest';
import type { DataValidation } from '../model.js';
import {
  EMPTY_RULE,
  EMPTY_VALIDATION,
  addConditionalFormat,
  conditionOf,
  describeCondition,
  operand,
  rangeName,
  replaceValidation,
  validationOf,
  validationsIn,
} from './sheet-rules.js';

const WINDOW = { top: 1, left: 0, bottom: 9, right: 2 };

describe('operand', () => {
  it('reads numbers, formulas, dates and text', () => {
    expect(operand(' 1,5 ')).toBe('1.5');
    expect(operand('-20')).toBe('-20');
    expect(operand('=A1*2')).toBe('A1*2');
    expect(operand('31/12/2026')).toBe('DATE(2026,12,31)');
    expect(operand('Sì "ok"')).toBe('"Sì ""ok"""');
  });
});

describe('conditional formats', () => {
  it('writes the condition a form describes', () => {
    expect(conditionOf({ ...EMPTY_RULE, kind: 'greater', first: '10' })).toEqual({
      kind: 'cellIs',
      operator: 'greater',
      formulas: ['10'],
    });
    expect(conditionOf({ ...EMPTY_RULE, kind: 'between', first: '1', second: '' })).toBeNull();
    expect(conditionOf({ ...EMPTY_RULE, kind: 'top', first: 'x' })).toBeNull();
    expect(conditionOf({ ...EMPTY_RULE, kind: 'colorScale' })).toEqual({
      kind: 'colorScale',
      points: [
        { value: { kind: 'min' }, colour: 'F8696B' },
        { value: { kind: 'max' }, colour: '63BE7B' },
      ],
    });
    expect(conditionOf({ ...EMPTY_RULE, kind: 'expression', first: '=$A2>0' })).toEqual({
      kind: 'expression',
      formula: '$A2>0',
    });
  });

  it('adds a rule after the others with its look', () => {
    const first = addConditionalFormat([], WINDOW, { ...EMPTY_RULE, kind: 'duplicates', bold: true })!;
    expect(first).toEqual([
      {
        ranges: [{ firstRow: 1, firstColumn: 0, lastRow: 9, lastColumn: 2 }],
        rules: [
          {
            condition: { kind: 'duplicates', unique: false },
            look: { fill: 'FFC9C9', fontColour: '9C0006', bold: true },
            priority: 1,
            stopIfTrue: false,
          },
        ],
      },
    ]);
    const second = addConditionalFormat(first, WINDOW, { ...EMPTY_RULE, kind: 'dataBar' })!;
    expect(second[1].rules[0].priority).toBe(2);
    expect(second[1].rules[0].look).toBeNull();
    expect(addConditionalFormat(first, WINDOW, { ...EMPTY_RULE, kind: 'greater' })).toBeNull();
  });

  it('describes a rule shortly', () => {
    expect(describeCondition({ kind: 'cellIs', operator: 'between', formulas: ['1', '5'] })).toBe('between 1 – 5');
    expect(describeCondition({ kind: 'blanks', blank: true })).toBe('blanks');
    expect(rangeName({ firstRow: 1, firstColumn: 0, lastRow: 9, lastColumn: 2 })).toBe('A2:C10');
    expect(rangeName({ firstRow: 0, firstColumn: 1, lastRow: 0, lastColumn: 1 })).toBe('B1');
  });
});

describe('validations', () => {
  it('writes a list, a comparison and a custom rule', () => {
    expect(validationOf({ ...EMPTY_VALIDATION, first: 'Sì; No ,Forse' }, WINDOW)).toMatchObject({
      kind: 'list',
      dropdown: true,
      formulas: ['"Sì,No,Forse"'],
    });
    expect(
      validationOf({ ...EMPTY_VALIDATION, kind: 'whole', operator: 'between', first: '1', second: '10', error: 'Da 1 a 10' }, WINDOW),
    ).toMatchObject({ kind: 'whole', operator: 'between', formulas: ['1', '10'], error: 'Da 1 a 10', showError: true });
    expect(validationOf({ ...EMPTY_VALIDATION, kind: 'decimal', operator: 'greater', first: '' }, WINDOW)).toBeNull();
    expect(validationOf({ ...EMPTY_VALIDATION, kind: 'custom', first: '=LEN(A2)<5' }, WINDOW)).toMatchObject({
      formulas: ['LEN(A2)<5'],
    });
    expect(validationOf({ ...EMPTY_VALIDATION, kind: 'any' }, WINDOW)).toBeNull();
  });

  it('replaces what lies inside the window and keeps the rest', () => {
    const outside: DataValidation = { kind: 'any', ranges: [{ firstRow: 20, firstColumn: 0, lastRow: 30, lastColumn: 0 }] };
    const within: DataValidation = {
      kind: 'whole',
      ranges: [
        { firstRow: 2, firstColumn: 0, lastRow: 3, lastColumn: 0 },
        { firstRow: 40, firstColumn: 0, lastRow: 40, lastColumn: 0 },
      ],
    };
    const replacement = validationOf({ ...EMPTY_VALIDATION, first: 'a' }, WINDOW);
    const result = replaceValidation([outside, within], WINDOW, replacement);
    expect(result).toHaveLength(3);
    expect(result[1].ranges).toEqual([{ firstRow: 40, firstColumn: 0, lastRow: 40, lastColumn: 0 }]);
    expect(replaceValidation([outside, within], WINDOW, null)).toHaveLength(2);
    expect(validationsIn([outside, within], WINDOW)).toEqual([within]);
  });
});
