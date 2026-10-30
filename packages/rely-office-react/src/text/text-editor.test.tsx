// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import { act, fireEvent, render, screen } from '@testing-library/react';
import { createRef } from 'react';
import * as Y from 'yjs';
import { ITALIAN_TEXT_EDITOR_LABELS } from './labels.js';
import { TextEditor, shiftOffset, textChange, type TextEditorHandle } from './text-editor.js';
import {
  dominantLineEnding,
  lineAndColumn,
  readTextFile,
  textNeedsUnicode,
  writeTextFile,
  type TextFile,
} from './text-files.js';

const bytes = (...values: number[]) => new Uint8Array(values);

describe('text files', () => {
  it('reads UTF-8 with and without a byte order mark', () => {
    const plain = readTextFile(new TextEncoder().encode('caffè\n'));
    expect(plain).toEqual({ text: 'caffè\n', encoding: 'utf-8', byteOrderMark: false, lineEnding: '\n' });
    const marked = readTextFile(bytes(0xef, 0xbb, 0xbf, 0x61));
    expect(marked).toMatchObject({ text: 'a', encoding: 'utf-8', byteOrderMark: true });
  });

  it('reads UTF-16 in both byte orders', () => {
    expect(readTextFile(bytes(0xff, 0xfe, 0x61, 0x00, 0xe8, 0x00))).toMatchObject({ text: 'aè', encoding: 'utf-16le' });
    expect(readTextFile(bytes(0xfe, 0xff, 0x00, 0x61))).toMatchObject({ text: 'a', encoding: 'utf-16be' });
  });

  it('reads what is not UTF-8 as Windows-1252', () => {
    const file = readTextFile(bytes(0x63, 0x61, 0x66, 0x66, 0xe8, 0x80));
    expect(file).toMatchObject({ text: 'caffè€', encoding: 'windows-1252', byteOrderMark: false });
  });

  it('keeps the line ending of the file and edits with line feeds', () => {
    const file = readTextFile(new TextEncoder().encode('a\r\nb\r\nc\n'));
    expect(file.text).toBe('a\nb\nc\n');
    expect(file.lineEnding).toBe('\r\n');
    expect(new TextDecoder().decode(writeTextFile(file).bytes)).toBe('a\r\nb\r\nc\r\n');
    expect(dominantLineEnding('a\rb\rc')).toBe('\r');
    expect(dominantLineEnding('no lines')).toBe('\n');
  });

  it('writes a file back in its encoding with its byte order mark', () => {
    const utf16: TextFile = { text: 'aè', encoding: 'utf-16le', byteOrderMark: true, lineEnding: '\n' };
    expect([...writeTextFile(utf16).bytes]).toEqual([0xff, 0xfe, 0x61, 0x00, 0xe8, 0x00]);
    const big: TextFile = { text: 'a', encoding: 'utf-16be', byteOrderMark: false, lineEnding: '\n' };
    expect([...writeTextFile(big).bytes]).toEqual([0x00, 0x61]);
    const marked: TextFile = { text: 'a', encoding: 'utf-8', byteOrderMark: true, lineEnding: '\n' };
    expect([...writeTextFile(marked).bytes]).toEqual([0xef, 0xbb, 0xbf, 0x61]);
    const latin: TextFile = { text: 'è€', encoding: 'windows-1252', byteOrderMark: false, lineEnding: '\n' };
    expect(writeTextFile(latin)).toEqual({ bytes: bytes(0xe8, 0x80), encoding: 'windows-1252' });
  });

  it('saves a Windows-1252 file as UTF-8 rather than lose a character', () => {
    const file: TextFile = { text: 'π', encoding: 'windows-1252', byteOrderMark: false, lineEnding: '\n' };
    expect(textNeedsUnicode(file.text)).toBe(true);
    expect(textNeedsUnicode('è€')).toBe(false);
    const written = writeTextFile(file);
    expect(written.encoding).toBe('utf-8');
    expect(new TextDecoder().decode(written.bytes)).toBe('π');
  });

  it('gives the line and column of an offset', () => {
    expect(lineAndColumn('ab\ncd', 0)).toEqual({ line: 1, column: 1 });
    expect(lineAndColumn('ab\ncd', 4)).toEqual({ line: 2, column: 2 });
    expect(lineAndColumn('ab', 99)).toEqual({ line: 1, column: 3 });
  });
});

describe('text changes', () => {
  it('finds the replaced part between two texts', () => {
    expect(textChange('hello world', 'hello brave world')).toEqual({ at: 6, removed: 0, inserted: 'brave ' });
    expect(textChange('abcdef', 'abef')).toEqual({ at: 2, removed: 2, inserted: '' });
    expect(textChange('aaa', 'aaaa')).toEqual({ at: 3, removed: 0, inserted: 'a' });
    expect(textChange('same', 'same')).toEqual({ at: 4, removed: 0, inserted: '' });
  });

  it('moves a caret through a remote change', () => {
    expect(shiftOffset(5, [{ insert: 'xx' }])).toBe(7);
    expect(shiftOffset(5, [{ retain: 6 }, { insert: 'xx' }])).toBe(5);
    expect(shiftOffset(5, [{ retain: 5 }, { insert: 'xx' }])).toBe(7);
    expect(shiftOffset(5, [{ retain: 1 }, { delete: 2 }])).toBe(3);
    expect(shiftOffset(5, [{ retain: 3 }, { delete: 10 }])).toBe(3);
    expect(shiftOffset(2, [{ retain: 4 }, { delete: 1 }])).toBe(2);
  });
});

describe('TextEditor', () => {
  const file: TextFile = { text: 'one\ntwo', encoding: 'utf-8', byteOrderMark: false, lineEnding: '\r\n' };

  it('edits the text and saves it the way the file was', () => {
    const handle = createRef<TextEditorHandle>();
    const changes: string[] = [];
    render(<TextEditor ref={handle} file={file} onChange={(text) => changes.push(text)} labels={ITALIAN_TEXT_EDITOR_LABELS} />);
    const area = screen.getByLabelText('Testo') as HTMLTextAreaElement;
    expect(area.value).toBe('one\ntwo');
    fireEvent.change(area, { target: { value: 'one\ntwo\nthree' } });
    expect(changes.at(-1)).toBe('one\ntwo\nthree');
    expect(new TextDecoder().decode(handle.current!.saveText())).toBe('one\r\ntwo\r\nthree');
    expect(screen.getByText('CRLF')).toBeTruthy();
    expect(screen.getByText('UTF-8')).toBeTruthy();
  });

  it('shows a read-only text without the toolbar', () => {
    render(<TextEditor file={file} editable={false} />);
    expect((screen.getByLabelText('Text') as HTMLTextAreaElement).readOnly).toBe(true);
    expect(screen.queryByRole('toolbar')).toBeNull();
  });

  it('warns when a Windows-1252 file will be saved as UTF-8', () => {
    render(<TextEditor file={{ ...file, encoding: 'windows-1252', text: 'π' }} />);
    expect(screen.getByText(/saved as UTF-8/)).toBeTruthy();
  });

  it('writes into a shared text and shows what others write', () => {
    const local = new Y.Doc();
    const remote = new Y.Doc();
    local.on('update', (update: Uint8Array, origin: unknown) => {
      if (origin !== 'remote') Y.applyUpdate(remote, update, 'local');
    });
    remote.on('update', (update: Uint8Array, origin: unknown) => {
      if (origin !== 'local') Y.applyUpdate(local, update, 'remote');
    });
    remote.getText('text').insert(0, 'shared');
    render(<TextEditor file={file} collaboration={{ text: local.getText('text') }} />);
    const area = screen.getByLabelText('Text') as HTMLTextAreaElement;
    expect(area.value).toBe('shared');
    fireEvent.change(area, { target: { value: 'shared text' } });
    expect(remote.getText('text').toString()).toBe('shared text');
    act(() => {
      remote.getText('text').insert(0, '> ');
    });
    expect(area.value).toBe('> shared text');
  });
});
