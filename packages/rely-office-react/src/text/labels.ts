// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import type { LineEnding, TextEncodingName } from './text-files.js';

/** The words of the text editor; a host passes its own language. */
export interface TextEditorLabels {
  toolbar: string;
  text: string;
  wrapLines: string;
  position: (line: number, column: number) => string;
  encoding: (encoding: TextEncodingName) => string;
  lineEnding: (lineEnding: LineEnding) => string;
  /** Shown when the text holds characters its Windows-1252 file cannot keep. */
  savedAsUnicode: string;
}

const ENCODING_NAMES: Record<TextEncodingName, string> = {
  'utf-8': 'UTF-8',
  'utf-16le': 'UTF-16 LE',
  'utf-16be': 'UTF-16 BE',
  'windows-1252': 'Windows-1252',
};

const LINE_ENDING_NAMES: Record<LineEnding, string> = { '\n': 'LF', '\r\n': 'CRLF', '\r': 'CR' };

export const ENGLISH_TEXT_EDITOR_LABELS: TextEditorLabels = {
  toolbar: 'Text tools',
  text: 'Text',
  wrapLines: 'Wrap lines',
  position: (line, column) => `Line ${line}, column ${column}`,
  encoding: (encoding) => ENCODING_NAMES[encoding],
  lineEnding: (lineEnding) => LINE_ENDING_NAMES[lineEnding],
  savedAsUnicode: 'The text holds characters Windows-1252 cannot write: it will be saved as UTF-8.',
};

export const ITALIAN_TEXT_EDITOR_LABELS: TextEditorLabels = {
  toolbar: 'Strumenti del testo',
  text: 'Testo',
  wrapLines: 'A capo automatico',
  position: (line, column) => `Riga ${line}, colonna ${column}`,
  encoding: (encoding) => ENCODING_NAMES[encoding],
  lineEnding: (lineEnding) => LINE_ENDING_NAMES[lineEnding],
  savedAsUnicode: 'Il testo contiene caratteri che Windows-1252 non può scrivere: verrà salvato in UTF-8.',
};
