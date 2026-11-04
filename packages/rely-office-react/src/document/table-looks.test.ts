// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-RelyBytes-Attribution

import { Editor } from '@tiptap/core';
import type { Block, DocumentTree, TableBorders } from '../model.js';
import { editorToDocumentTree } from './editor-to-tree.js';
import { createRelyDocumentExtensions } from './extensions.js';
import { formatHtml } from './formatting.js';
import { borderCss, cellLooks, cleanBorderLine, cleanCellMargins, cleanColor, cleanTableBorders } from './table-looks.js';
import { documentTreeToEditor } from './tree-to-editor.js';

const RED = { style: 'single', widthEighths: 12, color: '#C00000' } as const;
const TABLE_BORDERS: TableBorders = {
  top: RED,
  bottom: RED,
  left: { style: 'none' },
  right: { style: 'none' },
  insideH: { style: 'single', widthEighths: 4, color: '#808080' },
  insideV: { style: 'none' },
};

/** A contract's table as `readDocumentTree` gives it, and a bordered, shaded paragraph. */
const BLOCKS: Block[] = [
  {
    type: 'paragraph',
    content: [{ type: 'text', text: 'Premessa' }],
    format: { borders: { top: RED, bottom: { style: 'double', widthEighths: 6 } }, shading: '#FFF2CC' },
  },
  {
    type: 'table',
    columnWidthsTwips: [3000, 6000],
    borders: TABLE_BORDERS,
    cellMarginsTwips: { top: 57, left: 113, bottom: 57, right: 113 },
    rows: [
      {
        header: true,
        cells: [
          { colSpan: 1, rowSpan: 1, fill: '#1F3864', verticalAlign: 'center', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Parte' }] }] },
          { colSpan: 1, rowSpan: 1, fill: '#1F3864', verticalAlign: 'center', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Dati' }] }] },
        ],
      },
      {
        header: false,
        cells: [
          {
            colSpan: 1,
            rowSpan: 1,
            borders: { bottom: { style: 'dashed', widthEighths: 12, color: '#00B050' } },
            verticalAlign: 'bottom',
            blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Fornitore' }] }],
          },
          { colSpan: 1, rowSpan: 1, fill: '#F2F2F2', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: '{{controparte.nome}}' }] }] },
        ],
      },
    ],
  },
];

function tree(blocks: Block[]): DocumentTree {
  return {
    blocks,
    media: [],
    report: {
      truncated: false, textBoxes: 0, footnotes: 0, endnotes: 0, comments: 0, headersAndFooters: 0,
      fieldsKeptAsText: 0, charts: 0, smartArt: 0, equations: 0, shapes: 0, embeddedObjects: 0,
      unsupportedLinks: 0, oversizedImages: 0, imagesOverLimit: 0, missingImages: 0,
      trackedInsertions: 0, trackedDeletions: 0, hiddenRuns: 0,
    },
    notes: [],
    headersFooters: { header: [], footer: [] },
    page: { widthTwips: 11906, heightTwips: 16838, marginsTwips: [1440, 1440, 1440, 1440] },
  };
}

function openEditor(blocks: Block[] = BLOCKS, element?: HTMLElement): { editor: Editor; back: () => Block[] } {
  const { document } = documentTreeToEditor(tree(blocks));
  const editor = new Editor({ extensions: createRelyDocumentExtensions(), content: document.content, element });
  return { editor, back: () => editorToDocumentTree({ ...document, content: editor.getJSON() }).blocks };
}

/** The position inside the first paragraph whose text is `text`. */
function positionOf(editor: Editor, text: string): number {
  let found = -1;
  editor.state.doc.descendants((node, position) => {
    if (found < 0 && node.isText && node.text === text) found = position;
  });
  return found;
}

describe('the looks of tables and paragraphs in the editor', () => {
  it('come back as they went in when nothing is touched', () => {
    const { editor, back } = openEditor();
    try {
      expect(back()).toEqual(BLOCKS);
    } finally {
      editor.destroy();
    }
  });

  it('stay on a cell whose text is edited', () => {
    const { editor, back } = openEditor();
    try {
      editor.chain().setTextSelection(positionOf(editor, '{{controparte.nome}}')).insertContent('Spett. ').run();
      editor.chain().setTextSelection(positionOf(editor, 'Fornitore') + 9).insertContent(' e cliente').run();

      const table = back()[1];
      if (table.type !== 'table') throw new Error('not a table');
      expect(table.rows[1].cells[0]).toEqual({
        ...((BLOCKS[1] as Extract<Block, { type: 'table' }>).rows[1].cells[0]),
        blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Fornitore e cliente' }] }],
      });
      expect(table.rows[1].cells[1]).toMatchObject({
        fill: '#F2F2F2',
        blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Spett. {{controparte.nome}}' }] }],
      });
      expect(table.borders).toEqual(TABLE_BORDERS);
    } finally {
      editor.destroy();
    }
  });

  it('give a row added in the editor no looks of its own, the table keeping its lines', () => {
    const { editor, back } = openEditor();
    try {
      editor.chain().setTextSelection(positionOf(editor, 'Fornitore')).addRowAfter().run();

      const table = back()[1];
      if (table.type !== 'table') throw new Error('not a table');
      expect(table.rows).toHaveLength(3);
      expect(table.rows[2].cells.map((cell) => Object.keys(cell).sort())).toEqual([
        ['blocks', 'colSpan', 'rowSpan'],
        ['blocks', 'colSpan', 'rowSpan'],
      ]);
      expect(table.borders).toEqual(TABLE_BORDERS);
      expect(table.cellMarginsTwips).toEqual({ top: 57, left: 113, bottom: 57, right: 113 });
    } finally {
      editor.destroy();
    }
  });

  it('give a table made in the editor none, so it is written with the grid', () => {
    const { editor, back } = openEditor([{ type: 'paragraph', content: [{ type: 'text', text: 'x' }] }]);
    try {
      editor.chain().setTextSelection(1).insertTable({ rows: 2, cols: 2, withHeaderRow: false }).run();

      const table = back().find((block) => block.type === 'table');
      expect(table).toBeDefined();
      expect(Object.keys(table!).sort()).toEqual(['rows', 'type']);
      expect(table!.type === 'table' && table!.rows[0].cells[0]).toEqual({
        colSpan: 1,
        rowSpan: 1,
        blocks: [{ type: 'paragraph', content: [] }],
      });
    } finally {
      editor.destroy();
    }
  });

  it('are drawn: the lines and cell margins on the table, fills, edges and alignment on the cells', () => {
    const element = document.createElement('div');
    document.body.appendChild(element);
    const { editor } = openEditor(BLOCKS, element);
    try {
      const table = element.querySelector('table')!;
      expect(table.classList.contains('rely-table-lines')).toBe(true);
      expect(table.style.getPropertyValue('--rely-table-top')).toBe('1.5pt solid #C00000');
      expect(table.style.getPropertyValue('--rely-table-inside-v')).toBe('none');
      expect(table.style.getPropertyValue('--rely-cell-padding')).toBe('2.85pt 5.65pt 2.85pt 5.65pt');
      const cells = element.querySelectorAll('th, td');
      expect(cells[0].getAttribute('data-fill')).toBe('#1F3864');
      expect(cells[0].getAttribute('style')).toContain('vertical-align: middle');
      // The DOM may give a colour back as rgb(): the line and its width are what is checked.
      expect(cells[2].getAttribute('style')).toContain('border-bottom: 1.5pt dashed');
      expect(cells[2].getAttribute('data-borders')).toContain('#00B050');
      const paragraph = element.querySelector('p')!;
      expect(paragraph.getAttribute('style')).toMatch(/background-color: (#FFF2CC|rgb\(255, 242, 204\))/);
      expect(paragraph.getAttribute('style')).toContain('border-bottom: 2.25pt double');

      editor.commands.command(({ tr }) => {
        tr.setNodeAttribute(positionOf(editor, 'Premessa') + 'Premessa'.length + 1, 'borders', null);
        return true;
      });
      expect(table.classList.contains('rely-table-lines')).toBe(false);
      expect(table.style.getPropertyValue('--rely-table-top')).toBe('');
    } finally {
      editor.destroy();
      element.remove();
    }
  });

  it('come back from the editor HTML, as a copy and paste carries them', () => {
    const { editor } = openEditor();
    const html = editor.getHTML();
    editor.destroy();
    const pasted = new Editor({ extensions: createRelyDocumentExtensions(), content: html });
    try {
      const { document } = documentTreeToEditor(tree([]));
      const blocks = editorToDocumentTree({ ...document, content: pasted.getJSON() }).blocks;
      expect(blocks[0]).toMatchObject({ format: BLOCKS[0].type === 'paragraph' ? BLOCKS[0].format : {} });
      expect(blocks[1]).toMatchObject({
        borders: TABLE_BORDERS,
        cellMarginsTwips: { top: 57, left: 113, bottom: 57, right: 113 },
      });
      const table = blocks[1];
      expect(table.type === 'table' && table.rows.map((row) => row.cells.map(cellLooksOf))).toEqual([
        [{ fill: '#1F3864', verticalAlign: 'center' }, { fill: '#1F3864', verticalAlign: 'center' }],
        [{ borders: { bottom: { style: 'dashed', widthEighths: 12, color: '#00B050' } }, verticalAlign: 'bottom' }, { fill: '#F2F2F2' }],
      ]);
    } finally {
      pasted.destroy();
    }
  });
});

function cellLooksOf(cell: { fill?: string; borders?: unknown; verticalAlign?: string }): Record<string, unknown> {
  return cellLooks(cell as Record<string, unknown>);
}

describe('what the editor gives the tree', () => {
  it('keeps the colours, lines and margins it can write, and nothing else', () => {
    expect(cleanColor(' c00000 ')).toBe('#C00000');
    expect(cleanColor('#abc')).toBeNull();
    expect(cleanColor('red')).toBeNull();
    expect(cleanBorderLine({ style: 'wavy' })).toBeNull();
    expect(cleanBorderLine('single')).toBeNull();
    expect(cleanBorderLine({ style: 'dotted', widthEighths: 3.4, color: 'url(x)' })).toEqual({ style: 'dotted', widthEighths: 3 });
    expect(cleanTableBorders({ top: { style: 'nope' }, insideV: { style: 'none' }, other: RED })).toEqual({ insideV: { style: 'none' } });
    expect(cleanTableBorders({})).toBeNull();
    expect(cleanCellMargins({ top: -5, left: 99999, bottom: '7', right: 120.6 })).toEqual({ top: 0, left: 31680, right: 121 });
    expect(cleanCellMargins(null)).toBeNull();
    expect(cellLooks({ fill: 'nope', verticalAlign: 'middle', borders: { left: RED } })).toEqual({ borders: { left: RED } });
  });

  it('draws a line as CSS, a double rule wide enough to show', () => {
    expect(borderCss({ style: 'single' })).toBe('0.5pt solid currentColor');
    expect(borderCss({ style: 'double', widthEighths: 4, color: '#2F5496' })).toBe('1.5pt double #2F5496');
    expect(borderCss({ style: 'dashed', widthEighths: 999 })).toBe('12pt dashed currentColor');
    expect(borderCss({ style: 'none' })).toBe('none');
    expect(borderCss(undefined)).toBeNull();
    expect(formatHtml({ shading: 'javascript:x', borders: { left: { style: 'dotted', widthEighths: 2 } } })).toMatchObject({
      style: 'border-left: 0.25pt dotted currentColor',
    });
  });
});
