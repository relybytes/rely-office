// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type { JSONContent } from '@tiptap/core';
import type { Block, DocumentTree, Inline, Mark, MediaItem } from '../model.js';
import type {
  EditorDocument,
  EditorMappingReport,
  TreeToEditorOptions,
} from './editor-document.js';
import { mediaDataUrl } from './media.js';

type ListItemBlock = Extract<Block, { type: 'listItem' }>;

interface MappingContext {
  media: Map<number, MediaItem>;
  imageSource: (item: MediaItem) => string;
  report: EditorMappingReport;
}

/**
 * A document tree as the editor's content. Every block, mark and inline of the tree has a node or
 * a mark of its own, so a tree comes back from `editorToDocumentTree` as it went in; what the
 * editor does not show as text (notes, comments, page, headers, sections) rides along in `aside`.
 */
export function documentTreeToEditor(
  tree: DocumentTree,
  options: TreeToEditorOptions = {},
): { document: EditorDocument; report: EditorMappingReport } {
  const context: MappingContext = {
    media: new Map(tree.media.map((item) => [item.index, item])),
    imageSource: options.imageSource ?? mediaDataUrl,
    report: { marksDropped: 0, imagesLeftOut: 0 },
  };
  const breaks = new Map<number, number>();
  const sections = tree.sections ?? [];
  sections.slice(0, -1).forEach((section, index) => breaks.set(section.endBlock, index));

  const content = mapTopLevelBlocks(tree.blocks, breaks, context);
  return {
    document: {
      content: { type: 'doc', content: content.length > 0 ? content : [{ type: 'paragraph' }] },
      media: tree.media,
      aside: {
        report: tree.report,
        notes: tree.notes,
        headersFooters: tree.headersFooters,
        page: tree.page,
        ...(tree.sections === undefined ? {} : { sections: tree.sections }),
        ...(tree.comments === undefined ? {} : { comments: tree.comments }),
        ...(tree.objects === undefined ? {} : { objects: tree.objects }),
      },
    },
    report: context.report,
  };
}

/**
 * Blocks outside the body (a header, a footer, a note, a comment) as the content of an editor of
 * their own, their images shown from the document's media.
 */
export function blocksToEditorContent(
  blocks: Block[],
  media: MediaItem[],
  options: TreeToEditorOptions = {},
): JSONContent {
  const context: MappingContext = {
    media: new Map(media.map((item) => [item.index, item])),
    imageSource: options.imageSource ?? mediaDataUrl,
    report: { marksDropped: 0, imagesLeftOut: 0 },
  };
  return { type: 'doc', content: mapBlocks(blocks, context) };
}

/** The top level, with a section break where each section but the last ends. */
function mapTopLevelBlocks(
  blocks: Block[],
  breaks: Map<number, number>,
  context: MappingContext,
): JSONContent[] {
  const nodes: JSONContent[] = [];
  let index = 0;
  while (index < blocks.length) {
    const block = blocks[index];
    if (block.type === 'listItem') {
      const items: ListItemBlock[] = [];
      while (
        index < blocks.length &&
        blocks[index].type === 'listItem' &&
        (blocks[index] as ListItemBlock).listId === block.listId &&
        (items.length === 0 || !breaks.has(index))
      ) {
        items.push(blocks[index] as ListItemBlock);
        index += 1;
      }
      nodes.push(mapList(items, context));
    } else {
      nodes.push(...mapBlock(block, context));
      index += 1;
    }
    const section = breaks.get(index);
    if (section !== undefined) {
      nodes.push({ type: 'sectionBreak', attrs: { section } });
    }
  }
  return nodes;
}

/** Blocks nested in a quote or a table cell, where no section ends. */
function mapBlocks(blocks: Block[], context: MappingContext): JSONContent[] {
  const nodes = mapTopLevelBlocks(blocks, new Map(), context);
  return nodes.length > 0 ? nodes : [{ type: 'paragraph' }];
}

function mapBlock(block: Block, context: MappingContext): JSONContent[] {
  switch (block.type) {
    case 'paragraph':
      return [textBlock('paragraph', { align: block.align ?? null, format: block.format ?? null }, block.content, context)];
    case 'heading':
      return [
        textBlock(
          'heading',
          { level: block.level, align: block.align ?? null, format: block.format ?? null },
          block.content,
          context,
        ),
      ];
    case 'listItem':
      return [mapList([block], context)];
    case 'blockquote':
      return [{ type: 'blockquote', content: mapBlocks(block.blocks, context) }];
    case 'codeBlock':
      return [
        {
          type: 'codeBlock',
          attrs: { language: null },
          ...(block.text.length > 0 ? { content: [{ type: 'text', text: block.text }] } : {}),
        },
      ];
    case 'horizontalRule':
      return [{ type: 'horizontalRule' }];
    case 'table': {
      const widths = block.columnWidthsTwips ?? [];
      return [
        {
          type: 'table',
          content: block.rows.map((row) => {
            let column = 0;
            return {
              type: 'tableRow',
              content: row.cells.map((cell) => {
                const span = widths.slice(column, column + cell.colSpan);
                column += cell.colSpan;
                const colwidth = span.length === cell.colSpan ? span.map(twipsToPixels) : null;
                return {
                  type: row.header ? 'tableHeader' : 'tableCell',
                  attrs: { colspan: cell.colSpan, rowspan: cell.rowSpan, colwidth },
                  content: mapBlocks(cell.blocks, context),
                };
              }),
            };
          }),
        },
      ];
    }
  }
}

/**
 * Twentieths of a point as the CSS pixels a table column is drawn with, 96 to the inch; kept to
 * a hundredth of a pixel so a width the person did not drag comes back to the twip.
 */
export function twipsToPixels(twips: number): number {
  return Math.max(1, Math.round((twips / 15) * 100) / 100);
}

function textBlock(
  type: string,
  attrs: Record<string, unknown>,
  inlines: Inline[],
  context: MappingContext,
): JSONContent {
  const content = mapInlines(inlines, context);
  return content.length > 0 ? { type, attrs, content } : { type, attrs };
}

/**
 * Flat list items, one list of the tree, as nested lists: an item one level deeper than the one
 * before goes into a list inside that item; a level with no item above it to nest in is taken
 * as the deepest one that has.
 */
function mapList(items: ListItemBlock[], context: MappingContext): JSONContent {
  const listNode = (ordered: boolean, listId: number | null): JSONContent => ({
    type: ordered ? 'orderedList' : 'bulletList',
    attrs: ordered ? { start: 1, listId } : { listId },
    content: [],
  });
  const root = listNode(items[0].ordered, items[0].listId);
  const stack: JSONContent[] = [root];
  for (const item of items) {
    const top = stack[stack.length - 1];
    const deepest = top.content!.length === 0 ? stack.length - 1 : stack.length;
    const level = Math.max(0, Math.min(item.level, deepest));
    while (stack.length > level + 1) stack.pop();
    if (level === stack.length) {
      const parent = stack[stack.length - 1];
      const lastItem = parent.content![parent.content!.length - 1];
      const nested = listNode(item.ordered, null);
      lastItem.content!.push(nested);
      stack.push(nested);
    }
    stack[stack.length - 1].content!.push({
      type: 'listItem',
      content: [textBlock('paragraph', { align: null, format: item.format ?? null }, item.content, context)],
    });
  }
  return root;
}

function mapInlines(inlines: Inline[], context: MappingContext): JSONContent[] {
  const nodes: JSONContent[] = [];
  for (const inline of inlines) {
    switch (inline.type) {
      case 'text': {
        if (inline.text.length === 0) break;
        const marks = mapMarks(inline.marks ?? [], context);
        nodes.push(marks.length > 0 ? { type: 'text', text: inline.text, marks } : { type: 'text', text: inline.text });
        break;
      }
      case 'hardBreak':
        nodes.push({ type: 'hardBreak' });
        break;
      case 'image': {
        const item = context.media.get(inline.media);
        if (item === undefined) {
          context.report.imagesLeftOut += 1;
          break;
        }
        nodes.push({
          type: 'image',
          attrs: {
            src: context.imageSource(item),
            alt: inline.alt ?? null,
            title: null,
            width: inline.width ?? null,
            height: inline.height ?? null,
            anchor: inline.anchor ?? null,
            media: inline.media,
          },
        });
        break;
      }
      case 'noteReference':
        nodes.push({ type: 'noteReference', attrs: { note: inline.note } });
        break;
      case 'commentAnchor':
        nodes.push({ type: 'commentAnchor', attrs: { comment: inline.comment } });
        break;
      case 'math':
        nodes.push({ type: 'math', attrs: { latex: inline.latex, display: inline.display } });
        break;
    }
  }
  return nodes;
}

function mapMarks(marks: Mark[], context: MappingContext): NonNullable<JSONContent['marks']> {
  const mapped = marks.map((mark): { type: string; attrs?: Record<string, unknown> } => {
    switch (mark.type) {
      case 'textStyle':
        return {
          type: 'textStyle',
          attrs: {
            fontFamily: mark.fontFamily ?? null,
            fontSize: mark.fontSize ?? null,
            color: mark.color ?? null,
          },
        };
      case 'highlight':
        return { type: 'highlight', attrs: { color: mark.color } };
      case 'link':
        return { type: 'link', attrs: { href: mark.href } };
      case 'comment':
        return { type: 'comment', attrs: { comment: mark.comment } };
      case 'insertion':
      case 'deletion':
        return { type: mark.type, attrs: { author: mark.author, date: mark.date ?? null } };
      case 'formatChange':
        return {
          type: 'formatChange',
          attrs: { author: mark.author, date: mark.date ?? null, previous: mapMarks(mark.previous, context) },
        };
      default:
        return { type: mark.type };
    }
  });
  if (!mapped.some((mark) => mark.type === 'code')) return mapped;
  // The editor's code mark excludes every other one, a link included, as Word's does not.
  const kept = mapped.filter((mark) => mark.type === 'code');
  context.report.marksDropped += mapped.length - kept.length;
  return kept;
}
