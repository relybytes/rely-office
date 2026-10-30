// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type ReactElement,
  type Ref,
} from 'react';
import type * as Y from 'yjs';
import { ENGLISH_TEXT_EDITOR_LABELS, type TextEditorLabels } from './labels.js';
import { lineAndColumn, textNeedsUnicode, writeTextFile, type TextFile } from './text-files.js';

/** What a host can ask of a mounted text editor. */
export interface TextEditorHandle {
  /** The file as it is now, its encoding and line ending kept. */
  getFile: () => TextFile;
  /** The bytes a save writes; see `writeTextFile` for when the encoding changes. */
  saveText: () => Uint8Array;
  focus: () => void;
}

/** The shared text a collaborative editor writes into, kept in sync by the host's session. */
export interface TextCollaboration {
  text: Y.Text;
}

export interface TextEditorProps {
  /** The file to open; the editor keeps its own state after that, so key it to open another. */
  file: TextFile;
  /** Called after every change with the text as it is now. */
  onChange?: (text: string) => void;
  /** False shows the text without letting it change. */
  editable?: boolean;
  labels?: Partial<TextEditorLabels>;
  /** Start with long lines wrapped; false scrolls them sideways. */
  wrapLines?: boolean;
  /**
   * Edit a shared text instead of the file's own: the text of `file` is ignored, and what others
   * write appears as they write it.
   */
  collaboration?: TextCollaboration;
  className?: string;
}

/** Where a change between two texts is: the prefix and the suffix they share, and what replaced the rest. */
export function textChange(before: string, after: string): { at: number; removed: number; inserted: string } {
  const shortest = Math.min(before.length, after.length);
  let prefix = 0;
  while (prefix < shortest && before.charCodeAt(prefix) === after.charCodeAt(prefix)) prefix += 1;
  let suffix = 0;
  while (
    suffix < shortest - prefix &&
    before.charCodeAt(before.length - 1 - suffix) === after.charCodeAt(after.length - 1 - suffix)
  ) {
    suffix += 1;
  }
  return {
    at: prefix,
    removed: before.length - prefix - suffix,
    inserted: after.slice(prefix, after.length - suffix),
  };
}

type Delta = { insert?: string | object; delete?: number; retain?: number }[];

/** An offset moved through a remote change, so a caret stays on the text it was beside. */
export function shiftOffset(offset: number, delta: Delta): number {
  let position = 0;
  let shifted = offset;
  for (const step of delta) {
    if (position > offset) break;
    if (step.retain !== undefined) {
      position += step.retain;
    } else if (step.insert !== undefined) {
      shifted += typeof step.insert === 'string' ? step.insert.length : 1;
    } else if (step.delete !== undefined) {
      if (position < offset) shifted -= Math.min(step.delete, offset - position);
      position += step.delete;
    }
  }
  return Math.max(0, shifted);
}

/**
 * A plain text file in a monospaced editor: the encoding, byte order mark and line ending of the
 * file are kept on save, and the status line says where the caret is.
 */
export const TextEditor = forwardRef(function TextEditor(
  { file, onChange, editable = true, labels, wrapLines = true, collaboration, className }: TextEditorProps,
  ref: Ref<TextEditorHandle>,
): ReactElement {
  const words = useMemo(() => ({ ...ENGLISH_TEXT_EDITOR_LABELS, ...labels }), [labels]);
  const area = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState(() => collaboration?.text.toString() ?? file.text);
  const textRef = useRef(text);
  const [caret, setCaret] = useState(0);
  const [wrap, setWrap] = useState(wrapLines);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const origin = useMemo(() => ({}), []);

  const show = useCallback((next: string) => {
    textRef.current = next;
    setText(next);
    onChangeRef.current?.(next);
  }, []);

  useEffect(() => {
    if (!collaboration) return;
    const shared = collaboration.text;
    show(shared.toString());
    const observe = (event: Y.YTextEvent) => {
      if (event.transaction.origin === origin) return;
      const element = area.current;
      const start = element ? shiftOffset(element.selectionStart, event.delta as Delta) : 0;
      const end = element ? shiftOffset(element.selectionEnd, event.delta as Delta) : 0;
      const next = shared.toString();
      show(next);
      if (element && globalThis.document?.activeElement === element) {
        queueMicrotask(() => element.setSelectionRange(start, end));
      }
    };
    shared.observe(observe);
    return () => shared.unobserve(observe);
  }, [collaboration, origin, show]);

  const write = (next: string) => {
    const before = textRef.current;
    if (collaboration) {
      const change = textChange(before, next);
      const shared = collaboration.text;
      shared.doc?.transact(() => {
        if (change.removed > 0) shared.delete(change.at, change.removed);
        if (change.inserted.length > 0) shared.insert(change.at, change.inserted);
      }, origin);
    }
    show(next);
  };

  useImperativeHandle(
    ref,
    () => ({
      getFile: () => ({ ...file, text: textRef.current }),
      saveText: () => writeTextFile({ ...file, text: textRef.current }).bytes,
      focus: () => area.current?.focus(),
    }),
    [file],
  );

  const position = lineAndColumn(text, caret);
  const lossy = file.encoding === 'windows-1252' && textNeedsUnicode(text);
  const track = () => setCaret(area.current?.selectionStart ?? 0);

  return (
    <div className={['rely-text-editor', className].filter(Boolean).join(' ')}>
      {editable ? (
        <div className="rely-toolbar" role="toolbar" aria-label={words.toolbar}>
          <label className="rely-tool-check">
            <input type="checkbox" checked={wrap} onChange={(event) => setWrap(event.target.checked)} />
            {words.wrapLines}
          </label>
        </div>
      ) : null}
      <textarea
        ref={area}
        className="rely-text-area"
        aria-label={words.text}
        value={text}
        readOnly={!editable}
        spellCheck={false}
        wrap={wrap ? 'soft' : 'off'}
        onChange={(event) => {
          write(event.target.value);
          track();
        }}
        onSelect={track}
        onKeyUp={track}
        onClick={track}
        onKeyDown={(event) => {
          if (event.key !== 'Tab' || !editable || event.ctrlKey || event.metaKey || event.altKey) return;
          event.preventDefault();
          const element = event.currentTarget;
          const start = element.selectionStart;
          const end = element.selectionEnd;
          write(`${text.slice(0, start)}\t${text.slice(end)}`);
          queueMicrotask(() => element.setSelectionRange(start + 1, start + 1));
        }}
      />
      <div className="rely-text-status" aria-live="polite">
        <span>{words.position(position.line, position.column)}</span>
        <span>{words.encoding(file.encoding)}</span>
        <span>{words.lineEnding(file.lineEnding)}</span>
        {lossy ? <span className="rely-text-warning">{words.savedAsUnicode}</span> : null}
      </div>
    </div>
  );
});
