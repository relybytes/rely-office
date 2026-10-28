// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

/** The words of the spreadsheet grid; a host passes its own language. */
export interface SheetGridLabels {
  toolbar: string;
  grid: string;
  sheets: string;
  cellName: string;
  formula: string;
  bold: string;
  italic: string;
  underline: string;
  strike: string;
  fontColour: string;
  fill: string;
  alignLeft: string;
  alignCenter: string;
  alignRight: string;
  wrap: string;
  numberFormat: string;
  formatGeneral: string;
  formatNumber: string;
  formatPercent: string;
  formatDate: string;
  formatCurrency: string;
  merge: string;
  unmerge: string;
  insertRow: string;
  deleteRow: string;
  insertColumn: string;
  deleteColumn: string;
  clear: string;
  addSheet: string;
  removeSheet: string;
  renameSheet: string;
  /** The name a new sheet gets, from its number. */
  newSheetName: (number: number) => string;
  invalidValue: string;
}

export const ENGLISH_SHEET_GRID_LABELS: SheetGridLabels = {
  toolbar: 'Formatting',
  grid: 'Sheet',
  sheets: 'Sheets',
  cellName: 'Cell',
  formula: 'Contents',
  bold: 'Bold',
  italic: 'Italic',
  underline: 'Underline',
  strike: 'Strikethrough',
  fontColour: 'Text colour',
  fill: 'Fill colour',
  alignLeft: 'Align left',
  alignCenter: 'Centre',
  alignRight: 'Align right',
  wrap: 'Wrap text',
  numberFormat: 'Number format',
  formatGeneral: 'General',
  formatNumber: 'Number',
  formatPercent: 'Percentage',
  formatDate: 'Date',
  formatCurrency: 'Currency',
  merge: 'Merge cells',
  unmerge: 'Unmerge cells',
  insertRow: 'Insert row above',
  deleteRow: 'Delete row',
  insertColumn: 'Insert column left',
  deleteColumn: 'Delete column',
  clear: 'Clear',
  addSheet: 'Add sheet',
  removeSheet: 'Remove sheet',
  renameSheet: 'Sheet name',
  newSheetName: (number) => `Sheet${number}`,
  invalidValue: 'This value is not accepted',
};

export const ITALIAN_SHEET_GRID_LABELS: SheetGridLabels = {
  toolbar: 'Formattazione',
  grid: 'Foglio',
  sheets: 'Fogli',
  cellName: 'Cella',
  formula: 'Contenuto',
  bold: 'Grassetto',
  italic: 'Corsivo',
  underline: 'Sottolineato',
  strike: 'Barrato',
  fontColour: 'Colore del testo',
  fill: 'Colore di riempimento',
  alignLeft: 'Allinea a sinistra',
  alignCenter: 'Centra',
  alignRight: 'Allinea a destra',
  wrap: 'Testo a capo',
  numberFormat: 'Formato numero',
  formatGeneral: 'Generale',
  formatNumber: 'Numero',
  formatPercent: 'Percentuale',
  formatDate: 'Data',
  formatCurrency: 'Valuta',
  merge: 'Unisci celle',
  unmerge: 'Separa celle',
  insertRow: 'Inserisci riga sopra',
  deleteRow: 'Elimina riga',
  insertColumn: 'Inserisci colonna a sinistra',
  deleteColumn: 'Elimina colonna',
  clear: 'Cancella',
  addSheet: 'Aggiungi foglio',
  removeSheet: 'Elimina foglio',
  renameSheet: 'Nome del foglio',
  newSheetName: (number) => `Foglio${number}`,
  invalidValue: 'Questo valore non è accettato',
};
