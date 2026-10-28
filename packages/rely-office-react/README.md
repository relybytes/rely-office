# @relybytes/rely-office-react

Office documents in the browser, for the frontend: a docx, doc or odt, a pptx, ppt or odp and an
xlsx, xls, ods or csv read and written by [rely-office](https://github.com/relybytes/rely-office)
compiled to WebAssembly, and three React editors over them.

| Component | Edits | Saves |
| --- | --- | --- |
| `DocumentEditor` | the document tree in a rich-text editor (TipTap): headings, paragraphs and their alignment, every mark of the tree, nested lists, tables with merged cells, images in the line, comments, tracked changes, footnote references, equations, section breaks | docx |
| `SheetGrid` | a `SheetDocument`: cells, formulas, styles, number formats, merges, conditional formats, data validation, rows, columns and sheets, the clipboard | xlsx or ods |
| `SlideEditor` | the slide model: shapes moved, resized and written into, text styled, pictures, tables, backgrounds, slides added, duplicated, reordered and hidden, speaker notes | pptx |

The backend has its own package, [`@relybytes/rely-office`](../rely-office-node/README.md), with
the same model plus rendering to PDF and PNG, which this package leaves out: the typesetter is
29 MB of WebAssembly and belongs on a server.

## Install

```sh
npm install @relybytes/rely-office-react
npm install react react-dom @tiptap/core @tiptap/pm @tiptap/react @tiptap/starter-kit @tiptap/extension-image @tiptap/extension-table
```

React 18 or later and TipTap 3 are peer dependencies, so an application that already has them
keeps one copy. The package is published on GitHub Packages under the `@relybytes` scope.

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

const { document, report } = openDocument(bytes); // docx, doc or odt
const editor = useRef<DocumentEditorHandle>(null);

<DocumentEditor
  ref={editor}
  document={document}
  labels={ITALIAN_DOCUMENT_EDITOR_LABELS}
  onChange={(next) => scheduleAutosave(next)}
  onImageFile={async (file) => (file.size > 10_000_000 ? null : new Uint8Array(await file.arrayBuffer()))}
/>;

const docx = editor.current!.saveDocx(); // Uint8Array
```

What the editor does not show as text rides along and is written back untouched: the notes the
references point at, the comments, the page, the headers and footers, the sections and the
embedded objects. `report` counts what the mapping had to leave out: marks a code run cannot carry
and images whose bytes are missing.

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

const xlsx = sheet.save(); // or sheet.saveOds()
```

The grid changes the workbook in place and draws only the cells in view, so a large sheet scrolls
as a small one. A cell is typed into directly or through the formula bar; Enter, Tab and the
arrows move, Shift extends, Delete clears, Ctrl+B, I and U style. A copy inside the workbook
pastes its formulas with their references moved; text from another program is pasted as typed
cells. A value a stop validation refuses is not written, and the message of the rule is shown.

## A presentation

```tsx
import { SlideEditor, ITALIAN_SLIDE_EDITOR_LABELS, readPresentation, type SlideEditorHandle } from '@relybytes/rely-office-react';

const presentation = readPresentation(bytes); // pptx, odp or ppt
const slides = useRef<SlideEditorHandle>(null);

<SlideEditor ref={slides} presentation={presentation} labels={ITALIAN_SLIDE_EDITOR_LABELS} />;

const pptx = slides.current!.savePptx();
```

A shape is moved by dragging it or with the arrows (Shift for larger steps), resized from its
corners and written into with a double click or Enter; the formatting of each run is kept while
its text changes. What a slide draws from its layout and master shows locked.

## The look

`styles.css` styles the three components; every colour, the radius, the font and the page width
are custom properties set on the component's root class, so a host restyles them on a parent:

```css
.my-app .rely-document-editor { --rely-accent: #7c3aed; --rely-font: 'Lato', sans-serif; }
```

The words of every toolbar come in English and Italian (`ENGLISH_*_LABELS`, `ITALIAN_*_LABELS`);
pass any of them to translate the rest.

## The runtime alone

`@relybytes/rely-office-react/runtime` exports the WebAssembly calls without the components, for
a page that only reads or converts: `detectFormat`, `extractText`, `readSheets`,
`readDocumentTree`, `writeDocx`, `convertToDocx`, `readPresentation`, `writePptx`,
`convertToPptx`, `createSheet` and `openSheet`, each as the backend package has it.

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
