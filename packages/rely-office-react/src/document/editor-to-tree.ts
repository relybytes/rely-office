// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type { JSONContent } from '@tiptap/core';
import type {
  Alignment,
  Block,
  DocumentTree,
  HeadersFooters,
  ImageAnchor,
  Inline,
  Mark,
  MediaItem,
  Section,
  TableRow,
} from '../model.js';
import type { EditorDocument } from './editor-document.js';
import { cleanFormat } from './formatting.js';
import { mapBlockImages } from './media.js';
import { compactComments, compactNotes } from './references.js';

const ALIGNMENTS: ReadonlySet<string> = new Set(['left', 'center', 'right', 'justify']);
const SAFE_LINK = /^(https?:|mailto:)/i;
/** The order the tree lists the marks of a run in, whatever order the editor holds them in. */
const MARK_ORDER = [
  'bold',
  'italic',
  'underline',
  'strike',
  'code',
  'superscript',
  'subscript',
  'textStyle',
  'highlight',
  'link',
  'comment',
  'insertion',
  'deletion',
  'formatChange',
];
const SIMPLE_MARKS: ReadonlySet<string> = new Set([
  'bold',
  'italic',
  'underline',
  'strike',
  'code',
  'superscript',
  'subscript',
]);

interface WalkContext {
  /** Blocks written at the top level so far: where a section break falls. */
  sectionEnds: { section: number; endBlock: number }[];
  nextListId: number;
  usedListIds: Set<number>;
}

/**
 * The editor's content back as a document tree `writeDocx` writes: the blocks from the content,
 * the rest from `aside`, and only the images something still shows, numbered again from 0.
 */
export function editorToDocumentTree(document: EditorDocument): DocumentTree {
  const context: WalkContext = {
    sectionEnds: [],
    nextListId: firstFreeListId(document.content),
    usedListIds: new Set(),
  };
  const blocks: Block[] = [];
  for (const node of document.content.content ?? []) {
    if (node.type === 'sectionBreak') {
      context.sectionEnds.push({ section: Number(node.attrs?.section ?? -1), endBlock: blocks.length });
      continue;
    }
    blocks.push(...mapNode(node, context));
  }

  const { aside } = document;
  const sections = buildSections(aside.sections, context.sectionEnds, blocks.length, aside.headersFooters);
  const tree: DocumentTree = {
    blocks,
    media: document.media,
    report: aside.report,
    notes: aside.notes,
    headersFooters: aside.headersFooters,
    page: aside.page,
    ...(sections === undefined ? {} : { sections }),
    ...(aside.comments === undefined ? {} : { comments: aside.comments }),
    ...(aside.objects === undefined ? {} : { objects: aside.objects }),
    ...(aside.macroProject === undefined ? {} : { macroProject: aside.macroProject }),
  };
  return compactMedia(compactComments(compactNotes(tree)));
}

/** The content of an editor of blocks outside the body (a header, a note, a comment) as blocks. */
export function editorContentToBlocks(content: JSONContent): Block[] {
  const context: WalkContext = { sectionEnds: [], nextListId: firstFreeListId(content), usedListIds: new Set() };
  return mapNodes(content.content, context);
}

/** The sections whose break is still in the text, each ending where its break now is. */
function buildSections(
  original: Section[] | undefined,
  ends: { section: number; endBlock: number }[],
  total: number,
  lastHeadersFooters: HeadersFooters,
): Section[] | undefined {
  if (original === undefined || original.length === 0) return undefined;
  const kept: Section[] = [];
  for (const end of ends) {
    const section = original[end.section];
    if (section !== undefined && end.endBlock > (kept[kept.length - 1]?.endBlock ?? 0)) {
      kept.push({ ...section, endBlock: end.endBlock });
    }
  }
  // The last section's headers are the document's, which the editor changes.
  kept.push({ ...original[original.length - 1], endBlock: total, headersFooters: lastHeadersFooters });
  return kept.length > 1 ? kept : undefined;
}

function firstFreeListId(node: JSONContent): number {
  let highest = 0;
  const visit = (current: JSONContent): void => {
    const listId = current.attrs?.listId;
    if (typeof listId === 'number' && listId > highest) highest = listId;
    current.content?.forEach(visit);
  };
  visit(node);
  return highest + 1;
}

function mapNodes(nodes: JSONContent[] | undefined, context: WalkContext): Block[] {
  return (nodes ?? []).flatMap((node) => mapNode(node, context));
}

function mapNode(node: JSONContent, context: WalkContext): Block[] {
  switch (node.type) {
    case 'paragraph':
      return [withAlign({ type: 'paragraph', content: mapInlines(node.content) }, node)];
    case 'heading': {
      const level = Math.min(Math.max(Math.round(Number(node.attrs?.level ?? 1)), 1), 6);
      return [
        withAlign(
          { type: 'heading', level: level as 1 | 2 | 3 | 4 | 5 | 6, content: mapInlines(node.content) },
          node,
        ),
      ];
    }
    case 'bulletList':
    case 'orderedList':
      return flattenList(node, 0, allocateListId(node, context), []);
    case 'blockquote':
      return [{ type: 'blockquote', blocks: mapNodes(node.content, context) }];
    case 'codeBlock':
      return [{ type: 'codeBlock', text: plainText(node) }];
    case 'horizontalRule':
      return [{ type: 'horizontalRule' }];
    case 'table': {
      const columnWidthsTwips = tableWidths(node);
      return [
        {
          type: 'table',
          rows: (node.content ?? []).map((row) => mapRow(row, context)),
          ...(columnWidthsTwips === null ? {} : { columnWidthsTwips }),
        },
      ];
    }
    case 'image':
      return [{ type: 'paragraph', content: mapInlines([node]) }];
    case 'sectionBreak':
      return [];
    default: {
      // A node of the host's own: its text blocks, or its text in a paragraph.
      const children = node.content ?? [];
      if (children.some((child) => child.type !== 'text' && child.type !== 'hardBreak')) {
        return mapNodes(children, context);
      }
      return children.length === 0 ? [] : [{ type: 'paragraph', content: mapInlines(children) }];
    }
  }
}

type AlignedBlock = Extract<Block, { type: 'paragraph' | 'heading' }>;

function withAlign(block: AlignedBlock, node: JSONContent): AlignedBlock {
  const align = node.attrs?.align;
  const format = cleanFormat(node.attrs?.format as Record<string, unknown> | null | undefined);
  return {
    ...block,
    ...(typeof align === 'string' && ALIGNMENTS.has(align) ? { align: align as Alignment } : {}),
    ...(format === null ? {} : { format }),
  };
}

/** The widths of a table's columns in twips, from its first row, when every cell has one. */
function tableWidths(table: JSONContent): number[] | null {
  const cells = table.content?.[0]?.content ?? [];
  const widths: number[] = [];
  for (const cell of cells) {
    const colwidth = cell.attrs?.colwidth as unknown;
    const span = Math.max(1, Math.round(Number(cell.attrs?.colspan ?? 1)));
    if (!Array.isArray(colwidth) || colwidth.length !== span) return null;
    for (const pixels of colwidth) {
      const value = Number(pixels);
      if (!Number.isFinite(value) || value <= 0) return null;
      widths.push(Math.round(value * 15));
    }
  }
  return widths.length > 0 ? widths : null;
}

/** A list keeps the `listId` it came with, unless an earlier list of the text already took it. */
function allocateListId(node: JSONContent, context: WalkContext): number {
  const listId = node.attrs?.listId;
  if (typeof listId === 'number' && !context.usedListIds.has(listId)) {
    context.usedListIds.add(listId);
    return listId;
  }
  const allocated = context.nextListId;
  context.nextListId += 1;
  context.usedListIds.add(allocated);
  return allocated;
}

/**
 * Nested lists as the flat items of one list. An item's first paragraph is the item; the
 * paragraphs after it join it on a new line, since a list item of the tree holds one paragraph.
 */
function flattenList(list: JSONContent, level: number, listId: number, out: Block[]): Block[] {
  const ordered = list.type === 'orderedList';
  for (const item of list.content ?? []) {
    const content: Inline[] = [];
    const nested: JSONContent[] = [];
    let started = false;
    let format: ReturnType<typeof cleanFormat> = null;
    for (const child of item.content ?? []) {
      if (child.type === 'bulletList' || child.type === 'orderedList') {
        nested.push(child);
        continue;
      }
      if (started) content.push({ type: 'hardBreak' });
      else format = cleanFormat(child.attrs?.format as Record<string, unknown> | null | undefined);
      content.push(...mapInlines(child.type === 'paragraph' || child.type === 'heading' ? child.content : [{ type: 'text', text: plainText(child) }]));
      started = true;
    }
    out.push({ type: 'listItem', ordered, level, listId, content, ...(format === null ? {} : { format }) });
    for (const child of nested) flattenList(child, level + 1, listId, out);
  }
  return out;
}

function mapRow(row: JSONContent, context: WalkContext): TableRow {
  const cells = row.content ?? [];
  return {
    header: cells.length > 0 && cells.every((cell) => cell.type === 'tableHeader'),
    cells: cells.map((cell) => ({
      colSpan: Math.max(1, Math.round(Number(cell.attrs?.colspan ?? 1))),
      rowSpan: Math.max(1, Math.round(Number(cell.attrs?.rowspan ?? 1))),
      blocks: mapNodes(cell.content, context),
    })),
  };
}

function plainText(node: JSONContent): string {
  if (node.type === 'text') return node.text ?? '';
  if (node.type === 'hardBreak') return '\n';
  return (node.content ?? []).map(plainText).join('');
}

function mapInlines(nodes: JSONContent[] | undefined): Inline[] {
  const inlines: Inline[] = [];
  for (const node of nodes ?? []) {
    const attrs = node.attrs ?? {};
    switch (node.type) {
      case 'text': {
        if (!node.text) break;
        const marks = mapMarks(node.marks);
        inlines.push(marks.length > 0 ? { type: 'text', text: node.text, marks } : { type: 'text', text: node.text });
        break;
      }
      case 'hardBreak':
        inlines.push({ type: 'hardBreak' });
        break;
      case 'image': {
        if (typeof attrs.media !== 'number') break;
        const size = (value: unknown): number | undefined => {
          const number = Number(value);
          return value !== null && Number.isFinite(number) && number > 0 ? Math.round(number) : undefined;
        };
        const width = size(attrs.width);
        const height = size(attrs.height);
        const anchor = floatingAnchor(attrs.anchor, width, height);
        inlines.push({
          type: 'image',
          media: attrs.media,
          ...(typeof attrs.alt === 'string' && attrs.alt.length > 0 ? { alt: attrs.alt } : {}),
          ...(width === undefined ? {} : { width }),
          ...(height === undefined ? {} : { height }),
          ...(anchor === null ? {} : { anchor }),
        });
        break;
      }
      case 'noteReference':
        if (typeof attrs.note === 'number') inlines.push({ type: 'noteReference', note: attrs.note });
        break;
      case 'commentAnchor':
        if (typeof attrs.comment === 'number') inlines.push({ type: 'commentAnchor', comment: attrs.comment });
        break;
      case 'math':
        inlines.push({ type: 'math', latex: String(attrs.latex ?? ''), display: attrs.display === true });
        break;
      default:
        if (node.content !== undefined) inlines.push(...mapInlines(node.content));
        else if (node.text) inlines.push({ type: 'text', text: node.text });
    }
  }
  return inlines;
}

function mapMarks(marks: JSONContent['marks']): Mark[] {
  const mapped: Mark[] = [];
  for (const mark of marks ?? []) {
    const attrs = mark.attrs ?? {};
    const text = (value: unknown): string | undefined =>
      typeof value === 'string' && value.length > 0 ? value : undefined;
    if (SIMPLE_MARKS.has(mark.type)) {
      mapped.push({ type: mark.type } as Mark);
    } else if (mark.type === 'link') {
      const href = text(attrs.href);
      if (href !== undefined && SAFE_LINK.test(href)) mapped.push({ type: 'link', href });
    } else if (mark.type === 'textStyle') {
      const style = {
        ...(text(attrs.fontFamily) === undefined ? {} : { fontFamily: text(attrs.fontFamily) }),
        ...(text(attrs.fontSize) === undefined ? {} : { fontSize: text(attrs.fontSize) }),
        ...(text(attrs.color) === undefined ? {} : { color: text(attrs.color) }),
      };
      if (Object.keys(style).length > 0) mapped.push({ type: 'textStyle', ...style });
    } else if (mark.type === 'highlight') {
      const color = text(attrs.color);
      if (color !== undefined) mapped.push({ type: 'highlight', color });
    } else if (mark.type === 'comment') {
      if (typeof attrs.comment === 'number') mapped.push({ type: 'comment', comment: attrs.comment });
    } else if (mark.type === 'insertion' || mark.type === 'deletion') {
      const date = text(attrs.date);
      mapped.push({
        type: mark.type,
        author: String(attrs.author ?? ''),
        ...(date === undefined ? {} : { date }),
      });
    } else if (mark.type === 'formatChange') {
      const date = text(attrs.date);
      mapped.push({
        type: 'formatChange',
        author: String(attrs.author ?? ''),
        ...(date === undefined ? {} : { date }),
        previous: mapMarks(Array.isArray(attrs.previous) ? (attrs.previous as JSONContent['marks']) : []),
      });
    }
  }
  return mapped.sort((a, b) => MARK_ORDER.indexOf(a.type) - MARK_ORDER.indexOf(b.type));
}

/** Keep the images something still shows, numbered again from 0 in the order they were added. */
function compactMedia(tree: DocumentTree): DocumentTree {
  const used = new Set<number>();
  const collect = (blocks: Block[]): void => {
    mapBlockImages(blocks, (image) => {
      used.add(image.media);
      return image;
    });
  };
  forEachBlockList(tree, collect);
  tree.objects?.forEach((object) => used.add(object.media));

  const byIndex = new Map(tree.media.map((item) => [item.index, item]));
  const renumbered = new Map<number, number>();
  const media: MediaItem[] = [];
  for (const index of [...used].sort((a, b) => a - b)) {
    const item = byIndex.get(index);
    if (item === undefined) continue;
    renumbered.set(index, media.length);
    media.push({ ...item, index: media.length });
  }
  const remap = (blocks: Block[]): Block[] =>
    mapBlockImages(blocks, (image) => {
      const index = renumbered.get(image.media);
      return index === undefined ? null : { ...image, media: index };
    });
  const remapHeaders = (headers: HeadersFooters): HeadersFooters =>
    Object.fromEntries(
      Object.entries(headers).map(([key, blocks]) => [key, remap(blocks as Block[])]),
    ) as unknown as HeadersFooters;

  return {
    ...tree,
    blocks: remap(tree.blocks),
    media,
    notes: tree.notes.map((note) => ({ ...note, blocks: remap(note.blocks) })),
    headersFooters: remapHeaders(tree.headersFooters),
    ...(tree.sections === undefined
      ? {}
      : { sections: tree.sections.map((section) => ({ ...section, headersFooters: remapHeaders(section.headersFooters) })) }),
    ...(tree.comments === undefined
      ? {}
      : { comments: tree.comments.map((comment) => ({ ...comment, blocks: remap(comment.blocks) })) }),
    ...(tree.objects === undefined
      ? {}
      : {
          objects: tree.objects
            .filter((object) => renumbered.has(object.media))
            .map((object) => ({ ...object, media: renumbered.get(object.media)! })),
        }),
  };
}

function forEachBlockList(tree: DocumentTree, visit: (blocks: Block[]) => void): void {
  visit(tree.blocks);
  tree.notes.forEach((note) => visit(note.blocks));
  const headers = (value: HeadersFooters): void =>
    Object.values(value).forEach((blocks) => visit((blocks as Block[] | undefined) ?? []));
  headers(tree.headersFooters);
  tree.sections?.forEach((section) => headers(section.headersFooters));
  tree.comments?.forEach((comment) => visit(comment.blocks));
}

/** The EMU of a CSS pixel. */
const EMU_PER_PIXEL = 9_525;

/** The anchor of a floating image as the editor left it, its size the one the image was given. */
function floatingAnchor(value: unknown, width: number | undefined, height: number | undefined): ImageAnchor | null {
  if (value === null || typeof value !== 'object') return null;
  const anchor = value as ImageAnchor;
  if (typeof anchor.wrap?.type !== 'string' || typeof anchor.horizontal?.base !== 'string') return null;
  return {
    ...anchor,
    ...(width === undefined ? {} : { widthEmu: width * EMU_PER_PIXEL }),
    ...(height === undefined ? {} : { heightEmu: height * EMU_PER_PIXEL }),
  };
}
