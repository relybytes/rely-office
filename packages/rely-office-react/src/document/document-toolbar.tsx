// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import type { Editor } from '@tiptap/core';
import { useEditorState } from '@tiptap/react';
import { useEffect, useRef, useState, type ReactElement, type ReactNode } from 'react';
import type { Alignment, ImageAnchor, ImageWrap, NamedStyle, ParagraphFormat } from '../model.js';
import { FONT_FAMILIES, FONT_SIZES, LINE_SPACINGS } from './formatting.js';
import type { DocumentEditorLabels } from './labels.js';
import { searchKey } from './search.js';
import { fontStack, fontSubstitute, isFontAvailable, pickerFamilies, stackFamily } from './fonts.js';

type Panel = 'link' | 'equation' | 'find' | null;

const NAMED_STYLES: [NamedStyle, keyof DocumentEditorLabels][] = [
  ['title', 'styleTitle'],
  ['subtitle', 'styleSubtitle'],
  ['intenseQuote', 'styleIntenseQuote'],
  ['caption', 'styleCaption'],
  ['noSpacing', 'styleNoSpacing'],
];

/** A line spacing as a multiple of a single line: 360 is 1.5. */
function spacingName(value: number): string {
  return String(Math.round((value / 240) * 100) / 100);
}

/** The family a CSS font list starts with, without quotes. */
function firstFamily(family: string | null | undefined): string {
  return stackFamily(family) ?? '';
}

/** A family as the picker names it: with the bundled face it is shown with when the browser lacks it. */
function familyLabel(family: string, labels: DocumentEditorLabels): string {
  const substitute = fontSubstitute(family);
  if (substitute.toLowerCase() === family.toLowerCase() || isFontAvailable(family)) return family;
  return `${family} (${labels.fontShownAs(substitute)})`;
}

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

/** The EMU of a CSS pixel. */
const EMU_PER_PIXEL = 9_525;

const WRAPS: [ImageWrap['type'] | 'inline', keyof DocumentEditorLabels][] = [
  ['inline', 'wrapInline'],
  ['around', 'wrapAround'],
  ['contour', 'wrapContour'],
  ['topAndBottom', 'wrapTopAndBottom'],
  ['behind', 'wrapBehind'],
  ['inFront', 'wrapInFront'],
];

/** The anchor an image in the line gets when it is made to float: at the left of the column,
 * at the top of its paragraph, with Word's eighth of an inch kept beside it. */
export function floatingAnchorFor(attributes: Record<string, unknown>, wrap: ImageWrap): ImageAnchor {
  const pixels = (value: unknown) => (Number.isFinite(Number(value)) && Number(value) > 0 ? Number(value) : 100);
  return {
    horizontal: { base: 'column', offsetEmu: 0, align: 'start' },
    vertical: { base: 'paragraph', offsetEmu: 0 },
    widthEmu: Math.round(pixels(attributes.width) * EMU_PER_PIXEL),
    heightEmu: Math.round(pixels(attributes.height) * EMU_PER_PIXEL),
    wrap,
    distanceEmu: [0, 0, 114_300, 114_300],
  };
}

/** How the text goes around the chosen image and which side it stands on. */
function ImageLayout({
  labels,
  attributes,
  onAnchor,
}: Readonly<{
  labels: DocumentEditorLabels;
  attributes: Record<string, unknown>;
  onAnchor: (anchor: ImageAnchor | null) => void;
}>): ReactElement {
  const anchor = (attributes.anchor ?? null) as ImageAnchor | null;
  const wrap = anchor?.wrap.type ?? 'inline';
  const side = anchor?.horizontal.align ?? (anchor && anchor.horizontal.offsetEmu > 2_743_200 ? 'end' : 'start');
  return (
    <div className="rely-tool-group">
      <select
        className="rely-tool-select"
        aria-label={labels.imageWrap}
        value={wrap}
        onChange={(event) => {
          const chosen = event.target.value as ImageWrap['type'] | 'inline';
          if (chosen === 'inline') onAnchor(null);
          else if (chosen !== 'contour') {
            const next: ImageWrap = { type: chosen };
            onAnchor(anchor ? { ...anchor, wrap: next } : floatingAnchorFor(attributes, next));
          }
        }}
      >
        {WRAPS.filter(([kind]) => kind !== 'contour' || wrap === 'contour').map(([kind, key]) => (
          <option key={kind} value={kind}>
            {labels[key] as string}
          </option>
        ))}
      </select>
      <select
        className="rely-tool-select"
        aria-label={labels.imagePosition}
        value={side}
        disabled={anchor === null}
        onChange={(event) => {
          if (!anchor) return;
          const align = event.target.value as 'start' | 'center' | 'end';
          onAnchor({ ...anchor, horizontal: { base: 'column', offsetEmu: 0, align } });
        }}
      >
        <option value="start">{labels.positionLeft}</option>
        <option value="center">{labels.positionCenter}</option>
        <option value="end">{labels.positionRight}</option>
      </select>
    </div>
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
  trackChanges,
  onTrackChanges,
  pageView,
  onPageView,
  findRequest = 0,
  documentFamilies = [],
}: Readonly<{
  editor: Editor;
  labels: DocumentEditorLabels;
  onImage: (file: File) => void;
  onComment: () => void;
  onFootnote: () => void;
  trackChanges: boolean;
  onTrackChanges: (enabled: boolean) => void;
  pageView: boolean;
  onPageView: (enabled: boolean) => void;
  /** Raised by the editor on Ctrl+F, to open the find panel. */
  findRequest?: number;
  /** The families the document names, listed before the standard ones. */
  documentFamilies?: readonly string[];
}>): ReactElement {
  const [panel, setPanel] = useState<Panel>(null);
  const [linkAddress, setLinkAddress] = useState('');
  const [latex, setLatex] = useState('');
  const [findText, setFindText] = useState('');
  const [replaceText, setReplaceText] = useState('');
  const [matchCase, setMatchCase] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (findRequest > 0) setPanel('find');
  }, [findRequest]);

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
      format: ((current.getAttributes('paragraph').format ?? current.getAttributes('heading').format ?? null) as
        | ParagraphFormat
        | null),
      fontFamily: firstFamily(current.getAttributes('textStyle').fontFamily as string | null),
      fontSize: (current.getAttributes('textStyle').fontSize as string | null) ?? '',
      search: searchKey.getState(current.state) ?? null,
      align: (current.getAttributes('paragraph').align ?? current.getAttributes('heading').align ?? null) as
        | Alignment
        | null,
      colour: (current.getAttributes('textStyle').color as string | null) ?? '#000000',
      hasSelection: !current.state.selection.empty,
      canUndo: current.can().undo(),
      canRedo: current.can().redo(),
      canSink: current.can().sinkListItem('listItem'),
      canLift: current.can().liftListItem('listItem'),
      image: current.isActive('image'),
      imageAttributes: current.isActive('image') ? current.getAttributes('image') : null,
    }),
  });

  const chain = () => editor.chain().focus();
  const openPanel = (next: Panel) => {
    if (panel === 'find' && next !== 'find') editor.commands.setSearch('');
    if (panel === 'find' && next === 'find') {
      editor.commands.setSearch('');
      setPanel(null);
      return;
    }
    setPanel(panel === next ? null : next);
    if (next === 'link') setLinkAddress((editor.getAttributes('link').href as string | undefined) ?? '');
  };
  const search = (text: string, caseSensitive: boolean) => {
    setFindText(text);
    setMatchCase(caseSensitive);
    editor.commands.setSearch(text, caseSensitive);
  };
  const styleValue = state.heading > 0 ? String(state.heading) : (state.format?.style ?? '0');
  const sizePoints = /^(\d+(?:\.\d+)?)pt$/.exec(state.fontSize)?.[1] ?? '';
  const spacing = state.format?.lineSpacing?.rule === 'auto' ? String(state.format.lineSpacing.value) : '';
  const families = pickerFamilies(
    state.fontFamily === '' ? documentFamilies : [state.fontFamily, ...documentFamilies],
    FONT_FAMILIES,
  );
  const sizes = sizePoints === '' || FONT_SIZES.includes(Number(sizePoints)) ? FONT_SIZES : [Number(sizePoints), ...FONT_SIZES];
  const matches = state.search?.matches.length ?? 0;

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
            value={styleValue}
            onChange={(event) => {
              const value = event.target.value;
              const level = Number(value);
              if (value === '0') chain().setNamedStyle(null).run();
              else if (Number.isInteger(level) && level > 0) chain().setNamedStyle({ heading: level as 1 | 2 | 3 | 4 | 5 | 6 }).run();
              else chain().setNamedStyle(value as NamedStyle).run();
            }}
          >
            <option value="0">{labels.paragraph}</option>
            {[1, 2, 3, 4, 5, 6].map((level) => (
              <option key={level} value={String(level)}>
                {labels.heading(level)}
              </option>
            ))}
            {NAMED_STYLES.map(([style, key]) => (
              <option key={style} value={style}>
                {labels[key] as string}
              </option>
            ))}
          </select>
          <select
            className="rely-tool-select"
            aria-label={labels.fontFamily}
            value={state.fontFamily}
            onChange={(event) => chain().setTextStyle({ fontFamily: event.target.value || null }).run()}
          >
            <option value="">{labels.defaultFont}</option>
            {families.map((family) => (
              <option key={family} value={family} style={{ fontFamily: fontStack(family) }}>
                {familyLabel(family, labels)}
              </option>
            ))}
          </select>
          <select
            className="rely-tool-select rely-tool-size"
            aria-label={labels.fontSize}
            value={sizePoints}
            onChange={(event) => chain().setTextStyle({ fontSize: event.target.value ? `${event.target.value}pt` : null }).run()}
          >
            <option value="">{'–'}</option>
            {sizes.map((size) => (
              <option key={size} value={String(size)}>
                {String(size)}
              </option>
            ))}
          </select>
          <select
            className="rely-tool-select rely-tool-size"
            aria-label={labels.lineSpacing}
            title={labels.lineSpacing}
            value={spacing}
            onChange={(event) =>
              chain()
                .setParagraphFormat({
                  lineSpacing: event.target.value ? { rule: 'auto', value: Number(event.target.value) } : null,
                })
                .run()
            }
          >
            <option value="">{'↕'}</option>
            {LINE_SPACINGS.map((value) => (
              <option key={value} value={String(value)}>
                {spacingName(value)}
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
          <ToolButton label={labels.pageBreak} onClick={() => chain().insertPageBreak().run()}>
            {'⤓'}
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
        {state.image && state.imageAttributes ? (
          <ImageLayout
            labels={labels}
            attributes={state.imageAttributes}
            onAnchor={(anchor) => {
              // The image stays chosen, so its layout can be changed again at once.
              const at = editor.state.selection.from;
              chain().updateAttributes('image', { anchor }).setNodeSelection(at).run();
            }}
          />
        ) : null}
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
          <ToolButton label={labels.trackChanges} active={trackChanges} onClick={() => onTrackChanges(!trackChanges)}>
            {'✎±'}
          </ToolButton>
          <ToolButton label={labels.acceptChange} onClick={() => chain().acceptChangesInSelection().run()}>
            {'✓'}
          </ToolButton>
          <ToolButton label={labels.rejectChange} onClick={() => chain().rejectChangesInSelection().run()}>
            {'✗'}
          </ToolButton>
          <ToolButton label={labels.acceptChanges} onClick={() => chain().acceptAllChanges().run()}>
            {'✓✓'}
          </ToolButton>
          <ToolButton label={labels.rejectChanges} onClick={() => chain().rejectAllChanges().run()}>
            {'✗✗'}
          </ToolButton>
          <ToolButton label={labels.clearFormatting} onClick={() => chain().unsetAllMarks().clearNodes().run()}>
            {'T̸'}
          </ToolButton>
        </div>
        <div className="rely-tool-group">
          <ToolButton label={labels.find} active={panel === 'find'} onClick={() => openPanel('find')}>
            {'⌕'}
          </ToolButton>
          <ToolButton label={labels.pageView} active={pageView} onClick={() => onPageView(!pageView)}>
            {'▯'}
          </ToolButton>
        </div>
      </div>
      {panel === 'find' ? (
        <form
          className="rely-toolbar-panel rely-find-panel"
          role="search"
          onSubmit={(event) => {
            event.preventDefault();
            editor.commands.findNext();
          }}
          onKeyDown={(event) => {
            if (event.key !== 'Escape') return;
            editor.commands.setSearch('');
            setPanel(null);
            editor.commands.focus();
          }}
        >
          <input
            type="search"
            aria-label={labels.findText}
            placeholder={labels.findText}
            value={findText}
            onChange={(event) => search(event.target.value, matchCase)}
            autoFocus
          />
          <span className="rely-find-count" aria-live="polite">
            {labels.findCount((state.search?.current ?? -1) + 1, matches)}
          </span>
          <button type="button" aria-label={labels.findPrevious} title={labels.findPrevious} onClick={() => editor.commands.findPrevious()}>
            {'↑'}
          </button>
          <button type="submit" aria-label={labels.findNext} title={labels.findNext}>
            {'↓'}
          </button>
          <label className="rely-find-case">
            <input type="checkbox" checked={matchCase} onChange={(event) => search(findText, event.target.checked)} />
            <span>{labels.matchCase}</span>
          </label>
          <input
            type="text"
            aria-label={labels.replaceText}
            placeholder={labels.replaceText}
            value={replaceText}
            onChange={(event) => setReplaceText(event.target.value)}
          />
          <button type="button" disabled={matches === 0} onClick={() => editor.commands.replaceMatch(replaceText)}>
            {labels.replace}
          </button>
          <button type="button" disabled={matches === 0} onClick={() => editor.commands.replaceAllMatches(replaceText)}>
            {labels.replaceAll}
          </button>
        </form>
      ) : null}
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
