// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// Documents and presentations in the browser: the reading and writing module of rely-office,
// built for the web. `initDocuments` loads it once; every other call needs it loaded.

import init, {
  DocxWriter,
  PptxWriter,
  convertToDocx as wasmConvertToDocx,
  convertToPptx as wasmConvertToPptx,
  detectFormat as wasmDetectFormat,
  extractTextJson,
  readDocumentTreeHandle,
  readPresentationHandle,
  readSheetsJson,
  version as wasmVersion,
} from '../../wasm/rely_office_wasm.js';
import type {
  DocumentBytes,
  DocumentFormat,
  DocumentTree,
  DocxInput,
  EmbeddedObject,
  ExtractedText,
  Limits,
  MediaItem,
  Presentation,
  PptxInput,
  TreeOptions,
  Workbook,
} from '../model.js';
import { call, toBytes, toFileName, toLimitsJson, type WasmSource } from './shared.js';

let loading: Promise<void> | null = null;
let loaded = false;

/**
 * Load the module once. By default it fetches the .wasm next to the module, which bundlers
 * resolve as an asset; pass a URL, a Response, the bytes or a compiled module otherwise.
 */
export function initDocuments(wasm?: WasmSource): Promise<void> {
  if (loading === null) {
    loading = init(wasm === undefined ? undefined : { module_or_path: wasm }).then(() => {
      loaded = true;
    });
    loading.catch(() => {
      loading = null;
    });
  }
  return loading;
}

function ready(): void {
  if (!loaded) {
    throw new Error('rely-office: call initDocuments() before reading or writing a document');
  }
}

/** The format of a document; the name only tells CSV from TSV and never overrides the bytes. */
export function detectFormat(bytes: DocumentBytes, fileName?: string): DocumentFormat {
  ready();
  return wasmDetectFormat(toBytes(bytes), toFileName(fileName)) as DocumentFormat;
}

/** The text of a document, for a search or a summary. */
export function extractText(bytes: DocumentBytes, fileName?: string, limits?: Limits): ExtractedText {
  ready();
  return JSON.parse(
    call(() => extractTextJson(toBytes(bytes), toFileName(fileName), toLimitsJson(limits))),
  ) as ExtractedText;
}

/** The sheets of a spreadsheet as tables of displayed values. */
export function readSheets(bytes: DocumentBytes, fileName?: string, limits?: Limits): Workbook {
  ready();
  return JSON.parse(
    call(() => readSheetsJson(toBytes(bytes), toFileName(fileName), toLimitsJson(limits))),
  ) as Workbook;
}

/** A docx, doc or odt as the neutral document tree, with the bytes of its images and objects. */
export function readDocumentTree(
  bytes: DocumentBytes,
  limits?: Limits,
  options?: TreeOptions,
  fileName?: string,
): DocumentTree {
  ready();
  const handle = call(() =>
    readDocumentTreeHandle(
      toBytes(bytes),
      toLimitsJson(limits),
      options === undefined ? undefined : JSON.stringify(options),
      fileName,
    ),
  );
  try {
    const tree = JSON.parse(handle.json) as Omit<DocumentTree, 'media' | 'objects'> & {
      media: { index: number; contentType: string }[];
      objects?: (Omit<EmbeddedObject, 'bytes'> & { index: number })[];
    };
    const media: MediaItem[] = tree.media.map((item) => ({
      index: item.index,
      contentType: item.contentType,
      bytes: handle.takeMediaBytes(item.index) ?? new Uint8Array(0),
    }));
    const objects: EmbeddedObject[] | undefined = tree.objects?.map((item) => ({
      media: item.media,
      program: item.program,
      name: item.name,
      package: item.package,
      bytes: handle.takeObjectBytes(item.index) ?? new Uint8Array(0),
    }));
    const { objects: _objects, ...rest } = tree;
    return objects === undefined ? { ...rest, media } : { ...rest, media, objects };
  } finally {
    handle.free();
  }
}

/** A document tree as a docx Word opens and `readDocumentTree` reads back to the same tree. */
export function writeDocx(tree: DocxInput): Uint8Array {
  ready();
  const { blocks, page, media, notes, headersFooters, sections, comments, objects } = tree;
  const writer = new DocxWriter();
  try {
    for (const item of media ?? []) {
      writer.addMedia(toBytes(item));
    }
    for (const object of objects ?? []) {
      writer.addObject(
        object.media,
        toBytes(object.bytes),
        object.name ?? 'oleObject.bin',
        object.program,
      );
    }
    return call(() =>
      writer.write(
        JSON.stringify({ blocks: blocks ?? [], page, notes, headersFooters, sections, comments }),
      ),
    );
  } finally {
    writer.free();
  }
}

/** The tree in a writer with its images added, for `write` to turn into a file. */
function withTreeWriter<T>(tree: DocxInput, write: (writer: DocxWriter, json: string) => T): T {
  ready();
  const { blocks, page, media, notes, headersFooters, sections, comments } = tree;
  const writer = new DocxWriter();
  try {
    for (const item of media ?? []) {
      writer.addMedia(toBytes(item));
    }
    const json = JSON.stringify({ blocks: blocks ?? [], page, notes, headersFooters, sections, comments });
    return call(() => write(writer, json));
  } finally {
    writer.free();
  }
}

/** The tree as an odt, `OpenDocument` text; an equation is written as its LaTeX. */
export function writeOdt(tree: DocxInput): Uint8Array {
  return withTreeWriter(tree, (writer, json) => writer.writeOdt(json));
}

/** The tree as Markdown with GitHub's tables, footnotes and math, its images in `data:` addresses. */
export function writeMarkdown(tree: DocxInput): string {
  return withTreeWriter(tree, (writer, json) => writer.writeMarkdown(json));
}

/** The tree as a page of HTML that stands alone, its images in `data:` addresses. */
export function writeHtml(tree: DocxInput, title = ''): string {
  return withTreeWriter(tree, (writer, json) => writer.writeHtml(json, title));
}

/** A doc, an odt or a docx as a docx. */
export function convertToDocx(bytes: DocumentBytes, limits?: Limits): Uint8Array {
  ready();
  return call(() => wasmConvertToDocx(toBytes(bytes), toLimitsJson(limits)));
}

/** A ppt, an odp or a pptx as a pptx. */
export function convertToPptx(bytes: DocumentBytes, limits?: Limits): Uint8Array {
  ready();
  return call(() => wasmConvertToPptx(toBytes(bytes), toLimitsJson(limits)));
}

/** A pptx, an odp or a ppt as slides for an editor, with the bytes of its images. */
export function readPresentation(bytes: DocumentBytes, limits?: Limits): Presentation {
  ready();
  const handle = call(() => readPresentationHandle(toBytes(bytes), toLimitsJson(limits)));
  try {
    const presentation = JSON.parse(handle.json) as Omit<Presentation, 'media'> & {
      media: { index: number; contentType: string }[];
    };
    return {
      ...presentation,
      media: presentation.media.map((item) => ({
        index: item.index,
        contentType: item.contentType,
        bytes: handle.takeMediaBytes(item.index) ?? new Uint8Array(0),
      })),
    };
  } finally {
    handle.free();
  }
}

/** Slides as a pptx PowerPoint opens and `readPresentation` reads back to the same slides. */
function writeSlides(presentation: PptxInput, write: (writer: PptxWriter, json: string) => Uint8Array): Uint8Array {
  ready();
  const { widthEmu, heightEmu, slides, media } = presentation;
  const writer = new PptxWriter();
  try {
    for (const item of media ?? []) {
      writer.addMedia(toBytes(item instanceof Uint8Array ? item : 'bytes' in item ? item.bytes : item));
    }
    const json = JSON.stringify({ widthEmu, heightEmu, slides: slides ?? [] });
    return call(() => write(writer, json));
  } finally {
    writer.free();
  }
}

export function writePptx(presentation: PptxInput): Uint8Array {
  return writeSlides(presentation, (writer, json) => writer.write(json));
}

/** The slides as an odp, `OpenDocument` presentation. */
export function writeOdp(presentation: PptxInput): Uint8Array {
  return writeSlides(presentation, (writer, json) => writer.writeOdp(json));
}

/** The version of the library inside the module. */
export function documentsVersion(): string {
  ready();
  return wasmVersion();
}
