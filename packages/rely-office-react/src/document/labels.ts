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
  imageWrap: string;
  wrapInline: string;
  wrapAround: string;
  wrapContour: string;
  wrapTopAndBottom: string;
  wrapBehind: string;
  wrapInFront: string;
  imagePosition: string;
  positionLeft: string;
  positionCenter: string;
  positionRight: string;
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
  addComment: string;
  commentText: string;
  reply: string;
  deleteComment: string;
  insertFootnote: string;
  notes: string;
  noteText: (number: number) => string;
  deleteNote: string;
  headersFooters: string;
  header: string;
  footer: string;
  firstHeader: string;
  firstFooter: string;
  evenHeader: string;
  evenFooter: string;
  section: (number: number) => string;
  /** The author of a comment when the host names none. */
  you: string;
  fontFamily: string;
  fontSize: string;
  defaultFont: string;
  /** Beside a family the browser does not have, the bundled one it is shown with. */
  fontShownAs: (substitute: string) => string;
  lineSpacing: string;
  styleTitle: string;
  styleSubtitle: string;
  styleIntenseQuote: string;
  styleCaption: string;
  styleNoSpacing: string;
  pageBreak: string;
  pageView: string;
  find: string;
  findText: string;
  replaceText: string;
  matchCase: string;
  findNext: string;
  findPrevious: string;
  replace: string;
  replaceAll: string;
  closeFind: string;
  trackChanges: string;
  acceptChange: string;
  rejectChange: string;
  /** How many places a search found, and which one is selected. */
  findCount: (current: number, total: number) => string;
  /** The words under a page in the page view. */
  pageNumber: (page: number) => string;
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
  imageWrap: 'Text wrapping',
  wrapInline: 'In line with text',
  wrapAround: 'Square',
  wrapContour: 'Tight',
  wrapTopAndBottom: 'Top and bottom',
  wrapBehind: 'Behind text',
  wrapInFront: 'In front of text',
  imagePosition: 'Position',
  positionLeft: 'Left',
  positionCenter: 'Centre',
  positionRight: 'Right',
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
  addComment: 'Add comment',
  commentText: 'Comment',
  reply: 'Reply',
  deleteComment: 'Delete comment',
  insertFootnote: 'Insert footnote',
  notes: 'Notes',
  noteText: (number) => `Note ${number}`,
  deleteNote: 'Delete note',
  headersFooters: 'Headers and footers',
  header: 'Header',
  footer: 'Footer',
  firstHeader: 'First page header',
  firstFooter: 'First page footer',
  evenHeader: 'Even page header',
  evenFooter: 'Even page footer',
  section: (number) => `Section ${number}`,
  you: 'You',
  fontFamily: 'Font',
  fontSize: 'Font size',
  defaultFont: 'Default',
  fontShownAs: (substitute) => `shown as ${substitute}`,
  lineSpacing: 'Line spacing',
  styleTitle: 'Title',
  styleSubtitle: 'Subtitle',
  styleIntenseQuote: 'Intense quote',
  styleCaption: 'Caption',
  styleNoSpacing: 'No spacing',
  pageBreak: 'Page break',
  pageView: 'Page view',
  find: 'Find and replace',
  findText: 'Find',
  replaceText: 'Replace with',
  matchCase: 'Match case',
  findNext: 'Next',
  findPrevious: 'Previous',
  replace: 'Replace',
  replaceAll: 'Replace all',
  closeFind: 'Close',
  trackChanges: 'Track changes',
  acceptChange: 'Accept change',
  rejectChange: 'Reject change',
  findCount: (current, total) => (total === 0 ? 'No results' : `${current} of ${total}`),
  pageNumber: (page) => `Page ${page}`,
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
  imageWrap: 'Disposizione testo',
  wrapInline: 'In linea con il testo',
  wrapAround: 'Incorniciato',
  wrapContour: 'Ravvicinato',
  wrapTopAndBottom: 'Sopra e sotto',
  wrapBehind: 'Dietro al testo',
  wrapInFront: 'Davanti al testo',
  imagePosition: 'Posizione',
  positionLeft: 'A sinistra',
  positionCenter: 'Al centro',
  positionRight: 'A destra',
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
  addComment: 'Aggiungi commento',
  commentText: 'Commento',
  reply: 'Rispondi',
  deleteComment: 'Elimina commento',
  insertFootnote: 'Inserisci nota a piè di pagina',
  notes: 'Note',
  noteText: (number) => `Nota ${number}`,
  deleteNote: 'Elimina nota',
  headersFooters: 'Intestazioni e piè di pagina',
  header: 'Intestazione',
  footer: 'Piè di pagina',
  firstHeader: 'Intestazione della prima pagina',
  firstFooter: 'Piè di pagina della prima pagina',
  evenHeader: 'Intestazione delle pagine pari',
  evenFooter: 'Piè di pagina delle pagine pari',
  section: (number) => `Sezione ${number}`,
  you: 'Tu',
  fontFamily: 'Carattere',
  fontSize: 'Dimensione carattere',
  defaultFont: 'Predefinito',
  fontShownAs: (substitute) => `mostrato con ${substitute}`,
  lineSpacing: 'Interlinea',
  styleTitle: 'Titolo',
  styleSubtitle: 'Sottotitolo',
  styleIntenseQuote: 'Citazione intensa',
  styleCaption: 'Didascalia',
  styleNoSpacing: 'Nessuna spaziatura',
  pageBreak: 'Interruzione di pagina',
  pageView: 'Vista pagina',
  find: 'Trova e sostituisci',
  findText: 'Trova',
  replaceText: 'Sostituisci con',
  matchCase: 'Maiuscole/minuscole',
  findNext: 'Successivo',
  findPrevious: 'Precedente',
  replace: 'Sostituisci',
  replaceAll: 'Sostituisci tutto',
  closeFind: 'Chiudi',
  trackChanges: 'Revisioni',
  acceptChange: 'Accetta modifica',
  rejectChange: 'Rifiuta modifica',
  findCount: (current, total) => (total === 0 ? 'Nessun risultato' : `${current} di ${total}`),
  pageNumber: (page) => `Pagina ${page}`,
};
