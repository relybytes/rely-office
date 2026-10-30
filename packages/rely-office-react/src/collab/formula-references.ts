// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

// The references of a formula moved the way the workbook moves them: past inserted and deleted
// rows and columns, a renamed sheet and a removed one. An operation made before another carries
// its formulas as text, and they must point where the workbook would have moved them had the
// formula been there first. Read like the engine reads a formula, in either locale: texts and
// function names are left alone, and only a reference that changes is written again.

/** The last row and column a reference can name, as Excel and the engine count them. */
const MAX_ROWS = 1_048_576;
const MAX_COLUMNS = 16_384;

/** Rows or columns inserted or deleted, as the references of a formula see them. */
export interface ReferenceShift {
  axis: 'row' | 'column';
  at: number;
  count: number;
  inserted: boolean;
}

interface Coordinate {
  index: number;
  absolute: boolean;
}

/** One end of a reference: a cell, or a whole column or row when the other is missing. */
interface Endpoint {
  column: Coordinate | null;
  row: Coordinate | null;
}

/** A reference as written: its sheet, if named, one end or two. */
export interface FormulaReference {
  /** The sheet the reference names, unquoted; null when it names none and so means the formula's own. */
  sheet: string | null;
  /** The sheet the second end names, when it repeats one. */
  endSheet: string | null;
  start: Endpoint;
  end: Endpoint | null;
}

/** What to write in place of a reference: a new one, `#REF!`, or null to keep it as written. */
type Rewrite = (reference: FormulaReference) => FormulaReference | '#REF!' | null;

const isWordStart = (character: string): boolean => /[\p{L}_$]/u.test(character);
const isWordPart = (character: string): boolean => /[\p{L}\p{N}._$]/u.test(character);

function columnIndex(letters: string): number {
  let index = 0;
  for (const letter of letters.toUpperCase()) index = index * 26 + (letter.charCodeAt(0) - 64);
  return index - 1;
}

function columnLetters(index: number): string {
  let letters = '';
  for (let rest = index + 1; rest > 0; rest = Math.floor((rest - 1) / 26)) {
    letters = String.fromCharCode(65 + ((rest - 1) % 26)) + letters;
  }
  return letters;
}

/** One end of a reference read from a word, or null when the word is not one. */
function parseEndpoint(word: string): Endpoint | null {
  const cell = /^(\$?)([A-Za-z]{1,3})(\$?)(\d+)$/.exec(word);
  if (cell) {
    const column = columnIndex(cell[2]!);
    const row = Number(cell[4]) - 1;
    if (column >= MAX_COLUMNS || row < 0 || row >= MAX_ROWS) return null;
    return { column: { index: column, absolute: cell[1] === '$' }, row: { index: row, absolute: cell[3] === '$' } };
  }
  const column = /^(\$?)([A-Za-z]{1,3})$/.exec(word);
  if (column) {
    const index = columnIndex(column[2]!);
    return index < MAX_COLUMNS ? { column: { index, absolute: column[1] === '$' }, row: null } : null;
  }
  const row = /^(\$?)(\d+)$/.exec(word);
  if (row) {
    const index = Number(row[2]) - 1;
    return index >= 0 && index < MAX_ROWS ? { column: null, row: { index, absolute: row[1] === '$' } } : null;
  }
  return null;
}

const kind = (endpoint: Endpoint): 'cell' | 'column' | 'row' =>
  endpoint.column && endpoint.row ? 'cell' : endpoint.column ? 'column' : 'row';

function printEndpoint(endpoint: Endpoint): string {
  let text = '';
  if (endpoint.column) text += `${endpoint.column.absolute ? '$' : ''}${columnLetters(endpoint.column.index)}`;
  if (endpoint.row) text += `${endpoint.row.absolute ? '$' : ''}${endpoint.row.index + 1}`;
  return text;
}

/** A sheet name as the engine writes it in a formula: bare when it can be, quoted otherwise. */
export function printSheetName(name: string): string {
  const endpoint = parseEndpoint(name);
  const plain = /^\p{L}[\p{L}\p{N}_]*$/u.test(name) && !(endpoint && kind(endpoint) === 'cell');
  return plain ? name : `'${name.replace(/'/g, "''")}'`;
}

function printReference(reference: FormulaReference): string {
  const prefix = (sheet: string | null) => (sheet === null ? '' : `${printSheetName(sheet)}!`);
  const start = prefix(reference.sheet) + printEndpoint(reference.start);
  return reference.end ? `${start}:${prefix(reference.endSheet)}${printEndpoint(reference.end)}` : start;
}

/** A sheet name, quoted or bare, with its `!`, at `index`; null when there is none there. */
function readSheetPrefix(formula: string, index: number): { name: string; next: number } | null {
  if (formula[index] === "'") {
    let name = '';
    let at = index + 1;
    while (at < formula.length) {
      if (formula[at] === "'") {
        if (formula[at + 1] === "'") {
          name += "'";
          at += 2;
          continue;
        }
        break;
      }
      name += formula[at];
      at += 1;
    }
    if (at >= formula.length || formula[at + 1] !== '!') return null;
    return { name, next: at + 2 };
  }
  if (!isWordStart(formula[index] ?? '')) return null;
  let at = index;
  while (at < formula.length && isWordPart(formula[at]!)) at += 1;
  if (formula[at] !== '!') return null;
  return { name: formula.slice(index, at), next: at + 1 };
}

function readWord(formula: string, index: number): { word: string; next: number } {
  let at = index;
  while (at < formula.length && isWordPart(formula[at]!)) at += 1;
  return { word: formula.slice(index, at), next: at };
}

function skipSpaces(formula: string, index: number): number {
  let at = index;
  while (at < formula.length && /\s/.test(formula[at]!)) at += 1;
  return at;
}

/**
 * The reference that starts at `index`, after an optional sheet, with where it ends; null when
 * what is there is not a reference (a name, a function, a number).
 */
function readReference(formula: string, index: number): { reference: FormulaReference; next: number } | null {
  const prefix = readSheetPrefix(formula, index);
  const startAt = prefix ? prefix.next : index;
  if (!/[A-Za-z$0-9]/.test(formula[startAt] ?? '')) return null;
  const first = readWord(formula, startAt);
  const start = parseEndpoint(first.word);
  if (!start) return null;
  // A cell glued to `(` is a function such as LOG10, one glued to `[` a table.
  const after = skipSpaces(formula, first.next);
  if (formula[after] === '(' || formula[first.next] === '[') return null;
  const reference: FormulaReference = { sheet: prefix?.name ?? null, endSheet: null, start, end: null };
  if (formula[after] === ':') {
    let endAt = skipSpaces(formula, after + 1);
    const endPrefix = readSheetPrefix(formula, endAt);
    if (endPrefix) endAt = endPrefix.next;
    const second = readWord(formula, endAt);
    const end = parseEndpoint(second.word);
    const afterEnd = skipSpaces(formula, second.next);
    if (end && kind(end) === kind(start) && formula[afterEnd] !== '(' && formula[second.next] !== '[') {
      return { reference: { ...reference, endSheet: endPrefix?.name ?? null, end }, next: second.next };
    }
  }
  // A whole column or row is only one written as a range.
  if (kind(start) !== 'cell') return null;
  return { reference, next: first.next };
}

/** The length of a number written at `index`, its decimal point or comma and exponent included. */
function numberLength(formula: string, index: number): number {
  let at = index;
  while (at < formula.length) {
    const character = formula[at]!;
    const digitAfter = /\d/.test(formula[at + 1] ?? '');
    if (/\d/.test(character)) at += 1;
    else if ((character === '.' || character === ',') && digitAfter) at += 1;
    else if (/[eE]/.test(character) && /[\d+-]/.test(formula[at + 1] ?? '')) at += 2;
    else break;
  }
  return at - index;
}

/** The end of a `[...]` group at `index`, nested groups included. */
function bracketEnd(formula: string, index: number): number {
  let depth = 0;
  for (let at = index; at < formula.length; at += 1) {
    if (formula[at] === '[') depth += 1;
    else if (formula[at] === ']') {
      depth -= 1;
      if (depth === 0) return at + 1;
    }
  }
  return formula.length;
}

/** The end of a quoted text at `index`, a doubled quote standing for one. */
function quotedEnd(formula: string, index: number, quote: string): number {
  let at = index + 1;
  while (at < formula.length) {
    if (formula[at] === quote) {
      if (formula[at + 1] === quote) at += 2;
      else return at + 1;
    } else at += 1;
  }
  return formula.length;
}

/** The formula with every reference `rewrite` changes written again, the rest as it was. */
export function rewriteReferences(formula: string, rewrite: Rewrite): string {
  let output = '';
  let index = 0;
  let changed = false;
  while (index < formula.length) {
    const character = formula[index]!;
    if (character === '"') {
      const end = quotedEnd(formula, index, '"');
      output += formula.slice(index, end);
      index = end;
      continue;
    }
    if (character === '#') {
      // An error value; one such as #REF! may be glued to what was a reference, kept as it is.
      let end = index + 1;
      while (end < formula.length && /[A-Za-z0-9_/]/.test(formula[end]!)) end += 1;
      if (formula[end] === '!' || formula[end] === '?') end += 1;
      if (formula[end - 1] === '!') end = readWord(formula, end).next;
      output += formula.slice(index, end);
      index = end;
      continue;
    }
    if (character === '[') {
      // An external workbook or a table: what follows it is not this workbook's.
      let end = bracketEnd(formula, index);
      const prefix = readSheetPrefix(formula, end);
      if (prefix) end = readWord(formula, prefix.next).next;
      output += formula.slice(index, end);
      index = end;
      continue;
    }
    if (character === "'" || isWordStart(character) || /\d/.test(character)) {
      const read = readReference(formula, index);
      if (read) {
        const replaced = rewrite(read.reference);
        if (replaced === null) output += formula.slice(index, read.next);
        else {
          output += replaced === '#REF!' ? '#REF!' : printReference(replaced);
          changed = true;
        }
        index = read.next;
        continue;
      }
      if (character === "'") {
        const end = quotedEnd(formula, index, "'");
        output += formula.slice(index, end);
        index = end;
        continue;
      }
      if (/\d/.test(character)) {
        const length = numberLength(formula, index);
        output += formula.slice(index, index + length);
        index += length;
        continue;
      }
      const prefix = readSheetPrefix(formula, index);
      const end = prefix ? readWord(formula, prefix.next).next : readWord(formula, index).next;
      output += formula.slice(index, end);
      index = end;
      continue;
    }
    output += character;
    index += 1;
  }
  return changed ? output : formula;
}

const sameSheet = (a: string, b: string): boolean => a.toLowerCase() === b.toLowerCase();

/** Which references a change applies to: those of a sheet by name, and the unqualified ones when that sheet is the formula's own. */
export interface ReferenceTarget {
  /** The name of the sheet the change was made on; unknown names only the unqualified references. */
  sheetName?: string;
  /** Whether the formula belongs to that sheet, so that its unqualified references are on it. */
  ownSheet: boolean;
}

function targets(reference: FormulaReference, target: ReferenceTarget): boolean {
  if (reference.sheet === null) return target.ownSheet;
  return target.sheetName !== undefined && sameSheet(reference.sheet, target.sheetName);
}

function moveCoordinate(coordinate: Coordinate, shift: ReferenceShift): Coordinate | null {
  if (shift.inserted) {
    const index = coordinate.index >= shift.at ? coordinate.index + shift.count : coordinate.index;
    const limit = shift.axis === 'row' ? MAX_ROWS : MAX_COLUMNS;
    return index < limit ? { ...coordinate, index } : null;
  }
  if (coordinate.index < shift.at) return coordinate;
  if (coordinate.index < shift.at + shift.count) return null;
  return { ...coordinate, index: coordinate.index - shift.count };
}

/** A formula's references moved past rows or columns inserted or deleted on a sheet. */
export function shiftFormula(formula: string, shift: ReferenceShift, target: ReferenceTarget): string {
  const axis = shift.axis;
  const coordinate = (endpoint: Endpoint) => endpoint[axis];
  const placed = (endpoint: Endpoint, moved: Coordinate): Endpoint =>
    axis === 'row' ? { ...endpoint, row: moved } : { ...endpoint, column: moved };
  return rewriteReferences(formula, (reference) => {
    if (!targets(reference, target)) return null;
    const first = coordinate(reference.start);
    if (!first) return null;
    if (!reference.end) {
      const moved = moveCoordinate(first, shift);
      if (!moved) return '#REF!';
      return moved.index === first.index ? null : { ...reference, start: placed(reference.start, moved) };
    }
    const second = coordinate(reference.end)!;
    const [low, high] = first.index <= second.index ? [first, second] : [second, first];
    let newLow: Coordinate | null;
    let newHigh: Coordinate | null;
    if (shift.inserted) {
      newLow = moveCoordinate(low, shift);
      newHigh = moveCoordinate(high, shift);
      if (!newLow || !newHigh) return '#REF!';
    } else {
      const last = shift.at + shift.count - 1;
      if (low.index >= shift.at && high.index <= last) return '#REF!';
      newLow = { ...low, index: low.index < shift.at ? low.index : low.index > last ? low.index - shift.count : shift.at };
      newHigh = { ...high, index: high.index < shift.at ? high.index : high.index > last ? high.index - shift.count : shift.at - 1 };
    }
    if (newLow.index === low.index && newHigh.index === high.index) return null;
    const lowIsStart = low === first;
    return {
      ...reference,
      start: placed(reference.start, lowIsStart ? newLow : newHigh),
      end: placed(reference.end, lowIsStart ? newHigh : newLow),
    };
  });
}

/** A formula whose references to a sheet name it by its new name. */
export function renameFormulaSheet(formula: string, from: string, to: string): string {
  return rewriteReferences(formula, (reference) => {
    const start = reference.sheet !== null && sameSheet(reference.sheet, from);
    const end = reference.endSheet !== null && sameSheet(reference.endSheet, from);
    if (!start && !end) return null;
    return { ...reference, sheet: start ? to : reference.sheet, endSheet: end ? to : reference.endSheet };
  });
}

/** A formula whose references to a removed sheet are `#REF!`. */
export function removeFormulaSheet(formula: string, name: string): string {
  return rewriteReferences(formula, (reference) =>
    reference.sheet !== null && sameSheet(reference.sheet, name) ? '#REF!' : null,
  );
}
