// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// A page with the three editors over documents made in the page itself, and two editors sharing
// one document as two people would: `npm run demo`.

import { StrictMode, useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Awareness, applyAwarenessUpdate, encodeAwarenessUpdate } from 'y-protocols/awareness';
import * as Y from 'yjs';
import { documentCollaborationExtensions } from '../src/collab/document-extensions.js';
import type { CollabSession } from '../src/collab/session.js';
import {
  DocumentEditor,
  ITALIAN_DOCUMENT_EDITOR_LABELS,
  ITALIAN_SHEET_GRID_LABELS,
  ITALIAN_SLIDE_EDITOR_LABELS,
  SheetGrid,
  SlideEditor,
  createSheet,
  initDocuments,
  newGeometryShape,
  newTextShape,
  openDocument,
  readPresentation,
  writeDocx,
  withRunStyle,
  writePptx,
  type EditorDocument,
  type Presentation,
  type SheetDocument,
  type SlideShape,
  type TextBody,
  type DocumentEditorHandle,
  type TableCell,
} from '../src/index.js';
import '../src/styles.css';

type View = 'document' | 'sheet' | 'slides' | 'together';

const cell = (text: string): TableCell => ({
  colSpan: 1,
  rowSpan: 1,
  blocks: [{ type: 'paragraph', content: [{ type: 'text', text }] }],
});

function sampleDocument(): EditorDocument {
  const docx = writeDocx({
    blocks: [
      { type: 'heading', level: 1, content: [{ type: 'text', text: 'Verbale della riunione' }] },
      {
        type: 'paragraph',
        content: [
          { type: 'text', text: 'Presenti: ' },
          { type: 'text', text: 'Anna Bianchi', marks: [{ type: 'bold' }, { type: 'comment', comment: 0 }] },
          { type: 'text', text: ', Marco Rossi e ' },
          { type: 'text', text: 'Giulia Verdi', marks: [{ type: 'textStyle', color: '#C00000' }] },
          { type: 'text', text: '.' },
        ],
      },
      {
        type: 'heading',
        level: 2,
        content: [{ type: 'text', text: 'Ordine del giorno' }, { type: 'noteReference', note: 0 }],
      },
      { type: 'listItem', ordered: true, level: 0, listId: 1, content: [{ type: 'text', text: 'Budget del trimestre' }] },
      { type: 'listItem', ordered: true, level: 1, listId: 1, content: [{ type: 'text', text: 'Costi del personale' }] },
      { type: 'listItem', ordered: true, level: 0, listId: 1, content: [{ type: 'text', text: 'Nuovi clienti' }] },
      {
        type: 'table',
        rows: [
          { header: true, cells: [cell('Voce'), cell('Importo')] },
          { header: false, cells: [cell('Consulenza'), cell('12.000 euro')] },
        ],
      },
      {
        type: 'paragraph',
        content: [
          { type: 'text', text: 'Margine: ' },
          { type: 'math', latex: '\\frac{ricavi - costi}{ricavi}', display: false },
        ],
      },
      {
        type: 'blockquote',
        blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'La prossima riunione si terrà lunedì.' }] }],
      },
    ],
    notes: [{ kind: 'footnote', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Approvato il 10 ottobre.' }] }] }],
    headersFooters: {
      header: [{ type: 'paragraph', content: [{ type: 'text', text: 'Rely Platform - riservato' }] }],
      footer: [],
    },
    comments: [
      {
        author: 'Marco Rossi',
        date: '2026-10-28T09:00:00Z',
        blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'Confermare la presenza.' }] }],
        resolved: false,
      },
    ],
  });
  return openDocument(docx).document;
}

function samplePresentation(): Presentation {
  const title = newTextShape({ x: 900_000, y: 700_000, width: 10_000_000, height: 1_100_000 }, 'Risultati 2026');
  const points = newTextShape(
    { x: 900_000, y: 2_100_000, width: 6_200_000, height: 2_600_000 },
    'Ricavi in crescita del 18%\nTre nuovi clienti\nMargine stabile al 38%',
  );
  const badge = newTextShape({ x: 7_900_000, y: 2_300_000, width: 3_000_000, height: 1_300_000 }, '+18%');
  return readPresentation(
    writePptx({
      widthEmu: 12_192_000,
      heightEmu: 6_858_000,
      slides: [
        {
          background: {
            type: 'gradient',
            gradient: {
              stops: [
                { offset: 0, colour: 'DBEAFE' },
                { offset: 1, colour: 'FFFFFF' },
              ],
              kind: { type: 'linear', angle: 90 },
            },
          },
          shapes: [
            { ...title, content: { ...title.content, ...withRunStyle(textBody(title), { size: 4400, bold: true, colour: '1E3A8A' }) } },
            { ...points, content: { ...points.content, ...withRunStyle(textBody(points), { size: 2400, colour: '334155' }) } },
            {
              ...badge,
              geometry: { type: 'roundedRectangle' },
              fill: { type: 'solid', colour: '2563EB' },
              line: null,
              content: {
                ...badge.content,
                ...withRunStyle(textBody(badge), { size: 4800, bold: true, colour: 'FFFFFF' }),
                anchor: 'middle',
                paragraphs: withRunStyle(textBody(badge), { size: 4800, bold: true, colour: 'FFFFFF' }).paragraphs.map((paragraph) => ({ ...paragraph, align: 'center' as const })),
              },
            },
            { ...newGeometryShape({ x: 7_900_000, y: 3_900_000, width: 1_400_000, height: 1_400_000 }, { type: 'ellipse' }), fill: { type: 'solid', colour: 'F59E0B' }, line: null },
            { ...newGeometryShape({ x: 9_500_000, y: 3_900_000, width: 1_400_000, height: 1_400_000 }, { type: 'ellipse' }), fill: { type: 'solid', colour: '10B981' }, line: null },
          ],
        },
        { shapes: [newTextShape({ x: 900_000, y: 900_000, width: 10_000_000, height: 1_200_000 }, 'Prossimi passi')] },
      ],
    }),
  );
}

/** The text body of a text shape. */
function textBody(shape: SlideShape): TextBody {
  if (shape.content.type !== 'text') throw new Error('not a text shape');
  return shape.content;
}

async function sampleSheet(): Promise<SheetDocument> {
  const workbook = await createSheet({ locale: 'it' });
  workbook.setInputs(0, [
    [0, 0, 'Mese'],
    [0, 1, 'Ricavi'],
    [0, 2, 'Costi'],
    [0, 3, 'Margine'],
    [1, 0, 'Gennaio'],
    [1, 1, '12000'],
    [1, 2, '8000'],
    [1, 3, '=B2-C2'],
    [2, 0, 'Febbraio'],
    [2, 1, '15000'],
    [2, 2, '9500'],
    [2, 3, '=B3-C3'],
    [3, 0, 'Marzo'],
    [3, 1, '17500'],
    [3, 2, '10200'],
    [3, 3, '=B4-C4'],
    [4, 0, 'Totale'],
    [4, 1, '=SOMMA(B2:B4)'],
    [4, 2, '=SOMMA(C2:C4)'],
    [4, 3, '=SOMMA(D2:D4)'],
  ]);
  workbook.setStyle(0, { top: 0, left: 0, bottom: 0, right: 3 }, { bold: true, fill: 'DBEAFE' });
  workbook.setStyle(0, { top: 4, left: 0, bottom: 4, right: 3 }, { bold: true });
  workbook.setColumnWidth(0, 0, 14);
  workbook.setFrozen(0, 1, 1);
  workbook.setAutoFilter(0, { top: 0, left: 0, bottom: 3, right: 3 });
  workbook.setConditionalFormats(0, [
    {
      ranges: [{ firstRow: 1, firstColumn: 3, lastRow: 3, lastColumn: 3 }],
      rules: [{ condition: { kind: 'dataBar', min: { kind: 'min' }, max: { kind: 'max' }, colour: '63BE7B', showValue: true } }],
    },
  ]);
  workbook.setValidations(0, [
    { ranges: [{ firstRow: 1, firstColumn: 1, lastRow: 3, lastColumn: 2 }], kind: 'decimal', operator: 'greaterOrEqual', formulas: ['0'] },
  ]);
  const chart = workbook.chartForRange(0, { top: 0, left: 0, bottom: 3, right: 2 }, 'column');
  if (chart) workbook.addChart(0, chart, { row: 0, column: 5, offsetX: 0, offsetY: 0, width: 4_572_000, height: 2_743_200 });
  workbook.startCommentThread(0, 3, 3, 'Margine sopra l\'obiettivo di marzo.', 'Anna Bianchi', '2026-10-28T09:00:00Z');
  workbook.replyToComment(0, 3, 3, 'Confermo, lo porto in riunione.', 'Marco Rossi', '2026-10-28T09:20:00Z');
  return workbook;
}

/** Two people on one document: their Yjs documents and presences hear each other at once. */
function sharedSessions(): [CollabSession, CollabSession] {
  const docs = [new Y.Doc(), new Y.Doc()] as const;
  const presences = docs.map((doc) => new Awareness(doc));
  docs.forEach((doc, index) => {
    const other = docs[1 - index]!;
    doc.on('update', (update: Uint8Array, origin: unknown) => {
      if (origin !== 'remote') Y.applyUpdate(other, update, 'remote');
    });
    presences[index]!.on('update', ({ added, updated, removed }: Record<string, number[]>, origin: unknown) => {
      if (origin === 'remote') return;
      const changed = [...added!, ...updated!, ...removed!];
      applyAwarenessUpdate(presences[1 - index]!, encodeAwarenessUpdate(presences[index]!, changed), 'remote');
    });
  });
  return [0, 1].map((index) => ({ doc: docs[index], awareness: presences[index] }) as unknown as CollabSession) as [
    CollabSession,
    CollabSession,
  ];
}

const PEOPLE = [
  { name: 'Anna Bianchi', color: '#2563eb' },
  { name: 'Marco Rossi', color: '#db2777' },
];

function Together({ document }: { document: EditorDocument }) {
  const [sessions] = useState(sharedSessions);
  const second = useRef<DocumentEditorHandle>(null);

  useEffect(() => {
    // For a still picture (`?still`): Marco selects a line as if his page had the focus, so Anna's
    // side shows his caret and selection without anyone clicking.
    if (!new URLSearchParams(location.search).has('still')) return undefined;
    const timer = setTimeout(() => {
      const editor = second.current?.editor;
      if (!editor) return;
      let from = -1;
      editor.state.doc.descendants((node, position) => {
        if (from < 0 && node.isText && node.text?.includes('Nuovi clienti')) from = position;
      });
      if (from < 0) return;
      editor.view.hasFocus = () => true;
      editor.commands.setTextSelection({ from, to: from + 'Nuovi clienti'.length });
    }, 600);
    return () => clearTimeout(timer);
  }, []);

  return (
    <div className="together">
      {sessions.map((session, index) => (
        <section key={PEOPLE[index]!.name}>
          <header style={{ borderColor: PEOPLE[index]!.color }}>{PEOPLE[index]!.name}</header>
          <DocumentEditor
            ref={index === 1 ? second : undefined}
            document={document}
            labels={ITALIAN_DOCUMENT_EDITOR_LABELS}
            collaboration={{
              doc: session.doc,
              extensions: documentCollaborationExtensions(session, PEOPLE[index]!),
              slot: index,
              seed: index === 0,
            }}
          />
        </section>
      ))}
    </div>
  );
}

function Demo() {
  const [view, setView] = useState<View>((new URLSearchParams(location.search).get('view') as View | null) ?? 'document');
  const [document, setDocument] = useState<EditorDocument | null>(null);
  const [presentation, setPresentation] = useState<Presentation | null>(null);
  const [sheet, setSheet] = useState<SheetDocument | null>(null);

  useEffect(() => {
    void (async () => {
      await initDocuments();
      setDocument(sampleDocument());
      setPresentation(samplePresentation());
      setSheet(await sampleSheet());
    })();
  }, []);

  return (
    <>
      <nav>
        {(['document', 'sheet', 'slides', 'together'] as View[]).map((each) => (
          <button key={each} type="button" aria-pressed={view === each} onClick={() => setView(each)}>
            {{ document: 'Documento', sheet: 'Foglio', slides: 'Presentazione', together: 'Insieme' }[each]}
          </button>
        ))}
      </nav>
      <main>
        {view === 'document' && document ? <DocumentEditor document={document} labels={ITALIAN_DOCUMENT_EDITOR_LABELS} pageView={new URLSearchParams(location.search).has('page')} /> : null}
        {view === 'sheet' && sheet ? <SheetGrid sheet={sheet} labels={ITALIAN_SHEET_GRID_LABELS} /> : null}
        {view === 'slides' && presentation ? <SlideEditor presentation={presentation} labels={ITALIAN_SLIDE_EDITOR_LABELS} /> : null}
        {view === 'together' && document ? <Together document={document} /> : null}
      </main>
    </>
  );
}

createRoot(window.document.getElementById('root')!).render(
  <StrictMode>
    <Demo />
  </StrictMode>,
);
