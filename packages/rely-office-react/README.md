# @relybytes/rely-office-react

Office documents in the browser, for the frontend: a docx, doc, odt, Markdown or HTML file, a
pptx, ppt or odp and an xlsx, xls, ods or csv read and written by [rely-office](https://github.com/relybytes/rely-office)
compiled to WebAssembly, four React editors over them, and real-time collaboration over Yjs.

| Component | Edits | Saves |
| --- | --- | --- |
| `DocumentEditor` | the document tree in a rich-text editor (TipTap): headings, Word's named styles, fonts, sizes, line spacing, every mark of the tree, nested lists, tables with merged cells and resizable columns, images resized, floating images with their wrapping and side, comments, tracked changes recorded while typing, footnote references, equations, page breaks, a page view with headers and footers, find and replace | docx, odt, Markdown or HTML |
| `SheetGrid` | a `SheetDocument`: cells, formulas with function suggestions, styles, borders, number formats, merges, conditional formats and data validation with their editors, comments, defined names, charts made from a selection, changed, dragged and resized, rows and columns resized by drag, the fill handle, frozen panes, sort and filter, the file's pictures, sheets, the clipboard | xlsx, ods or csv |
| `SlideEditor` | the slide model: shapes moved along guides, resized, rotated, selected several at a time, copied and written into, preset shapes, text styled, pictures, tables edited cell by cell, backgrounds, slides added, duplicated, reordered and hidden, speaker notes, transitions and animations, the slide show | pptx or odp |
| `TextEditor` | a plain text file, its caret and its long lines wrapped or not | the same file, in its encoding, byte order mark and line ending |

The backend has its own package, [`@relybytes/rely-office`](../rely-office-node/README.md), with
the same model plus rendering to PDF and PNG, which this package leaves out: the typesetter is
31 MB of WebAssembly and belongs on a server.

## Install

```sh
npm install @relybytes/rely-office-react
npm install react react-dom @tiptap/core @tiptap/pm @tiptap/react @tiptap/starter-kit @tiptap/extension-image @tiptap/extension-table
```

React 18 or later and TipTap 3 are peer dependencies, so an application that already has them
keeps one copy. Each release is published to the public npm registry and to GitHub Packages
under the `@relybytes` scope; reading it from GitHub Packages needs the `.npmrc` of the
[backend package](../rely-office-node/README.md#install).

## Load the WebAssembly

Two modules, loaded once and only when needed: `initDocuments` for documents and presentations,
`initSheets` for spreadsheets. By default each fetches its `.wasm` next to the module, which
webpack, Next.js and Vite resolve as an asset; pass a URL or the bytes otherwise.

```ts
import { initDocuments, initSheets } from '@relybytes/rely-office-react';

await initDocuments();
await initSheets('/static/rely_office_sheet_wasm_bg.wasm');
```

## A document

```tsx
import { DocumentEditor, ITALIAN_DOCUMENT_EDITOR_LABELS, openDocument, type DocumentEditorHandle } from '@relybytes/rely-office-react';
import '@relybytes/rely-office-react/styles.css';

const { document, report } = openDocument(bytes, { fileName: 'verbale.docx' }); // docx, doc, odt, md or html
const editor = useRef<DocumentEditorHandle>(null);

<DocumentEditor
  ref={editor}
  document={document}
  labels={ITALIAN_DOCUMENT_EDITOR_LABELS}
  onChange={(next) => scheduleAutosave(next)}
  onImageFile={async (file) => (file.size > 10_000_000 ? null : new Uint8Array(await file.arrayBuffer()))}
/>;

const docx = editor.current!.saveDocx(); // Uint8Array
const odt = saveDocumentAs(editor.current!.getDocument(), 'odt'); // or 'docx', 'markdown', 'html'
```

The macros of a docm ride along unrun and `saveDocx` writes them back, so the bytes are a docm
again (`report.macros` says the file held some); the same holds for a pptm in the slide editor
and an xlsm in the grid, whose `sheet.macros()` answers `kept`, `dropped` or `none`.

Beside the text, a panel edits what a document holds around it: the comments with their replies
and their resolved state, the notes in the order the text refers to them, the headers and footers,
a section's own included. Each is a small editor with the same schema as the page, so a header
keeps its table and its logo. The toolbar adds a comment on the selection (by `author`, or
`labels.you`) and a footnote at the caret; deleting a comment or a note takes its marks or its
reference out of the text, and saving keeps only what the text still refers to, the notes
numbered in order. `sidePanel={false}` leaves the panel to the host.

The toolbar sets the font, the size and the line spacing, and the style list holds Word's
named styles (title, subtitle, intense quote, caption, no spacing) beside the headings: they are
written to the docx as Word's own styles. `pageView` (or its toolbar button) lays the text on pages
of the document's size, with its margins, its header and its footer and the pages numbered; a
paragraph that does not fit is split between its lines and a table between its rows, and the page
number and page count a header or a footer shows are filled in on each page. A page break goes in
at the caret. Ctrl+F opens find and replace. With tracked changes on (`trackChanges`, or the
toolbar), what is typed is marked inserted, what is deleted stays, marked deleted, and a change of
formatting keeps the formatting it replaced, all by `author`, until it is accepted or rejected,
one change or all of them; rejecting a formatting change puts the old formatting back.
An image is resized by dragging its corner, its proportions kept, and a table column by dragging
its edge; both sizes are saved. A floating image stands beside the text on its side; chosen, the
toolbar sets how the text goes around it (in line, square, top and bottom, behind or in front of
the text) and its side, and it is saved where it was placed.

What the editor does not show rides along and is written back untouched: the page, the sections
and the embedded objects. `report` counts what the mapping had to leave out: marks a code run
cannot carry and images whose bytes are missing.

A host that draws its own editor uses the mapping and the extensions without the component:

```ts
import { Editor } from '@tiptap/core';
import { createRelyDocumentExtensions, documentTreeToEditor, editorToDocumentTree } from '@relybytes/rely-office-react';

const { document } = documentTreeToEditor(tree);
const editor = new Editor({ extensions: createRelyDocumentExtensions({ extensions: myOwn }), content: document.content });
const back = editorToDocumentTree({ ...document, content: editor.getJSON() });
```

## A spreadsheet

```tsx
import { SheetGrid, ITALIAN_SHEET_GRID_LABELS, openSheet } from '@relybytes/rely-office-react';

const sheet = await openSheet(bytes, 'conti.xlsx', { locale: 'it' });

<SheetGrid sheet={sheet} labels={ITALIAN_SHEET_GRID_LABELS} onChange={() => setDirty(true)} />;

const xlsx = sheet.save(); // or sheet.saveOds(), sheet.saveCsv(0)
```

The grid changes the workbook in place and draws only the cells in view, so a large sheet scrolls
as a small one. A cell is typed into directly or through the formula bar; Enter, Tab and the
arrows move, Shift extends, Delete clears, Ctrl+B, I and U style, Ctrl+Z and Ctrl+Y undo and redo.
The formula bar shows a formula in the workbook's locale, `=SOMMA(A1;0,5)` in Italian. A copy inside the workbook
pastes its formulas with their references moved; text from another program is pasted as typed
cells. A value a stop validation refuses is not written, and the message of the rule is shown.

Dragging the edge of a column or row header resizes it and a double click fits it; dragging the
square at the corner of the selection fills the cells it reaches, numbers along their trend, dates
a day at a time, months, weekdays and numbered texts counting on, formulas with their references
moved (Ctrl+D and Ctrl+R fill down and right). The toolbar sorts by the active column, puts filter
buttons on the table (each opens the list of its values to show), freezes the rows and columns
above and left of the active cell, sets borders, and opens the panels that add and remove the
conditional formats and the validations of the selection. While a formula is typed the functions
it could be calling are offered in the locale's names; Tab or Enter takes one. The pictures and
charts of the file are drawn over the cells, each chart from the cells as they are now.

The comments panel writes the comment of the active cell (by `author`), a new one opening a
thread as Excel 365 does, answers it, resolves and reopens it, and a commented cell shows a mark
and its comment with the replies on hover; the names panel defines the names formulas read. The
charts panel inserts a chart of the selection in the type chosen, and changes the chosen one: its
type, title, legend, stacking and data. A chart is chosen by a click, moved by dragging it,
resized by its corner and removed with Delete. The pivot panel makes a pivot table of the
selection, places each column of it on the rows, the columns, the values (with how they are
summarised) or the filters, sets its form (compact, outline or tabular), where its subtotals go
and whether its labels repeat, and refreshes and removes the tables of the sheet.

## A presentation

```tsx
import { SlideEditor, ITALIAN_SLIDE_EDITOR_LABELS, readPresentation, type SlideEditorHandle } from '@relybytes/rely-office-react';

const presentation = readPresentation(bytes); // pptx, odp or ppt
const slides = useRef<SlideEditorHandle>(null);

<SlideEditor ref={slides} presentation={presentation} labels={ITALIAN_SLIDE_EDITOR_LABELS} />;

const pptx = slides.current!.savePptx(); // or saveOdp()
```

A shape is moved by dragging it or with the arrows (Shift for larger steps), resized from its
corners, rotated from the handle above it (Shift for steps of 15 degrees) and written into with a
double click or Enter; the formatting of each run is kept while its text changes. A moved shape
snaps to the edges and centres of the slide and of the other shapes, and a guide shows where (Alt
moves freely). Shift and a click, or a box dragged over the slide, select several shapes, which
move, colour, copy and delete together; Ctrl+C, Ctrl+X, Ctrl+V and Ctrl+D copy, cut, paste and
duplicate them, and text or an image pasted from elsewhere becomes a text box or a picture. The
shapes menu draws twenty presets, from the triangle to the speech bubble. A table is written into
cell by cell, with rows and columns added or removed and each cell coloured. What a slide draws
from its layout and master shows locked. Ctrl+Z and Ctrl+Y undo and redo every change, a drag as
one, up to 100 of them.

The effects panel sets the slide's transition, its duration and when it advances, and adds,
orders and times the animations of the selected shapes. F5, or the play button, plays the show
over the whole screen: a click, the space bar or an arrow runs the next animation or brings the
next slide in with its transition, hidden slides are skipped and Escape leaves. `SlideShow` plays
a presentation by itself, for a host's own player.

## Editing together

Each editor takes a `collaboration` prop and edits a shared copy instead of its own: documents,
slides and plain text merge as a Yjs document, a spreadsheet as a log of operations a room orders.
`@relybytes/rely-office-react/collab` holds the room a server runs and the session a page opens,
with no React in it; `@relybytes/rely-office-react/collab-editor` holds the TipTap extensions of a
shared document, which need the optional peers:

```sh
npm install @tiptap/extension-collaboration @tiptap/extension-collaboration-caret @tiptap/y-tiptap
```

```tsx
import { CollabSession } from '@relybytes/rely-office-react/collab';
import { documentCollaborationExtensions } from '@relybytes/rely-office-react/collab-editor';

const session = new CollabSession({ kind: 'document', address: fetchTicketUrl, user: { name, color } });
session.connect();
// once `session.welcome` is set, on the 'control' event of the welcome:
const welcome = session.welcome!;
<DocumentEditor
  document={opened}
  collaboration={{
    doc: session.doc,
    extensions: documentCollaborationExtensions(session, { name, color }),
    slot: welcome.slot,
    seed: welcome.seed,
    onSeeded: () => session.markSeeded(),
  }}
/>;
```

The protocol, the room's storage and save claims, and each binding are described in
[docs/collaboration.md](../../docs/collaboration.md).

## The look

`styles.css` styles the three components; every colour, the radius, the font and the page width
are custom properties set on the component's root class, so a host restyles them on a parent:

```css
.my-app .rely-document-editor { --rely-accent: #7c3aed; --rely-font: 'Lato', sans-serif; }
```

The words of every toolbar come in English and Italian (`ENGLISH_*_LABELS`, `ITALIAN_*_LABELS`);
pass any of them to translate the rest.

## Fonts

A document opens in its own body font and size (`defaultFont` of the tree), and every family the
editor writes is a stack ending with the bundled family of the same metrics and the generic
family: `fontStack('Calibri')` is `'Calibri', 'Carlito', sans-serif`, so a machine without
Calibri lays the text out with Calibri's widths. The font picker lists the document's families
first (those its file lists and those its text uses), and says which bundled face a family the
browser lacks is shown with. The faces a document embeds are loaded with the FontFace API while
it is open (`loadDocumentFonts`) and written back on save.

The bundled families are opt-in, with their SIL Open Font License texts in `dist/fonts/`:

```ts
import '@relybytes/rely-office-react/fonts.css'; // Carlito, Caladea, Liberation Sans, Serif, Mono
```

Without it a substitute is used only where the machine has it.

## The runtime alone

`@relybytes/rely-office-react/runtime` exports the WebAssembly calls without the components, for
a page that only reads or converts: `detectFormat`, `extractText`, `readSheets`,
`readDocumentTree`, `writeDocx`, `writeOdt`, `writeMarkdown`, `writeHtml`, `convertToDocx`,
`readPresentation`, `writePptx`, `writeOdp`, `convertToPptx`, `createSheet` and `openSheet`, each
as the backend package has it.

## Develop

```sh
npm ci
npm test            # mapping, components and runtime, over the real WebAssembly
npm run typecheck
npm run demo        # the three editors on a local page
```

The `.wasm` modules come from `node scripts/build-npm.mjs` at the root of the repository.

## Licence

Mozilla Public License 2.0, as rely-office. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
