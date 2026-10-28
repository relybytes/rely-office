// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { Extension } from '@tiptap/core';
import type { LineSpacing, NamedStyle, ParagraphFormat } from '../model.js';

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    relyParagraphFormat: {
      /** Merge into what the selected paragraphs set themselves; a null field goes back to the style's. */
      setParagraphFormat: (format: { [Key in keyof ParagraphFormat]?: ParagraphFormat[Key] | null }) => ReturnType;
      /**
       * Give the selected blocks a named style of Word: a heading level, a style of the list,
       * or the plain paragraph with `null`.
       */
      setNamedStyle: (style: NamedStyle | { heading: 1 | 2 | 3 | 4 | 5 | 6 } | null) => ReturnType;
      /** Start a new page at the cursor: the text after it moves to a paragraph that opens one. */
      insertPageBreak: () => ReturnType;
    };
  }
}

const FORMATTED = ['paragraph', 'heading'];

/** The fonts the toolbar offers, the ones Word and LibreOffice install everywhere. */
export const FONT_FAMILIES = [
  'Calibri',
  'Arial',
  'Helvetica',
  'Times New Roman',
  'Georgia',
  'Cambria',
  'Garamond',
  'Verdana',
  'Tahoma',
  'Trebuchet MS',
  'Courier New',
];

/** The sizes the toolbar offers, in points. */
export const FONT_SIZES = [8, 9, 10, 10.5, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 60, 72];

/** The line spacings the toolbar offers, as multiples of a single line, in 240ths. */
export const LINE_SPACINGS = [240, 276, 360, 480, 600, 720];

/** A format without the fields it does not set; null when it sets nothing. */
export function cleanFormat(format: Record<string, unknown> | null | undefined): ParagraphFormat | null {
  if (!format || typeof format !== 'object') return null;
  const cleaned: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(format)) {
    if (value === null || value === undefined || value === false) continue;
    cleaned[key] = value;
  }
  return Object.keys(cleaned).length > 0 ? (cleaned as ParagraphFormat) : null;
}

function points(twips: number): string {
  return `${Math.round((twips / 20) * 100) / 100}pt`;
}

/** The CSS of a line spacing: a multiple, or a height in points. */
export function lineHeightCss(spacing: LineSpacing): string {
  return spacing.rule === 'auto' ? String(Math.round((spacing.value / 240) * 1.2 * 1000) / 1000) : points(spacing.value);
}

/** How a paragraph format shows in the editor: inline style and the class of its named style. */
export function formatHtml(format: ParagraphFormat | null): Record<string, string> {
  if (!format) return {};
  const style: string[] = [];
  if (format.spaceBeforeTwips !== undefined) style.push(`margin-top: ${points(format.spaceBeforeTwips)}`);
  if (format.spaceAfterTwips !== undefined) style.push(`margin-bottom: ${points(format.spaceAfterTwips)}`);
  if (format.lineSpacing) style.push(`line-height: ${lineHeightCss(format.lineSpacing)}`);
  if (format.indentLeftTwips !== undefined) style.push(`margin-left: ${points(format.indentLeftTwips)}`);
  if (format.indentRightTwips !== undefined) style.push(`margin-right: ${points(format.indentRightTwips)}`);
  if (format.indentFirstLineTwips !== undefined) style.push(`text-indent: ${points(format.indentFirstLineTwips)}`);
  const classes = [
    format.style ? `rely-style-${format.style}` : '',
    format.pageBreakBefore ? 'rely-page-break-before' : '',
  ].filter(Boolean);
  return {
    ...(style.length > 0 ? { style: style.join('; ') } : {}),
    ...(classes.length > 0 ? { class: classes.join(' ') } : {}),
    'data-format': JSON.stringify(format),
  };
}

/** What a paragraph and a heading set themselves, kept whole for the tree and drawn as CSS. */
export const RelyParagraphFormat = Extension.create({
  name: 'relyParagraphFormat',
  addGlobalAttributes() {
    return [
      {
        types: FORMATTED,
        attributes: {
          format: {
            default: null,
            keepOnSplit: true,
            parseHTML: (element) => {
              const text = element.getAttribute('data-format');
              if (!text) return null;
              try {
                return cleanFormat(JSON.parse(text) as Record<string, unknown>);
              } catch {
                return null;
              }
            },
            renderHTML: (attributes) => formatHtml(attributes.format as ParagraphFormat | null),
          },
        },
      },
    ];
  },
  addCommands() {
    return {
      setParagraphFormat:
        (patch) =>
        ({ tr, state, dispatch }) => {
          const { from, to } = state.selection;
          let changed = false;
          state.doc.nodesBetween(from, to, (node, position) => {
            if (!FORMATTED.includes(node.type.name)) return true;
            const format = cleanFormat({ ...(node.attrs.format ?? {}), ...patch });
            tr.setNodeMarkup(position, undefined, { ...node.attrs, format });
            changed = true;
            return false;
          });
          if (changed && dispatch) dispatch(tr);
          return changed;
        },
      setNamedStyle:
        (style) =>
        ({ chain, state }) => {
          const { $from } = state.selection;
          const current = (state.doc.nodeAt($from.before($from.depth))?.attrs.format ?? null) as ParagraphFormat | null;
          if (style !== null && typeof style === 'object') {
            return chain()
              .setHeading({ level: style.heading })
              .setParagraphFormat({ style: null })
              .run();
          }
          const base = chain().setParagraph();
          return base.setParagraphFormat({ style: style ?? null, pageBreakBefore: current?.pageBreakBefore ?? null }).run();
        },
      insertPageBreak:
        () =>
        ({ chain }) =>
          chain().splitBlock().setParagraphFormat({ pageBreakBefore: true }).run(),
    };
  },
});
