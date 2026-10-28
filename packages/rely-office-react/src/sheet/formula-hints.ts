// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

/** The most function names offered at once while a formula is typed. */
export const MAX_FUNCTION_HINTS = 8;

/** The part of a formula a function name is being typed into, and what is typed so far. */
export interface FunctionToken {
  start: number;
  end: number;
  prefix: string;
}

/**
 * The function name the caret is at the end of, in a formula: letters, digits and dots after
 * `=`, an operator, a separator or a bracket; null outside a formula, inside a string, after a
 * sheet name or a cell reference such as `A1`.
 */
export function functionTokenAt(text: string, caret: number): FunctionToken | null {
  if (!text.startsWith('=') || caret < 2 || caret > text.length) return null;
  let quotes = 0;
  for (let index = 0; index < caret; index += 1) if (text[index] === '"') quotes += 1;
  if (quotes % 2 === 1) return null;
  let start = caret;
  while (start > 1 && /[A-Za-z0-9._À-ɏ]/.test(text[start - 1])) start -= 1;
  const prefix = text.slice(start, caret);
  if (prefix.length === 0 || !/^[A-Za-zÀ-ɏ]/.test(prefix)) return null;
  if (/^[A-Za-z]{1,3}\d+$/.test(prefix)) return null;
  const before = text[start - 1];
  if (!/[=(;,+\-*/^&<>:\s]/.test(before) || before === ':') return null;
  if (text[caret] !== undefined && /[A-Za-z0-9._]/.test(text[caret])) return null;
  return { start, end: caret, prefix };
}

/** The names that start with the typed prefix, case aside, the shortest first. */
export function suggestFunctions(names: readonly string[], prefix: string, limit = MAX_FUNCTION_HINTS): string[] {
  const wanted = prefix.toUpperCase();
  return names
    .filter((name) => name.toUpperCase().startsWith(wanted) && name.toUpperCase() !== wanted)
    .sort((first, second) => first.length - second.length || first.localeCompare(second))
    .slice(0, limit);
}

/** The formula with a chosen name in place of the token and an opening bracket after it. */
export function acceptFunction(text: string, token: FunctionToken, name: string): { text: string; caret: number } {
  const bracket = text[token.end] === '(' ? '' : '(';
  const next = `${text.slice(0, token.start)}${name}${bracket}${text.slice(token.end)}`;
  return { text: next, caret: token.start + name.length + 1 };
}
