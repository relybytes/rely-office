# Collaboration

Several people editing the same file at once, in the frontend package
`@relybytes/rely-office-react`. The library carries both ends of the channel without a transport:
`CollabRoom` is what a server runs for each open file, `CollabSession` is what a page opens, and a
binding per kind of file joins a session to its editor. The host owns the WebSocket, the
authentication, the storage of the room between connections and the saving of versions.

| Entry point | Holds | Needs |
| --- | --- | --- |
| `@relybytes/rely-office-react/collab` | the protocol, `CollabRoom`, `CollabSession`, the sheet, document and presentation bindings | `yjs`, `y-protocols`, `lib0` (dependencies of the package); no React, so a server imports it |
| `@relybytes/rely-office-react/collab-editor` | `documentCollaborationExtensions(session, user)`, the TipTap side of a shared document | the optional peers `@tiptap/extension-collaboration`, `@tiptap/extension-collaboration-caret`, `@tiptap/y-tiptap` |
| `@relybytes/rely-office-react` | the `collaboration` prop of `DocumentEditor`, `SheetGrid`, `SlideEditor` and `TextEditor` | React |

Every peer dependency is optional, so a server that runs rooms installs none of React or TipTap.

## Two models

| Kind (`CollabKind`) | Shared as | Why |
| --- | --- | --- |
| `document`, `presentation`, `text` | a Yjs document, merged by its CRDT | text and lists of shapes merge without a server deciding |
| `spreadsheet` | a log of operations ordered by the room | a formula, a sort or a row insert rewrites cells across sheets: the workbook's own engine must apply each edit, in one order, on every client |

## The channel

Binary frames, each starting with its kind as a variable-length integer (`CollabFrame`):

| Frame | Carries |
| --- | --- |
| `Sync` (0) | the Yjs sync messages of `y-protocols` |
| `Awareness` (1) | presence: who is here, their cursor, their selection |
| `Control` (2) | a JSON control message |
| `Operation` (3) | a JSON spreadsheet operation |

A client opens with `hello`, carrying a session key that is random and kept across reconnections
and the last operation it applied. The room answers `welcome`: the client's `slot`, whether it may
write, the base version of the file the room starts from, the room's sequence, and three flags:

- `seed`: this client puts the file into an empty shared document and answers `seeded`. One
  writing client seeds at a time; if it leaves first, the room sends `seed` to the next.
- `reload`: the client applied operations the base version does not hold, or missed some the room
  no longer keeps, and must open the base version again.
- `dirty`: the room holds changes no version of the file has.

For a spreadsheet the welcome is followed by the operations the client missed, then `ready`.
`resync` tells a client one of its operations cannot be placed; `error` refuses with a code
(`READ_ONLY`, `INVALID_FRAME`, `NOT_SEEDER`, `SAVE_REFUSED`).

## The room

`new CollabRoom(state, { transform, events, historyLimit })` is built from what the host stored
(`CollabRoomState`: kind, base version, sequences, `seeded`, `dirty`, the Yjs document as one
update, the operations after the base version). The host calls `join(peer)`, `receive(peerId,
frame)` and `leave(peerId)`, and stores what the room reports:

- `events.update`: a change to the shared document, to append;
- `events.operation`: an operation the room ordered, to append;
- `events.change`: the header changed;
- `events.saveClaim`: a client says it saved the room as a version.

**A save claim is not trusted.** The host checks that the version is the file's current one and was
written by that client's user, then calls `markSaved`, which moves the base version, recomputes
`dirty` (from the state vector the client wrote, or from the sequence) and tells every client.
`snapshot()` gives the state to store; `historyLimit` bounds the operations kept before the base
version to place a late one.

A read-only peer (`canWrite: false`) receives everything and has its writes refused.

## The session

`new CollabSession({ kind, address, user, doc?, openSocket?, reconnectDelays? })`, then `connect()`.
`address` is asked again before every connection, so a host can hand out a ticket good for one
connection. The session reconnects on its own (0.5 s, 1 s, 2 s, 5 s, then every 10 s by default)
until `destroy()`, and on a reconnection the room sends only what it missed. Its status is
`connecting`, `connected`, `disconnected` or `closed`; `collabPeers(session.awareness)` lists who
else is here with their name and colour.

To save, the page writes the file through its host, then calls `claimSaved(versionId, { stateVector
})` for a shared document or `claimSaved(versionId, { sequence })` for a spreadsheet.

## Documents

`DocumentEditor` takes `collaboration: { doc, extensions, slot, seed, onSeeded }`. The text lives
in the Yjs fragment `content` (`DOCUMENT_CONTENT_FIELD`), bound by the TipTap collaboration
extensions, which also draw the others' carets. What is not text, the comments, notes, media,
headers and footers, page and sections, lives beside it in `SharedDocumentAside`: comments, notes
and media are maps by index, and the rest is replaced whole, the last writer winning.

Two clients adding a comment at the same moment never pick the same index: `slotIndex(length,
slot)` gives the first index at or past the end that belongs to the client's slot, out of
`MAX_SLOTS` (64). Past 64 clients slots repeat; a gap is filled with a placeholder.

## Presentations

`SlideEditor` takes `collaboration: { shared, seed, onSeeded }`, where `shared` is a
`SharedPresentation` of the session's document. Slides and shapes are kept by key, not by index,
so a shape another client added does not shift the ones after it; an animation names its shape by
key for the same reason. `matchKeys` tells, after an edit, which items kept their key and which
changed. A slide's background, notes, hidden flag and transition are replaced whole. Undo and
redo take back this client's changes only.

## Plain text

`TextEditor` takes `collaboration: { text }`, a `Y.Text`: the file's own text is ignored, and what
the others type appears as they type it, the caret moved through their changes (`shiftOffset`).
`readTextFile` and `writeTextFile` keep the file's encoding (UTF-8, UTF-16 LE or BE,
Windows-1252), its byte order mark and its dominant line ending across a save.

## Spreadsheets

`new SheetCollaboration(session, sheet, { onReload })`, with `sheet` freshly opened from the base
version of the welcome; the grid is given `collaboration.document` instead of the workbook, the
same calls with the edits shared, and the binding itself as `SheetGrid`'s `collaboration` prop.

- Every edit of this client is applied at once, sent as `{ id, base, operation }` and kept until the
  room gives it its turn. An operation is a call of the workbook (`SHEET_MUTATORS`) with its
  arguments and the locale it was typed in.
- The room moves each operation past those it ordered before it with `transformSheetOperation`,
  the same function every client runs: a row inserted above moves a cell down, a deleted range
  drops what was inside it (`shiftIndex`, `shiftSpan`).
- The formulas an operation carries move with it, as the workbook moves the formulas it holds:
  cell inputs, a region put back by undo, the rules of conditional formats and validations (their
  ranges too) and defined names. A reference to inserted or deleted rows or columns shifts, a
  range grows or shrinks, one whose cells were all deleted becomes `#REF!`, as does one pushed past
  the last row or column; a renamed sheet is written with its new name, quoted when it must be, and
  a removed one becomes `#REF!` (`shiftFormula`, `renameFormulaSheet`, `removeFormulaSheet` in
  `formula-references.ts`). Formulas are read as the engine reads them, in either locale: texts,
  function names such as `LOG10(`, defined names, tables and external references are left alone.
- Unqualified references belong to the sheet the formula is on (the first sheet for a workbook
  name, which the workbook writes there); a qualified one is matched by name, so an insertion,
  deletion, rename or removal carries the name its sheet had when it was made (`sheetName`, set by
  the binding for `SHEET_NAMED_MUTATORS`). An operation without it moves only unqualified
  references.
- When the others' operations arrive, the workbook returns to the last state the room agreed
  (`checkpoint` / `rollback` of the `SheetDocument`), applies them, and puts this client's waiting
  edits back on top.
- Undo and redo take back this client's own edits, up to `MAX_SHARED_UNDO` (100). The workbook's
  own history is turned off (`setHistoryEnabled(false)`), since a snapshot of the whole workbook
  would also undo what the others wrote; instead, before each edit, the binding keeps the
  rectangle it touches (`region` / `restoreRegion`, at most 200,000 cells, `MAX_REGION_CELLS`) and
  undoes by putting it back as a new operation. An edit larger than that cannot be undone.
- `whenIdle()` resolves once every waiting edit has its turn, so a save writes what the room has
  and nothing it does not.
- `setSelection` and `remoteSelections` share where each client's selection is; the grid draws
  the others' in their colour.
- `onReload` is called when the workbook can no longer follow the room and the file must be
  opened again.

## Not done

- A room lives in one process: the host routes every connection of a file to it.
- Chart ranges are not moved, by the transform or by the workbook, when rows or columns are
  inserted or deleted or a sheet is renamed: a chart keeps the ranges it was given.
- `SlideEditor` shares no presence: the others' selections are not drawn on a slide.

## Testing

`test/collab.test.ts` runs rooms and sessions over an in-memory network (`test/collab-network.ts`):
seeding and its hand-over, readers refused, `dirty` and save claims, concurrent and late sheet
operations, undo of one's own edits, reconnection, and formulas moved past structure, each checked
against what the workbook does to the same formula already in place. `test/collab-editors.test.tsx` runs two
document editors, two grids and two slide editors against one room; the shared `TextEditor` is
tested in `src/text/text-editor.test.tsx`. Regions and checkpoints are tested on the Rust side in
`crates/rely-office-sheet-wasm/src/tests.rs`.
