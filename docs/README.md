# Documentation

| Document | Covers |
| --- | --- |
| [architecture.md](architecture.md) | why the library exists, crates and packages, the modules, the decisions, the WebAssembly boundary |
| [document-tree.md](document-tree.md) | the document tree of docx, doc and odt: blocks, inline content, marks, comments, tracked changes, equations, media, embedded objects, the import report, an example |
| [rendering.md](rendering.md) | documents to PDF and PNG through Typst: what each format becomes, the layout rules, fonts, the corpus comparison with LibreOffice, fuzzing |
| [editing.md](editing.md) | writing a docx from the tree and a pptx from the slides, and the editable spreadsheet: typing, formulas, functions, edits, conditional formats and validation, xlsx and ods out, the JavaScript API |
| [roadmap.md](roadmap.md) | the steps and their status, known risks |

The npm package has its own [README](../packages/rely-office-node/README.md) with the Node and
browser API and the limits; [CHANGELOG.md](../CHANGELOG.md) lists the changes of each release.
