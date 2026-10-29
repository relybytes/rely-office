# Documentation

| Document | Covers |
| --- | --- |
| [architecture.md](architecture.md) | why the library exists, crates and packages, the modules, the decisions, the WebAssembly boundary |
| [document-tree.md](document-tree.md) | the document tree of docx, doc, odt, Markdown and HTML: blocks, inline content, marks, floating images, comments, tracked changes, equations, media, embedded objects, the import report, an example |
| [rendering.md](rendering.md) | documents to PDF and PNG through Typst: what each format becomes, the layout rules, fonts, the corpus comparison with LibreOffice, fuzzing |
| [editing.md](editing.md) | writing a docx, an odt, Markdown and HTML from the tree and a pptx and an odp from the slides, Markdown and HTML read, and the editable spreadsheet: typing, formulas, names and functions, comments, charts, edits, conditional formats and validation, xlsx, ods and csv out, the JavaScript API |
| [roadmap.md](roadmap.md) | the steps and their status, known risks |

Each npm package has its own README: the backend package
[`@relybytes/rely-office`](../packages/rely-office-node/README.md) with the Node API and the
limits, the frontend package [`@relybytes/rely-office-react`](../packages/rely-office-react/README.md)
with the browser runtime and the editor components; [CHANGELOG.md](../CHANGELOG.md) lists the
changes of each release.
