// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type { Editor } from '@tiptap/core';
import { useEditorState } from '@tiptap/react';
import { useRef, useState, type ReactElement, type ReactNode } from 'react';
import type { Alignment } from '../model.js';
import type { DocumentEditorLabels } from './labels.js';

type Panel = 'link' | 'equation' | null;

function ToolButton({
  label,
  active = false,
  disabled = false,
  onClick,
  children,
}: Readonly<{
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}>): ReactElement {
  return (
    <button
      type="button"
      className="rely-tool"
      aria-label={label}
      title={label}
      aria-pressed={active}
      disabled={disabled}
      onMouseDown={(event) => event.preventDefault()}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

const ALIGNMENTS: [Alignment, keyof DocumentEditorLabels, string][] = [
  ['left', 'alignLeft', '≡'],
  ['center', 'alignCenter', '≣'],
  ['right', 'alignRight', '≢'],
  ['justify', 'alignJustify', '☰'],
];

/** The toolbar of the document editor, over the commands of `createRelyDocumentExtensions`. */
export function DocumentToolbar({
  editor,
  labels,
  onImage,
  onComment,
  onFootnote,
}: Readonly<{
  editor: Editor;
  labels: DocumentEditorLabels;
  onImage: (file: File) => void;
  onComment: () => void;
  onFootnote: () => void;
}>): ReactElement {
  const [panel, setPanel] = useState<Panel>(null);
  const [linkAddress, setLinkAddress] = useState('');
  const [latex, setLatex] = useState('');
  const fileInput = useRef<HTMLInputElement>(null);

  const state = useEditorState({
    editor,
    selector: ({ editor: current }) => ({
      bold: current.isActive('bold'),
      italic: current.isActive('italic'),
      underline: current.isActive('underline'),
      strike: current.isActive('strike'),
      code: current.isActive('code'),
      superscript: current.isActive('superscript'),
      subscript: current.isActive('subscript'),
      link: current.isActive('link'),
      bulletList: current.isActive('bulletList'),
      orderedList: current.isActive('orderedList'),
      blockquote: current.isActive('blockquote'),
      codeBlock: current.isActive('codeBlock'),
      table: current.isActive('table'),
      heading: ([1, 2, 3, 4, 5, 6] as const).find((level) => current.isActive('heading', { level })) ?? 0,
      align: (current.getAttributes('paragraph').align ?? current.getAttributes('heading').align ?? null) as
        | Alignment
        | null,
      colour: (current.getAttributes('textStyle').color as string | null) ?? '#000000',
      hasSelection: !current.state.selection.empty,
      canUndo: current.can().undo(),
      canRedo: current.can().redo(),
      canSink: current.can().sinkListItem('listItem'),
      canLift: current.can().liftListItem('listItem'),
    }),
  });

  const chain = () => editor.chain().focus();
  const openPanel = (next: Panel) => {
    setPanel(panel === next ? null : next);
    if (next === 'link') setLinkAddress((editor.getAttributes('link').href as string | undefined) ?? '');
  };

  return (
    <div className="rely-toolbar-wrap">
      <div className="rely-toolbar" role="toolbar" aria-label={labels.toolbar}>
        <div className="rely-tool-group">
          <ToolButton label={labels.undo} disabled={!state.canUndo} onClick={() => chain().undo().run()}>
            {'↶'}
          </ToolButton>
          <ToolButton label={labels.redo} disabled={!state.canRedo} onClick={() => chain().redo().run()}>
            {'↷'}
          </ToolButton>
        </div>
        <div className="rely-tool-group">
          <select
            className="rely-tool-select"
            aria-label={labels.textStyle}
            value={state.heading}
            onChange={(event) => {
              const level = Number(event.target.value);
              if (level === 0) chain().setParagraph().run();
              else chain().setHeading({ level: level as 1 | 2 | 3 | 4 | 5 | 6 }).run();
            }}
          >
            <option value={0}>{labels.paragraph}</option>
            {[1, 2, 3, 4, 5, 6].map((level) => (
              <option key={level} value={level}>
                {labels.heading(level)}
              </option>
            ))}
          </select>
        </div>
        <div className="rely-tool-group">
          <ToolButton label={labels.bold} active={state.bold} onClick={() => chain().toggleBold().run()}>
            <b>B</b>
          </ToolButton>
          <ToolButton label={labels.italic} active={state.italic} onClick={() => chain().toggleItalic().run()}>
            <i>I</i>
          </ToolButton>
          <ToolButton label={labels.underline} active={state.underline} onClick={() => chain().toggleUnderline().run()}>
            <u>U</u>
          </ToolButton>
          <ToolButton label={labels.strike} active={state.strike} onClick={() => chain().toggleStrike().run()}>
            <s>S</s>
          </ToolButton>
          <ToolButton label={labels.superscript} active={state.superscript} onClick={() => chain().toggleSuperscript().run()}>
            {'x²'}
          </ToolButton>
          <ToolButton label={labels.subscript} active={state.subscript} onClick={() => chain().toggleSubscript().run()}>
            {'x₂'}
          </ToolButton>
          <ToolButton label={labels.code} active={state.code} onClick={() => chain().toggleCode().run()}>
            {'</>'}
          </ToolButton>
          <label className="rely-tool rely-tool-colour" title={labels.textColour}>
            <span aria-hidden="true">A</span>
            <input
              type="color"
              aria-label={labels.textColour}
              value={state.colour}
              onChange={(event) => chain().setTextStyle({ color: event.target.value }).run()}
            />
          </label>
          <ToolButton label={labels.highlight} onClick={() => chain().setHighlight('#ffff00').run()}>
            <mark>ab</mark>
          </ToolButton>
          <ToolButton label={labels.clearHighlight} onClick={() => chain().unsetHighlight().run()}>
            ab
          </ToolButton>
        </div>
        <div className="rely-tool-group">
          {ALIGNMENTS.map(([align, key, glyph]) => (
            <ToolButton
              key={align}
              label={labels[key] as string}
              active={state.align === align}
              onClick={() => chain().setAlignment(state.align === align ? null : align).run()}
            >
              {glyph}
            </ToolButton>
          ))}
        </div>
        <div className="rely-tool-group">
          <ToolButton label={labels.bulletList} active={state.bulletList} onClick={() => chain().toggleBulletList().run()}>
            {'•'}
          </ToolButton>
          <ToolButton label={labels.orderedList} active={state.orderedList} onClick={() => chain().toggleOrderedList().run()}>
            1.
          </ToolButton>
          <ToolButton label={labels.outdent} disabled={!state.canLift} onClick={() => chain().liftListItem('listItem').run()}>
            {'⇤'}
          </ToolButton>
          <ToolButton label={labels.indent} disabled={!state.canSink} onClick={() => chain().sinkListItem('listItem').run()}>
            {'⇥'}
          </ToolButton>
          <ToolButton label={labels.quote} active={state.blockquote} onClick={() => chain().toggleBlockquote().run()}>
            {'“'}
          </ToolButton>
          <ToolButton label={labels.codeBlock} active={state.codeBlock} onClick={() => chain().toggleCodeBlock().run()}>
            {'{ }'}
          </ToolButton>
          <ToolButton label={labels.rule} onClick={() => chain().setHorizontalRule().run()}>
            {'―'}
          </ToolButton>
        </div>
        <div className="rely-tool-group">
          <ToolButton label={labels.link} active={state.link || panel === 'link'} onClick={() => openPanel('link')}>
            {'↗'}
          </ToolButton>
          <ToolButton label={labels.image} onClick={() => fileInput.current?.click()}>
            {'▣'}
          </ToolButton>
          <input
            ref={fileInput}
            type="file"
            accept="image/png,image/jpeg,image/gif,image/webp"
            hidden
            data-testid="rely-image-input"
            onChange={(event) => {
              const file = event.target.files?.[0];
              event.target.value = '';
              if (file) onImage(file);
            }}
          />
          <ToolButton label={labels.equation} active={panel === 'equation'} onClick={() => openPanel('equation')}>
            {'∑'}
          </ToolButton>
          <ToolButton
            label={labels.insertTable}
            onClick={() => chain().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()}
          >
            {'⊞'}
          </ToolButton>
        </div>
        {state.table ? (
          <div className="rely-tool-group">
            <ToolButton label={labels.addRow} onClick={() => chain().addRowAfter().run()}>
              {'+━'}
            </ToolButton>
            <ToolButton label={labels.addColumn} onClick={() => chain().addColumnAfter().run()}>
              {'+┃'}
            </ToolButton>
            <ToolButton label={labels.deleteRow} onClick={() => chain().deleteRow().run()}>
              {'−━'}
            </ToolButton>
            <ToolButton label={labels.deleteColumn} onClick={() => chain().deleteColumn().run()}>
              {'−┃'}
            </ToolButton>
            <ToolButton label={labels.mergeCells} onClick={() => chain().mergeCells().run()}>
              {'⬚'}
            </ToolButton>
            <ToolButton label={labels.splitCell} onClick={() => chain().splitCell().run()}>
              {'⊟'}
            </ToolButton>
            <ToolButton label={labels.deleteTable} onClick={() => chain().deleteTable().run()}>
              {'✖'}
            </ToolButton>
          </div>
        ) : null}
        <div className="rely-tool-group">
          <ToolButton label={labels.addComment} disabled={!state.hasSelection} onClick={onComment}>
            {'\u270e'}
          </ToolButton>
          <ToolButton label={labels.insertFootnote} onClick={onFootnote}>
            {'x\u00b9'}
          </ToolButton>
        </div>
        <div className="rely-tool-group">
          <ToolButton label={labels.acceptChanges} onClick={() => chain().acceptAllChanges().run()}>
            {'✓'}
          </ToolButton>
          <ToolButton label={labels.rejectChanges} onClick={() => chain().rejectAllChanges().run()}>
            {'✗'}
          </ToolButton>
          <ToolButton label={labels.clearFormatting} onClick={() => chain().unsetAllMarks().clearNodes().run()}>
            {'T̸'}
          </ToolButton>
        </div>
      </div>
      {panel === 'link' ? (
        <form
          className="rely-toolbar-panel"
          onSubmit={(event) => {
            event.preventDefault();
            const href = linkAddress.trim();
            if (href.length === 0) chain().extendMarkRange('link').unsetLink().run();
            else chain().extendMarkRange('link').setLink({ href }).run();
            setPanel(null);
          }}
        >
          <input
            type="url"
            aria-label={labels.linkAddress}
            placeholder="https://"
            value={linkAddress}
            onChange={(event) => setLinkAddress(event.target.value)}
            autoFocus
          />
          <button type="submit">{labels.applyLink}</button>
          <button
            type="button"
            onClick={() => {
              chain().extendMarkRange('link').unsetLink().run();
              setPanel(null);
            }}
          >
            {labels.removeLink}
          </button>
        </form>
      ) : null}
      {panel === 'equation' ? (
        <form
          className="rely-toolbar-panel"
          onSubmit={(event) => {
            event.preventDefault();
            if (latex.trim().length > 0) chain().insertMath(latex.trim()).run();
            setLatex('');
            setPanel(null);
          }}
        >
          <input
            type="text"
            aria-label={labels.equationLatex}
            placeholder="\frac{a}{b}"
            value={latex}
            onChange={(event) => setLatex(event.target.value)}
            autoFocus
          />
          <button type="submit">{labels.equation}</button>
        </form>
      ) : null}
    </div>
  );
}
