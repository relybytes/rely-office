// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

/** The words of the slide editor; a host passes its own language. */
export interface SlideEditorLabels {
  toolbar: string;
  undo: string;
  redo: string;
  slides: string;
  slide: (number: number) => string;
  addSlide: string;
  duplicateSlide: string;
  deleteSlide: string;
  moveSlideUp: string;
  moveSlideDown: string;
  hideSlide: string;
  textBox: string;
  newText: string;
  rectangle: string;
  ellipse: string;
  image: string;
  deleteShape: string;
  bringForward: string;
  sendBackward: string;
  bold: string;
  italic: string;
  underline: string;
  fontSize: string;
  textColour: string;
  alignLeft: string;
  alignCenter: string;
  alignRight: string;
  fill: string;
  outline: string;
  canvas: string;
  shape: (number: number) => string;
  lockedShape: string;
  notes: string;
  resize: string;
}

export const ENGLISH_SLIDE_EDITOR_LABELS: SlideEditorLabels = {
  toolbar: 'Slide tools',
  undo: 'Undo',
  redo: 'Redo',
  slides: 'Slides',
  slide: (number) => `Slide ${number}`,
  addSlide: 'New slide',
  duplicateSlide: 'Duplicate slide',
  deleteSlide: 'Delete slide',
  moveSlideUp: 'Move slide up',
  moveSlideDown: 'Move slide down',
  hideSlide: 'Hide slide in the show',
  textBox: 'Text box',
  newText: 'Text',
  rectangle: 'Rectangle',
  ellipse: 'Ellipse',
  image: 'Image',
  deleteShape: 'Delete shape',
  bringForward: 'Bring forward',
  sendBackward: 'Send backward',
  bold: 'Bold',
  italic: 'Italic',
  underline: 'Underline',
  fontSize: 'Font size',
  textColour: 'Text colour',
  alignLeft: 'Align left',
  alignCenter: 'Centre',
  alignRight: 'Align right',
  fill: 'Fill colour',
  outline: 'Outline colour',
  canvas: 'Slide',
  shape: (number) => `Shape ${number}`,
  lockedShape: 'From the layout',
  notes: 'Speaker notes',
  resize: 'Resize',
};

export const ITALIAN_SLIDE_EDITOR_LABELS: SlideEditorLabels = {
  toolbar: 'Strumenti della diapositiva',
  undo: 'Annulla',
  redo: 'Ripeti',
  slides: 'Diapositive',
  slide: (number) => `Diapositiva ${number}`,
  addSlide: 'Nuova diapositiva',
  duplicateSlide: 'Duplica diapositiva',
  deleteSlide: 'Elimina diapositiva',
  moveSlideUp: 'Sposta su',
  moveSlideDown: 'Sposta giù',
  hideSlide: 'Nascondi nella presentazione',
  textBox: 'Casella di testo',
  newText: 'Testo',
  rectangle: 'Rettangolo',
  ellipse: 'Ellisse',
  image: 'Immagine',
  deleteShape: 'Elimina forma',
  bringForward: 'Porta avanti',
  sendBackward: 'Porta indietro',
  bold: 'Grassetto',
  italic: 'Corsivo',
  underline: 'Sottolineato',
  fontSize: 'Dimensione del testo',
  textColour: 'Colore del testo',
  alignLeft: 'Allinea a sinistra',
  alignCenter: 'Centra',
  alignRight: 'Allinea a destra',
  fill: 'Colore di riempimento',
  outline: 'Colore del bordo',
  canvas: 'Diapositiva',
  shape: (number) => `Forma ${number}`,
  lockedShape: 'Dal layout',
  notes: 'Note del relatore',
  resize: 'Ridimensiona',
};
