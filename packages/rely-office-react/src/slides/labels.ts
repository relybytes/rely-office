// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

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
  slideShow: string;
  play: string;
  playFromStart: string;
  endOfShow: string;
  effects: string;
  close: string;
  transition: string;
  direction: string;
  directionLeft: string;
  directionRight: string;
  directionUp: string;
  directionDown: string;
  duration: string;
  advanceOnClick: string;
  advanceAfter: string;
  animations: string;
  noAnimations: string;
  addAnimation: string;
  trigger: string;
  triggerOnClick: string;
  triggerWithPrevious: string;
  triggerAfterPrevious: string;
  delay: string;
  moveEarlier: string;
  moveLater: string;
  removeAnimation: string;
  entrance: string;
  emphasis: string;
  exit: string;
  rotate: string;
  shapes: string;
  table: string;
  addTableRow: string;
  addTableColumn: string;
  deleteTableRow: string;
  deleteTableColumn: string;
  cellFill: string;
  copy: string;
  cut: string;
  paste: string;
  duplicate: string;
  effectName: (name: string) => string;
  shapeName: (preset: string) => string;
  animationOf: (shape: number) => string;
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
  slideShow: 'Slide show',
  play: 'Play from this slide',
  playFromStart: 'Play from the start',
  endOfShow: 'End of the show. Click to leave.',
  effects: 'Transitions and animations',
  close: 'Close',
  transition: 'Transition',
  direction: 'Direction',
  directionLeft: 'From the right',
  directionRight: 'From the left',
  directionUp: 'From the bottom',
  directionDown: 'From the top',
  duration: 'Duration (s)',
  advanceOnClick: 'Advance on click',
  advanceAfter: 'Advance after (s)',
  animations: 'Animations',
  noAnimations: 'No animations on this slide',
  addAnimation: 'Add to the selection',
  trigger: 'Start',
  triggerOnClick: 'On click',
  triggerWithPrevious: 'With previous',
  triggerAfterPrevious: 'After previous',
  delay: 'Delay (s)',
  moveEarlier: 'Move earlier',
  moveLater: 'Move later',
  removeAnimation: 'Remove',
  entrance: 'Entrance',
  emphasis: 'Emphasis',
  exit: 'Exit',
  rotate: 'Rotate',
  shapes: 'Shapes',
  table: 'Table',
  addTableRow: 'Add row below',
  addTableColumn: 'Add column right',
  deleteTableRow: 'Delete row',
  deleteTableColumn: 'Delete column',
  cellFill: 'Cell colour',
  copy: 'Copy',
  cut: 'Cut',
  paste: 'Paste',
  duplicate: 'Duplicate',
  /** The name of a transition or an animation effect by its PresentationML name. */
  effectName: (name) => ({
    none: 'None', fade: 'Fade', push: 'Push', wipe: 'Wipe', split: 'Split', cover: 'Cover', pull: 'Uncover',
    zoom: 'Zoom', cut: 'Cut', dissolve: 'Dissolve', reveal: 'Reveal', appear: 'Appear', fly: 'Fly in', grow: 'Grow',
    spin: 'Spin', disappear: 'Disappear',
  } as Record<string, string>)[name] ?? name,
  shapeName: (preset) => ({
    rectangle: 'Rectangle', roundedRectangle: 'Rounded rectangle', ellipse: 'Ellipse', line: 'Line', triangle: 'Triangle',
    rightTriangle: 'Right triangle', diamond: 'Diamond', parallelogram: 'Parallelogram', trapezoid: 'Trapezoid',
    pentagon: 'Pentagon', hexagon: 'Hexagon', octagon: 'Octagon', star: 'Star', arrowRight: 'Right arrow',
    arrowLeft: 'Left arrow', arrowUp: 'Up arrow', arrowDown: 'Down arrow', chevron: 'Chevron', plus: 'Plus',
    heart: 'Heart', callout: 'Speech bubble',
  } as Record<string, string>)[preset] ?? preset,
  animationOf: (shape) => `Shape ${shape}`,
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
  slideShow: 'Presentazione',
  play: 'Presenta da questa diapositiva',
  playFromStart: 'Presenta dall\'inizio',
  endOfShow: 'Fine della presentazione. Fai clic per uscire.',
  effects: 'Transizioni e animazioni',
  close: 'Chiudi',
  transition: 'Transizione',
  direction: 'Direzione',
  directionLeft: 'Da destra',
  directionRight: 'Da sinistra',
  directionUp: 'Dal basso',
  directionDown: 'Dall\'alto',
  duration: 'Durata (s)',
  advanceOnClick: 'Avanza al clic',
  advanceAfter: 'Avanza dopo (s)',
  animations: 'Animazioni',
  noAnimations: 'Nessuna animazione in questa diapositiva',
  addAnimation: 'Aggiungi alla selezione',
  trigger: 'Avvio',
  triggerOnClick: 'Al clic',
  triggerWithPrevious: 'Con il precedente',
  triggerAfterPrevious: 'Dopo il precedente',
  delay: 'Ritardo (s)',
  moveEarlier: 'Sposta prima',
  moveLater: 'Sposta dopo',
  removeAnimation: 'Rimuovi',
  entrance: 'Entrata',
  emphasis: 'Enfasi',
  exit: 'Uscita',
  rotate: 'Ruota',
  shapes: 'Forme',
  table: 'Tabella',
  addTableRow: 'Aggiungi riga sotto',
  addTableColumn: 'Aggiungi colonna a destra',
  deleteTableRow: 'Elimina riga',
  deleteTableColumn: 'Elimina colonna',
  cellFill: 'Colore cella',
  copy: 'Copia',
  cut: 'Taglia',
  paste: 'Incolla',
  duplicate: 'Duplica',
  effectName: (name) => ({
    none: 'Nessuna', fade: 'Dissolvenza', push: 'Spinta', wipe: 'Pulisci', split: 'Dividi', cover: 'Copri', pull: 'Scopri',
    zoom: 'Zoom', cut: 'Taglio', dissolve: 'Dissolvi', reveal: 'Rivela', appear: 'Apparizione', fly: 'Entrata veloce',
    grow: 'Aumenta', spin: 'Rotazione', disappear: 'Scomparsa',
  } as Record<string, string>)[name] ?? name,
  shapeName: (preset) => ({
    rectangle: 'Rettangolo', roundedRectangle: 'Rettangolo arrotondato', ellipse: 'Ellisse', line: 'Linea', triangle: 'Triangolo',
    rightTriangle: 'Triangolo rettangolo', diamond: 'Rombo', parallelogram: 'Parallelogramma', trapezoid: 'Trapezio',
    pentagon: 'Pentagono', hexagon: 'Esagono', octagon: 'Ottagono', star: 'Stella', arrowRight: 'Freccia a destra',
    arrowLeft: 'Freccia a sinistra', arrowUp: 'Freccia in su', arrowDown: 'Freccia in giù', chevron: 'Gallone', plus: 'Croce',
    heart: 'Cuore', callout: 'Fumetto',
  } as Record<string, string>)[preset] ?? preset,
  animationOf: (shape) => `Forma ${shape}`,
};
