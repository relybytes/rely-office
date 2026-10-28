// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

/** The words of the document editor's toolbar; a host passes its own language. */
export interface DocumentEditorLabels {
  toolbar: string;
  undo: string;
  redo: string;
  textStyle: string;
  paragraph: string;
  heading: (level: number) => string;
  bold: string;
  italic: string;
  underline: string;
  strike: string;
  code: string;
  superscript: string;
  subscript: string;
  textColour: string;
  highlight: string;
  clearHighlight: string;
  alignLeft: string;
  alignCenter: string;
  alignRight: string;
  alignJustify: string;
  bulletList: string;
  orderedList: string;
  indent: string;
  outdent: string;
  quote: string;
  codeBlock: string;
  rule: string;
  link: string;
  linkAddress: string;
  applyLink: string;
  removeLink: string;
  image: string;
  equation: string;
  equationLatex: string;
  insertTable: string;
  addRow: string;
  addColumn: string;
  deleteRow: string;
  deleteColumn: string;
  mergeCells: string;
  splitCell: string;
  deleteTable: string;
  acceptChanges: string;
  rejectChanges: string;
  clearFormatting: string;
  comments: string;
  resolved: string;
  sectionBreak: string;
}

export const ENGLISH_DOCUMENT_EDITOR_LABELS: DocumentEditorLabels = {
  toolbar: 'Formatting',
  undo: 'Undo',
  redo: 'Redo',
  textStyle: 'Text style',
  paragraph: 'Paragraph',
  heading: (level) => `Heading ${level}`,
  bold: 'Bold',
  italic: 'Italic',
  underline: 'Underline',
  strike: 'Strikethrough',
  code: 'Code',
  superscript: 'Superscript',
  subscript: 'Subscript',
  textColour: 'Text colour',
  highlight: 'Highlight',
  clearHighlight: 'No highlight',
  alignLeft: 'Align left',
  alignCenter: 'Centre',
  alignRight: 'Align right',
  alignJustify: 'Justify',
  bulletList: 'Bulleted list',
  orderedList: 'Numbered list',
  indent: 'Increase indent',
  outdent: 'Decrease indent',
  quote: 'Quote',
  codeBlock: 'Code block',
  rule: 'Horizontal line',
  link: 'Link',
  linkAddress: 'Address',
  applyLink: 'Apply',
  removeLink: 'Remove link',
  image: 'Image',
  equation: 'Equation',
  equationLatex: 'Equation in LaTeX',
  insertTable: 'Insert table',
  addRow: 'Add row',
  addColumn: 'Add column',
  deleteRow: 'Delete row',
  deleteColumn: 'Delete column',
  mergeCells: 'Merge cells',
  splitCell: 'Split cell',
  deleteTable: 'Delete table',
  acceptChanges: 'Accept all changes',
  rejectChanges: 'Reject all changes',
  clearFormatting: 'Clear formatting',
  comments: 'Comments',
  resolved: 'Resolved',
  sectionBreak: 'Section break',
};

export const ITALIAN_DOCUMENT_EDITOR_LABELS: DocumentEditorLabels = {
  toolbar: 'Formattazione',
  undo: 'Annulla',
  redo: 'Ripeti',
  textStyle: 'Stile del testo',
  paragraph: 'Paragrafo',
  heading: (level) => `Titolo ${level}`,
  bold: 'Grassetto',
  italic: 'Corsivo',
  underline: 'Sottolineato',
  strike: 'Barrato',
  code: 'Codice',
  superscript: 'Apice',
  subscript: 'Pedice',
  textColour: 'Colore del testo',
  highlight: 'Evidenzia',
  clearHighlight: 'Nessuna evidenziazione',
  alignLeft: 'Allinea a sinistra',
  alignCenter: 'Centra',
  alignRight: 'Allinea a destra',
  alignJustify: 'Giustifica',
  bulletList: 'Elenco puntato',
  orderedList: 'Elenco numerato',
  indent: 'Aumenta rientro',
  outdent: 'Riduci rientro',
  quote: 'Citazione',
  codeBlock: 'Blocco di codice',
  rule: 'Linea orizzontale',
  link: 'Collegamento',
  linkAddress: 'Indirizzo',
  applyLink: 'Applica',
  removeLink: 'Rimuovi collegamento',
  image: 'Immagine',
  equation: 'Equazione',
  equationLatex: 'Equazione in LaTeX',
  insertTable: 'Inserisci tabella',
  addRow: 'Aggiungi riga',
  addColumn: 'Aggiungi colonna',
  deleteRow: 'Elimina riga',
  deleteColumn: 'Elimina colonna',
  mergeCells: 'Unisci celle',
  splitCell: 'Dividi cella',
  deleteTable: 'Elimina tabella',
  acceptChanges: 'Accetta tutte le revisioni',
  rejectChanges: 'Rifiuta tutte le revisioni',
  clearFormatting: 'Cancella formattazione',
  comments: 'Commenti',
  resolved: 'Risolto',
  sectionBreak: 'Interruzione di sezione',
};
