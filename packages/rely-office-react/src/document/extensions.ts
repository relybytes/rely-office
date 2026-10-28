// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { Extension, Mark, Node, mergeAttributes, type AnyExtension } from '@tiptap/core';
import Image from '@tiptap/extension-image';
import { TableKit } from '@tiptap/extension-table';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { Plugin, PluginKey, type EditorState, type Transaction } from '@tiptap/pm/state';
import type { EditorView } from '@tiptap/pm/view';
import StarterKit from '@tiptap/starter-kit';
import type { Alignment } from '../model.js';
import { RelyParagraphFormat } from './formatting.js';
import { imageNodeView } from './image-view.js';
import { RelyPagination } from './pagination.js';
import { RelySearch } from './search.js';
import { RelyTrackRecording, TRACK_SKIP } from './track-changes.js';

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    relyAlignment: {
      /** Set where the lines of the selected paragraphs and headings sit; null for the start. */
      setAlignment: (align: Alignment | null) => ReturnType;
    };
    relyScript: {
      toggleSuperscript: () => ReturnType;
      toggleSubscript: () => ReturnType;
    };
    relyTextStyle: {
      /** Set or clear (null) the font, size or colour of the selected text. */
      setTextStyle: (style: { fontFamily?: string | null; fontSize?: string | null; color?: string | null }) => ReturnType;
    };
    relyHighlight: {
      setHighlight: (color: string) => ReturnType;
      unsetHighlight: () => ReturnType;
    };
    relyMath: {
      insertMath: (latex: string, display?: boolean) => ReturnType;
    };
    relyNoteReference: {
      /** Insert a reference to the note of an index of the document's notes. */
      insertNoteReference: (note: number) => ReturnType;
    };
    relyReferences: {
      /** Take out the marks and anchors of the comments of these indexes. */
      removeComments: (comments: number[]) => ReturnType;
      /** Take out the references to the notes of these indexes. */
      removeNoteReferences: (notes: number[]) => ReturnType;
    };
    relyTrackedChanges: {
      /** Keep every tracked insertion and drop every tracked deletion. */
      acceptAllChanges: () => ReturnType;
      /** Drop every tracked insertion and keep every tracked deletion. */
      rejectAllChanges: () => ReturnType;
    };
  }
}

const ALIGNABLE = ['paragraph', 'heading'];

/** The `align` of a paragraph or heading, as the tree names it. */
export const RelyAlignment = Extension.create({
  name: 'relyAlignment',
  addGlobalAttributes() {
    return [
      {
        types: ALIGNABLE,
        attributes: {
          align: {
            default: null,
            parseHTML: (element) => {
              const align = element.style.textAlign;
              return ['left', 'center', 'right', 'justify'].includes(align) ? align : null;
            },
            renderHTML: (attributes) =>
              attributes.align ? { style: `text-align: ${attributes.align}` } : {},
          },
        },
      },
    ];
  },
  addCommands() {
    return {
      setAlignment:
        (align) =>
        ({ commands }) =>
          ALIGNABLE.map((type) => commands.updateAttributes(type, { align })).some(Boolean),
    };
  },
});

/** The `listId` of a list: lists apart in the text with the same id number as one. */
export const RelyListIdentity = Extension.create({
  name: 'relyListIdentity',
  addGlobalAttributes() {
    return [
      {
        types: ['bulletList', 'orderedList'],
        attributes: {
          listId: {
            default: null,
            parseHTML: (element) => {
              const value = Number(element.getAttribute('data-list-id'));
              return Number.isInteger(value) && value > 0 ? value : null;
            },
            renderHTML: (attributes) =>
              attributes.listId === null ? {} : { 'data-list-id': String(attributes.listId) },
          },
        },
      },
    ];
  },
});

/** An image inside a line, as Word places them, naming its item of the document's media. */
export const RelyImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      width: { default: null },
      height: { default: null },
      media: {
        default: null,
        parseHTML: (element) => {
          const value = Number(element.getAttribute('data-media'));
          return Number.isInteger(value) && value >= 0 ? value : null;
        },
        renderHTML: (attributes) =>
          attributes.media === null ? {} : { 'data-media': String(attributes.media) },
      },
    };
  },
  addNodeView() {
    return imageNodeView;
  },
}).configure({ inline: true, allowBase64: true });

export const RelySuperscript = Mark.create({
  name: 'superscript',
  excludes: 'subscript',
  parseHTML: () => [{ tag: 'sup' }],
  renderHTML: ({ HTMLAttributes }) => ['sup', HTMLAttributes, 0],
  addCommands() {
    return { toggleSuperscript: () => ({ commands }) => commands.toggleMark(this.name) };
  },
});

export const RelySubscript = Mark.create({
  name: 'subscript',
  excludes: 'superscript',
  parseHTML: () => [{ tag: 'sub' }],
  renderHTML: ({ HTMLAttributes }) => ['sub', HTMLAttributes, 0],
  addCommands() {
    return { toggleSubscript: () => ({ commands }) => commands.toggleMark(this.name) };
  },
});

/** The font, size and colour a run sets itself, as CSS values. */
export const RelyTextStyle = Mark.create({
  name: 'textStyle',
  addAttributes() {
    const style = (property: string) => ({
      default: null,
      parseHTML: (element: HTMLElement) => element.style.getPropertyValue(property) || null,
    });
    return { fontFamily: style('font-family'), fontSize: style('font-size'), color: style('color') };
  },
  parseHTML: () => [{ tag: 'span[style]' }],
  renderHTML: ({ mark, HTMLAttributes }) => {
    const { fontFamily, fontSize, color } = mark.attrs as Record<string, string | null>;
    const style = [
      fontFamily ? `font-family: ${fontFamily}` : '',
      fontSize ? `font-size: ${fontSize}` : '',
      color ? `color: ${color}` : '',
    ]
      .filter(Boolean)
      .join('; ');
    return ['span', mergeAttributes(HTMLAttributes, style ? { style } : {}), 0];
  },
  addCommands() {
    return {
      setTextStyle:
        (style) =>
        ({ editor, commands }) => {
          const merged = { ...editor.getAttributes(this.name), ...style };
          const empty = Object.values(merged).every((value) => value === null || value === undefined);
          return empty ? commands.unsetMark(this.name) : commands.setMark(this.name, merged);
        },
    };
  },
});

export const RelyHighlight = Mark.create({
  name: 'highlight',
  addAttributes() {
    return {
      color: {
        default: '#ffff00',
        parseHTML: (element: HTMLElement) => element.style.backgroundColor || '#ffff00',
      },
    };
  },
  parseHTML: () => [{ tag: 'mark' }],
  renderHTML: ({ mark, HTMLAttributes }) => [
    'mark',
    mergeAttributes(HTMLAttributes, { style: `background-color: ${String(mark.attrs.color)}` }),
    0,
  ],
  addCommands() {
    return {
      setHighlight: (color) => ({ commands }) => commands.setMark(this.name, { color }),
      unsetHighlight: () => ({ commands }) => commands.unsetMark(this.name),
    };
  },
});

/** Text a comment of the document is on. */
export const RelyComment = Mark.create({
  name: 'comment',
  inclusive: false,
  excludes: '',
  addAttributes() {
    return {
      comment: {
        default: null,
        parseHTML: (element: HTMLElement) => Number(element.getAttribute('data-comment')),
      },
    };
  },
  parseHTML: () => [{ tag: 'span[data-comment]' }],
  renderHTML: ({ mark, HTMLAttributes }) => [
    'span',
    mergeAttributes(HTMLAttributes, { class: 'rely-comment', 'data-comment': String(mark.attrs.comment) }),
    0,
  ],
});

function trackedChange(name: 'insertion' | 'deletion', tag: 'ins' | 'del') {
  return Mark.create({
    name,
    inclusive: false,
    excludes: name === 'insertion' ? 'deletion' : 'insertion',
    addAttributes() {
      return {
        author: { default: '', parseHTML: (element: HTMLElement) => element.getAttribute('data-author') ?? '' },
        date: { default: null, parseHTML: (element: HTMLElement) => element.getAttribute('datetime') },
      };
    },
    parseHTML: () => [{ tag }],
    renderHTML: ({ mark, HTMLAttributes }) => [
      tag,
      mergeAttributes(HTMLAttributes, {
        class: `rely-${name}`,
        'data-author': String(mark.attrs.author ?? ''),
        title: String(mark.attrs.author ?? ''),
        ...(mark.attrs.date ? { datetime: String(mark.attrs.date) } : {}),
      }),
      0,
    ],
  });
}

export const RelyInsertion = trackedChange('insertion', 'ins');
export const RelyDeletion = trackedChange('deletion', 'del');

/** Accept or reject every tracked change of the document at once. */
export const RelyTrackedChanges = Extension.create({
  name: 'relyTrackedChanges',
  addCommands() {
    const settle =
      (keep: 'insertion' | 'deletion') =>
      () =>
      ({ tr, state, dispatch }: { tr: Transaction; state: EditorState; dispatch?: (tr: Transaction) => void }) => {
        const drop = keep === 'insertion' ? 'deletion' : 'insertion';
        const keepType = state.schema.marks[keep];
        const dropType = state.schema.marks[drop];
        if (!keepType || !dropType) return false;
        const removals: [number, number][] = [];
        state.doc.descendants((node: ProseMirrorNode, position: number) => {
          if (!node.isText) return;
          if (dropType.isInSet(node.marks)) removals.push([position, position + node.nodeSize]);
        });
        if (dispatch) {
          tr.removeMark(0, state.doc.content.size, keepType);
          for (const [from, to] of removals.reverse()) tr.delete(from, to);
          tr.setMeta(TRACK_SKIP, true);
          dispatch(tr);
        }
        return true;
      };
    return { acceptAllChanges: settle('insertion'), rejectAllChanges: settle('deletion') };
  },
});

/** Remove what refers to a comment or a note, for a comment or a note the person deletes. */
export const RelyReferences = Extension.create({
  name: 'relyReferences',
  addCommands() {
    const remove =
      (kind: 'comment' | 'note') =>
      (indexes: number[]) =>
      ({ tr, state, dispatch }: { tr: Transaction; state: EditorState; dispatch?: (tr: Transaction) => void }) => {
        const wanted = new Set(indexes);
        const deletions: [number, number][] = [];
        state.doc.descendants((node: ProseMirrorNode, position: number) => {
          if (kind === 'comment') {
            if (node.type.name === 'commentAnchor' && wanted.has(Number(node.attrs.comment))) {
              deletions.push([position, position + node.nodeSize]);
            }
            for (const mark of node.marks) {
              if (mark.type.name === 'comment' && wanted.has(Number(mark.attrs.comment))) {
                tr.removeMark(position, position + node.nodeSize, mark);
              }
            }
          } else if (node.type.name === 'noteReference' && wanted.has(Number(node.attrs.note))) {
            deletions.push([position, position + node.nodeSize]);
          }
        });
        if (dispatch) {
          for (const [from, to] of deletions.reverse()) tr.delete(from, to);
          tr.setMeta(TRACK_SKIP, true);
          dispatch(tr);
        }
        return true;
      };
    return { removeComments: remove('comment'), removeNoteReferences: remove('note') };
  },
});

/** A footnote or endnote reference, shown with its number. */
export const RelyNoteReference = Node.create({
  name: 'noteReference',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,
  addAttributes() {
    return {
      note: {
        default: 0,
        parseHTML: (element: HTMLElement) => Number(element.getAttribute('data-note')),
      },
    };
  },
  parseHTML: () => [{ tag: 'sup[data-note]' }],
  renderHTML: ({ node, HTMLAttributes }) => [
    'sup',
    mergeAttributes(HTMLAttributes, { class: 'rely-note-reference', 'data-note': String(node.attrs.note) }),
    String(Number(node.attrs.note) + 1),
  ],
  renderText: ({ node }) => `[${Number(node.attrs.note) + 1}]`,
  addProseMirrorPlugins() {
    // Notes are numbered in the order the text refers to them, as Word numbers them.
    const renumber = (view: EditorView) => {
      const order = new Map<number, number>();
      view.state.doc.descendants((node, position) => {
        if (node.type.name !== this.name) return;
        const note = Number(node.attrs.note);
        if (!order.has(note)) order.set(note, order.size + 1);
        const dom = view.nodeDOM(position);
        const number = String(order.get(note));
        if (dom instanceof HTMLElement && dom.textContent !== number) dom.textContent = number;
      });
    };
    return [
      new Plugin({
        key: new PluginKey('relyNoteNumbers'),
        view: (view) => {
          renumber(view);
          return { update: renumber };
        },
      }),
    ];
  },
  addCommands() {
    return {
      insertNoteReference:
        (note) =>
        ({ commands }) =>
          commands.insertContent({ type: this.name, attrs: { note } }),
    };
  },
});

/** A comment on a point of the text rather than on a stretch of it. */
export const RelyCommentAnchor = Node.create({
  name: 'commentAnchor',
  group: 'inline',
  inline: true,
  atom: true,
  addAttributes() {
    return {
      comment: {
        default: 0,
        parseHTML: (element: HTMLElement) => Number(element.getAttribute('data-comment-anchor')),
      },
    };
  },
  parseHTML: () => [{ tag: 'span[data-comment-anchor]' }],
  renderHTML: ({ node, HTMLAttributes }) => [
    'span',
    mergeAttributes(HTMLAttributes, {
      class: 'rely-comment-anchor',
      'data-comment-anchor': String(node.attrs.comment),
    }),
  ],
  renderText: () => '',
});

/** An equation, kept as its LaTeX and shown as it. */
export const RelyMath = Node.create({
  name: 'math',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,
  addAttributes() {
    return {
      latex: { default: '', parseHTML: (element: HTMLElement) => element.getAttribute('data-latex') ?? '' },
      display: { default: false, parseHTML: (element: HTMLElement) => element.hasAttribute('data-display') },
    };
  },
  parseHTML: () => [{ tag: 'span[data-latex]' }],
  renderHTML: ({ node, HTMLAttributes }) => [
    'span',
    mergeAttributes(HTMLAttributes, {
      class: node.attrs.display ? 'rely-math rely-math-display' : 'rely-math',
      'data-latex': String(node.attrs.latex),
      ...(node.attrs.display ? { 'data-display': '' } : {}),
    }),
    String(node.attrs.latex),
  ],
  renderText: ({ node }) => `$${String(node.attrs.latex)}$`,
  addCommands() {
    return {
      insertMath:
        (latex, display = false) =>
        ({ commands }) =>
          commands.insertContent({ type: this.name, attrs: { latex, display } }),
    };
  },
});

/** Where a section of the document ends, and the next starts on a page of its own. */
export const RelySectionBreak = Node.create({
  name: 'sectionBreak',
  group: 'block',
  atom: true,
  selectable: true,
  addAttributes() {
    return {
      section: {
        default: 0,
        parseHTML: (element: HTMLElement) => Number(element.getAttribute('data-section')),
      },
    };
  },
  parseHTML: () => [{ tag: 'div[data-section]' }],
  renderHTML: ({ node, HTMLAttributes }) => [
    'div',
    mergeAttributes(HTMLAttributes, { class: 'rely-section-break', 'data-section': String(node.attrs.section) }),
  ],
});

/** What the document editor is made of. */
export interface RelyDocumentExtensionOptions {
  /** The extensions of the host, added after these. */
  extensions?: AnyExtension[];
  /** Who the changes recorded while tracking is on are by. */
  author?: () => string;
}

/**
 * Every extension the document tree needs, so a tree loads into the editor and comes back as it
 * went in. A host building its own editor UI uses these with `documentTreeToEditor`.
 */
export function createRelyDocumentExtensions(options: RelyDocumentExtensionOptions = {}): AnyExtension[] {
  return [
    StarterKit.configure({
      link: {
        openOnClick: false,
        autolink: true,
        protocols: ['http', 'https', 'mailto'],
        isAllowedUri: (url) => /^(https?:|mailto:)/i.test(url),
      },
    }),
    RelyAlignment,
    RelyListIdentity,
    RelyImage,
    TableKit.configure({ table: { resizable: true, cellMinWidth: 30 } }),
    RelySuperscript,
    RelySubscript,
    RelyTextStyle,
    RelyHighlight,
    RelyComment,
    RelyInsertion,
    RelyDeletion,
    RelyTrackedChanges,
    RelyReferences,
    RelyNoteReference,
    RelyCommentAnchor,
    RelyMath,
    RelySectionBreak,
    RelyParagraphFormat,
    RelySearch,
    RelyTrackRecording.configure({ author: options.author ?? (() => '') }),
    RelyPagination,
    ...(options.extensions ?? []),
  ];
}
