// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';
import { acceptFunction, functionTokenAt, suggestFunctions } from './formula-hints.js';

const NAMES = ['SOMMA', 'SOMMA.SE', 'SOMMA.PIÙ.SE', 'SE', 'SENO', 'CERCA.VERT', 'MEDIA'];

describe('functionTokenAt', () => {
  it('finds the name being typed after = or a separator', () => {
    expect(functionTokenAt('=SO', 3)).toEqual({ start: 1, end: 3, prefix: 'SO' });
    expect(functionTokenAt('=A1+som', 7)).toEqual({ start: 4, end: 7, prefix: 'som' });
    expect(functionTokenAt('=SE(A1>0;me', 11)).toEqual({ start: 9, end: 11, prefix: 'me' });
    expect(functionTokenAt('=SOMMA.S', 8)?.prefix).toBe('SOMMA.S');
  });

  it('is silent outside a function name', () => {
    expect(functionTokenAt('SOMMA', 5)).toBeNull();
    expect(functionTokenAt('="SO', 4)).toBeNull();
    expect(functionTokenAt('=A1', 3)).toBeNull();
    expect(functionTokenAt('=A1:B', 5)).toBeNull();
    expect(functionTokenAt('=1', 2)).toBeNull();
    expect(functionTokenAt('=SOM', 2)).toBeNull();
    expect(functionTokenAt('=Foglio1!A', 10)).toBeNull();
  });
});

describe('suggestFunctions', () => {
  it('offers the names the prefix starts, the shortest first', () => {
    expect(suggestFunctions(NAMES, 'so')).toEqual(['SOMMA', 'SOMMA.SE', 'SOMMA.PIÙ.SE']);
    expect(suggestFunctions(NAMES, 'se')).toEqual(['SENO']);
    expect(suggestFunctions(NAMES, 'x')).toEqual([]);
    expect(suggestFunctions(NAMES, 's', 2)).toEqual(['SE', 'SENO']);
  });
});

describe('acceptFunction', () => {
  it('puts the name and a bracket in place of the token', () => {
    const token = functionTokenAt('=A1+som', 7)!;
    expect(acceptFunction('=A1+som', token, 'SOMMA')).toEqual({ text: '=A1+SOMMA(', caret: 10 });
    const inside = functionTokenAt('=som(A1)', 4)!;
    expect(acceptFunction('=som(A1)', inside, 'SOMMA')).toEqual({ text: '=SOMMA(A1)', caret: 7 });
  });
});
