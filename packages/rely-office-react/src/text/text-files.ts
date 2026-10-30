// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

/** The encodings a plain text file is read in and written back to. */
export type TextEncodingName = 'utf-8' | 'utf-16le' | 'utf-16be' | 'windows-1252';

/** The line ending a file uses. */
export type LineEnding = '\n' | '\r\n' | '\r';

/** A plain text file as the editor holds it: its text with `\n` line ends, and how to write it back. */
export interface TextFile {
  text: string;
  encoding: TextEncodingName;
  /** Whether the file starts with a byte order mark, which a save keeps. */
  byteOrderMark: boolean;
  lineEnding: LineEnding;
}

const UTF8_BOM = [0xef, 0xbb, 0xbf];

function startsWith(bytes: Uint8Array, prefix: number[]): boolean {
  return prefix.every((byte, index) => bytes[index] === byte);
}

/** The line ending most lines of a text use; `\n` when it has none. */
export function dominantLineEnding(text: string): LineEnding {
  let crlf = 0;
  let lf = 0;
  let cr = 0;
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index);
    if (code === 13) {
      if (text.charCodeAt(index + 1) === 10) {
        crlf += 1;
        index += 1;
      } else {
        cr += 1;
      }
    } else if (code === 10) {
      lf += 1;
    }
  }
  if (crlf > lf && crlf >= cr) return '\r\n';
  if (cr > lf && cr > crlf) return '\r';
  return '\n';
}

/**
 * The bytes of a plain text file as text: a byte order mark decides the encoding, valid UTF-8 is
 * UTF-8, and anything else is read as Windows-1252, which every byte decodes in.
 */
export function readTextFile(bytes: Uint8Array): TextFile {
  let encoding: TextEncodingName;
  let byteOrderMark = true;
  let body: Uint8Array;
  if (startsWith(bytes, UTF8_BOM)) {
    encoding = 'utf-8';
    body = bytes.subarray(3);
  } else if (startsWith(bytes, [0xff, 0xfe])) {
    encoding = 'utf-16le';
    body = bytes.subarray(2);
  } else if (startsWith(bytes, [0xfe, 0xff])) {
    encoding = 'utf-16be';
    body = bytes.subarray(2);
  } else {
    byteOrderMark = false;
    body = bytes;
    try {
      new TextDecoder('utf-8', { fatal: true }).decode(body);
      encoding = 'utf-8';
    } catch {
      encoding = 'windows-1252';
    }
  }
  const raw = new TextDecoder(encoding).decode(body);
  const lineEnding = dominantLineEnding(raw);
  return { text: raw.replace(/\r\n?/g, '\n'), encoding, byteOrderMark, lineEnding };
}

const WINDOWS_1252_HIGH: Record<number, number> = {
  0x20ac: 0x80, 0x201a: 0x82, 0x0192: 0x83, 0x201e: 0x84, 0x2026: 0x85, 0x2020: 0x86, 0x2021: 0x87,
  0x02c6: 0x88, 0x2030: 0x89, 0x0160: 0x8a, 0x2039: 0x8b, 0x0152: 0x8c, 0x017d: 0x8e, 0x2018: 0x91,
  0x2019: 0x92, 0x201c: 0x93, 0x201d: 0x94, 0x2022: 0x95, 0x2013: 0x96, 0x2014: 0x97, 0x02dc: 0x98,
  0x2122: 0x99, 0x0161: 0x9a, 0x203a: 0x9b, 0x0153: 0x9c, 0x017e: 0x9e, 0x0178: 0x9f,
};

function encodeWindows1252(text: string): { bytes: Uint8Array; lossy: boolean } {
  const bytes = new Uint8Array(text.length);
  let lossy = false;
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index);
    if (code < 0x80 || (code >= 0xa0 && code <= 0xff)) {
      bytes[index] = code;
    } else {
      const mapped = WINDOWS_1252_HIGH[code];
      if (mapped === undefined) lossy = true;
      bytes[index] = mapped ?? 0x3f;
    }
  }
  return { bytes, lossy };
}

function encodeUtf16(text: string, littleEndian: boolean): Uint8Array {
  const bytes = new Uint8Array(text.length * 2);
  const view = new DataView(bytes.buffer);
  for (let index = 0; index < text.length; index += 1) {
    view.setUint16(index * 2, text.charCodeAt(index), littleEndian);
  }
  return bytes;
}

/** Whether a text holds characters Windows-1252 cannot write, so a save would lose them. */
export function textNeedsUnicode(text: string): boolean {
  return encodeWindows1252(text).lossy;
}

/**
 * The text written back the way its file was: same encoding, byte order mark and line ending.
 * A Windows-1252 file holding characters that encoding cannot write is saved as UTF-8 instead
 * of losing them; `encoding` of the result says which one was used.
 */
export function writeTextFile(file: TextFile): { bytes: Uint8Array; encoding: TextEncodingName } {
  const text = file.lineEnding === '\n' ? file.text : file.text.replace(/\n/g, file.lineEnding);
  let encoding = file.encoding;
  let body: Uint8Array;
  if (encoding === 'windows-1252') {
    const encoded = encodeWindows1252(text);
    if (encoded.lossy) {
      encoding = 'utf-8';
      body = new TextEncoder().encode(text);
    } else {
      body = encoded.bytes;
    }
  } else if (encoding === 'utf-16le' || encoding === 'utf-16be') {
    body = encodeUtf16(text, encoding === 'utf-16le');
  } else {
    body = new TextEncoder().encode(text);
  }
  if (!file.byteOrderMark || encoding !== file.encoding) return { bytes: body, encoding };
  const mark =
    encoding === 'utf-8' ? UTF8_BOM : encoding === 'utf-16le' ? [0xff, 0xfe] : [0xfe, 0xff];
  const bytes = new Uint8Array(mark.length + body.length);
  bytes.set(mark, 0);
  bytes.set(body, mark.length);
  return { bytes, encoding };
}

/** The line and column, both from 1, of an offset in a text. */
export function lineAndColumn(text: string, offset: number): { line: number; column: number } {
  let line = 1;
  let lineStart = 0;
  const end = Math.min(offset, text.length);
  for (let index = 0; index < end; index += 1) {
    if (text.charCodeAt(index) === 10) {
      line += 1;
      lineStart = index + 1;
    }
  }
  return { line, column: end - lineStart + 1 };
}
