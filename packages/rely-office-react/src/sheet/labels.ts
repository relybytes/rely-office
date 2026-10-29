// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

/** The words of the spreadsheet grid; a host passes its own language. */
export interface SheetGridLabels {
  toolbar: string;
  undo: string;
  redo: string;
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
  sortAscending: string;
  sortDescending: string;
  filter: string;
  filterColumn: string;
  filterSearch: string;
  filterSelectAll: string;
  filterBlank: string;
  apply: string;
  cancel: string;
  freeze: string;
  unfreeze: string;
  borders: string;
  borderAll: string;
  borderOutline: string;
  borderInside: string;
  borderTop: string;
  borderBottom: string;
  borderLeft: string;
  borderRight: string;
  borderNone: string;
  borderLine: string;
  borderColour: string;
  lineThin: string;
  lineMedium: string;
  lineThick: string;
  lineDashed: string;
  lineDotted: string;
  lineDouble: string;
  conditionalFormats: string;
  validation: string;
  close: string;
  remove: string;
  addRule: string;
  appliesTo: string;
  noRules: string;
  ruleKind: string;
  ruleGreater: string;
  ruleLess: string;
  ruleBetween: string;
  ruleEqual: string;
  ruleTextContains: string;
  ruleDuplicates: string;
  ruleTop: string;
  ruleBlanks: string;
  ruleColorScale: string;
  ruleDataBar: string;
  ruleExpression: string;
  value: string;
  secondValue: string;
  lowColour: string;
  highColour: string;
  ruleIncomplete: string;
  validationKind: string;
  validationAny: string;
  validationList: string;
  validationWhole: string;
  validationDecimal: string;
  validationDate: string;
  validationTextLength: string;
  validationCustom: string;
  validationItems: string;
  operator: string;
  operatorBetween: string;
  operatorNotBetween: string;
  operatorEqual: string;
  operatorNotEqual: string;
  operatorGreater: string;
  operatorLess: string;
  operatorGreaterOrEqual: string;
  operatorLessOrEqual: string;
  allowBlank: string;
  errorStyle: string;
  errorStop: string;
  errorWarning: string;
  errorInformation: string;
  errorMessage: string;
  promptMessage: string;
  saveValidation: string;
  comments: string;
  commentOn: string;
  commentPlaceholder: string;
  saveComment: string;
  noComments: string;
  definedNames: string;
  noNames: string;
  name: string;
  nameRefersTo: string;
  nameScope: string;
  wholeWorkbook: string;
  addName: string;
  invalidName: string;
  functions: string;
  resizeColumn: string;
  resizeRow: string;
  fillHandle: string;
}

export const ENGLISH_SHEET_GRID_LABELS: SheetGridLabels = {
  toolbar: 'Formatting',
  undo: 'Undo',
  redo: 'Redo',
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
  sortAscending: 'Sort A to Z',
  sortDescending: 'Sort Z to A',
  filter: 'Filter',
  filterColumn: 'Filter column',
  filterSearch: 'Search',
  filterSelectAll: 'Select all',
  filterBlank: '(Blanks)',
  apply: 'OK',
  cancel: 'Cancel',
  freeze: 'Freeze panes at the selection',
  unfreeze: 'Unfreeze panes',
  borders: 'Borders',
  borderAll: 'All borders',
  borderOutline: 'Outside borders',
  borderInside: 'Inside borders',
  borderTop: 'Top border',
  borderBottom: 'Bottom border',
  borderLeft: 'Left border',
  borderRight: 'Right border',
  borderNone: 'No border',
  borderLine: 'Line',
  borderColour: 'Border colour',
  lineThin: 'Thin',
  lineMedium: 'Medium',
  lineThick: 'Thick',
  lineDashed: 'Dashed',
  lineDotted: 'Dotted',
  lineDouble: 'Double',
  conditionalFormats: 'Conditional formatting',
  validation: 'Data validation',
  close: 'Close',
  remove: 'Remove',
  addRule: 'Add rule',
  appliesTo: 'Applies to',
  noRules: 'No rules on this sheet',
  ruleKind: 'Format cells that',
  ruleGreater: 'are greater than',
  ruleLess: 'are less than',
  ruleBetween: 'are between',
  ruleEqual: 'are equal to',
  ruleTextContains: 'contain the text',
  ruleDuplicates: 'are duplicated',
  ruleTop: 'are in the top',
  ruleBlanks: 'are blank',
  ruleColorScale: 'get a colour scale',
  ruleDataBar: 'get a data bar',
  ruleExpression: 'make a formula true',
  value: 'Value',
  secondValue: 'and',
  lowColour: 'Lowest',
  highColour: 'Highest',
  ruleIncomplete: 'Fill in the values of the rule',
  validationKind: 'Allow',
  validationAny: 'Any value',
  validationList: 'List',
  validationWhole: 'Whole number',
  validationDecimal: 'Decimal',
  validationDate: 'Date',
  validationTextLength: 'Text length',
  validationCustom: 'Custom formula',
  validationItems: 'Items, separated by a semicolon',
  operator: 'Data',
  operatorBetween: 'between',
  operatorNotBetween: 'not between',
  operatorEqual: 'equal to',
  operatorNotEqual: 'not equal to',
  operatorGreater: 'greater than',
  operatorLess: 'less than',
  operatorGreaterOrEqual: 'greater than or equal to',
  operatorLessOrEqual: 'less than or equal to',
  allowBlank: 'Ignore blank cells',
  errorStyle: 'When the value is not valid',
  errorStop: 'Refuse it',
  errorWarning: 'Warn',
  errorInformation: 'Inform',
  errorMessage: 'Error message',
  promptMessage: 'Input message',
  saveValidation: 'Apply to the selection',
  comments: 'Comments',
  commentOn: 'Comment on',
  commentPlaceholder: 'Write a comment',
  saveComment: 'Save comment',
  noComments: 'No comments on this sheet',
  definedNames: 'Names',
  noNames: 'The workbook defines no names',
  name: 'Name',
  nameRefersTo: 'Refers to',
  nameScope: 'Scope',
  wholeWorkbook: 'Workbook',
  addName: 'Define name',
  invalidName: 'A name starts with a letter and holds letters, digits, dots and underscores; it cannot look like a cell',
  functions: 'Functions',
  resizeColumn: 'Resize column; double click fits it',
  resizeRow: 'Resize row; double click fits it',
  fillHandle: 'Drag to fill',
};

export const ITALIAN_SHEET_GRID_LABELS: SheetGridLabels = {
  toolbar: 'Formattazione',
  undo: 'Annulla',
  redo: 'Ripeti',
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
  sortAscending: 'Ordina dalla A alla Z',
  sortDescending: 'Ordina dalla Z alla A',
  filter: 'Filtro',
  filterColumn: 'Filtra colonna',
  filterSearch: 'Cerca',
  filterSelectAll: 'Seleziona tutto',
  filterBlank: '(Vuote)',
  apply: 'OK',
  cancel: 'Annulla',
  freeze: 'Blocca riquadri alla selezione',
  unfreeze: 'Sblocca riquadri',
  borders: 'Bordi',
  borderAll: 'Tutti i bordi',
  borderOutline: 'Bordi esterni',
  borderInside: 'Bordi interni',
  borderTop: 'Bordo superiore',
  borderBottom: 'Bordo inferiore',
  borderLeft: 'Bordo sinistro',
  borderRight: 'Bordo destro',
  borderNone: 'Nessun bordo',
  borderLine: 'Linea',
  borderColour: 'Colore del bordo',
  lineThin: 'Sottile',
  lineMedium: 'Media',
  lineThick: 'Spessa',
  lineDashed: 'Tratteggiata',
  lineDotted: 'Punteggiata',
  lineDouble: 'Doppia',
  conditionalFormats: 'Formattazione condizionale',
  validation: 'Convalida dati',
  close: 'Chiudi',
  remove: 'Rimuovi',
  addRule: 'Aggiungi regola',
  appliesTo: 'Si applica a',
  noRules: 'Nessuna regola in questo foglio',
  ruleKind: 'Formatta le celle che',
  ruleGreater: 'sono maggiori di',
  ruleLess: 'sono minori di',
  ruleBetween: 'sono comprese fra',
  ruleEqual: 'sono uguali a',
  ruleTextContains: 'contengono il testo',
  ruleDuplicates: 'sono duplicate',
  ruleTop: 'sono fra i primi',
  ruleBlanks: 'sono vuote',
  ruleColorScale: 'hanno una scala di colori',
  ruleDataBar: 'hanno una barra dei dati',
  ruleExpression: 'rendono vera una formula',
  value: 'Valore',
  secondValue: 'e',
  lowColour: 'Minimo',
  highColour: 'Massimo',
  ruleIncomplete: 'Completa i valori della regola',
  validationKind: 'Consenti',
  validationAny: 'Qualsiasi valore',
  validationList: 'Elenco',
  validationWhole: 'Numero intero',
  validationDecimal: 'Decimale',
  validationDate: 'Data',
  validationTextLength: 'Lunghezza testo',
  validationCustom: 'Formula personalizzata',
  validationItems: 'Voci, separate da punto e virgola',
  operator: 'Dati',
  operatorBetween: 'compreso fra',
  operatorNotBetween: 'non compreso fra',
  operatorEqual: 'uguale a',
  operatorNotEqual: 'diverso da',
  operatorGreater: 'maggiore di',
  operatorLess: 'minore di',
  operatorGreaterOrEqual: 'maggiore o uguale a',
  operatorLessOrEqual: 'minore o uguale a',
  allowBlank: 'Ignora celle vuote',
  errorStyle: 'Quando il valore non è valido',
  errorStop: 'Rifiutalo',
  errorWarning: 'Avvisa',
  errorInformation: 'Informa',
  errorMessage: 'Messaggio di errore',
  promptMessage: 'Messaggio di input',
  saveValidation: 'Applica alla selezione',
  comments: 'Commenti',
  commentOn: 'Commento su',
  commentPlaceholder: 'Scrivi un commento',
  saveComment: 'Salva commento',
  noComments: 'Nessun commento in questo foglio',
  definedNames: 'Nomi',
  noNames: 'La cartella non definisce nomi',
  name: 'Nome',
  nameRefersTo: 'Riferito a',
  nameScope: 'Ambito',
  wholeWorkbook: 'Cartella di lavoro',
  addName: 'Definisci nome',
  invalidName: 'Un nome inizia con una lettera e contiene lettere, cifre, punti e trattini bassi; non può sembrare una cella',
  functions: 'Funzioni',
  resizeColumn: 'Ridimensiona colonna; doppio clic per adattarla',
  resizeRow: 'Ridimensiona riga; doppio clic per adattarla',
  fillHandle: 'Trascina per riempire',
};
