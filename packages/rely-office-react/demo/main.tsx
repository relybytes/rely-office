// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// A page with the three editors over documents made in the page itself: `npm run demo`.

import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
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
  writePptx,
  type EditorDocument,
  type Presentation,
  type SheetDocument,
  type TableCell,
} from '../src/index.js';
import '../src/styles.css';

type View = 'document' | 'sheet' | 'slides';

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
                { offset: 0, colour: 'EFF6FF' },
                { offset: 1, colour: 'FFFFFF' },
              ],
              kind: { type: 'linear', angle: 90 },
            },
          },
          shapes: [
            newTextShape({ x: 900_000, y: 900_000, width: 10_000_000, height: 1_200_000 }, 'Risultati 2026'),
            newTextShape(
              { x: 900_000, y: 2_300_000, width: 6_000_000, height: 2_400_000 },
              'Ricavi in crescita del 18%\nTre nuovi clienti\nMargine stabile',
            ),
            newGeometryShape({ x: 7_800_000, y: 2_300_000, width: 3_000_000, height: 3_000_000 }, { type: 'ellipse' }),
          ],
        },
        { shapes: [newTextShape({ x: 900_000, y: 900_000, width: 10_000_000, height: 1_200_000 }, 'Prossimi passi')] },
      ],
    }),
  );
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
  return workbook;
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
        {(['document', 'sheet', 'slides'] as View[]).map((each) => (
          <button key={each} type="button" aria-pressed={view === each} onClick={() => setView(each)}>
            {{ document: 'Documento', sheet: 'Foglio', slides: 'Presentazione' }[each]}
          </button>
        ))}
      </nav>
      <main>
        {view === 'document' && document ? <DocumentEditor document={document} labels={ITALIAN_DOCUMENT_EDITOR_LABELS} pageView={new URLSearchParams(location.search).has('page')} /> : null}
        {view === 'sheet' && sheet ? <SheetGrid sheet={sheet} labels={ITALIAN_SHEET_GRID_LABELS} /> : null}
        {view === 'slides' && presentation ? <SlideEditor presentation={presentation} labels={ITALIAN_SLIDE_EDITOR_LABELS} /> : null}
      </main>
    </>
  );
}

createRoot(window.document.getElementById('root')!).render(
  <StrictMode>
    <Demo />
  </StrictMode>,
);
