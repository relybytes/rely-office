// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

// Smoke test of the built package, with no dependency: it writes a docx in memory as a stored
// zip and reads it back through every export.
//
// Usage: node packages/rely-office-node/test/smoke.cjs [path to the package or to a .tgz-extracted
// folder]; by default the package folder next to this test.

'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const packagePath = path.resolve(process.argv[2] ?? path.join(__dirname, '..'));
const office = require(packagePath);

/** The odp the odp test builds, edited again by the presentation test. */
let odp = null;

const PNG = Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13]);

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

/** A zip of stored (uncompressed) members: `[name, Uint8Array | string]`. */
function storedZip(members) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const [name, content] of members) {
    const nameBytes = Buffer.from(name, 'utf8');
    const data = typeof content === 'string' ? Buffer.from(content, 'utf8') : Buffer.from(content);
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE(offset, 42);
    locals.push(local, nameBytes, data);
    centrals.push(central, nameBytes);
    offset += local.length + nameBytes.length + data.length;
  }
  const centralDirectory = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(members.length, 8);
  end.writeUInt16LE(members.length, 10);
  end.writeUInt32LE(centralDirectory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, centralDirectory, end]);
}

const W = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';
const R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const docx = storedZip([
  [
    '[Content_Types].xml',
    '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>',
  ],
  [
    'word/document.xml',
    `<w:document xmlns:w="${W}" xmlns:r="${R}" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>` +
      '<w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t>Contratto di servizio</w:t></w:r></w:p>' +
      '<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Importante</w:t></w:r>' +
      '<w:hyperlink r:id="rId2"><w:r><w:t xml:space="preserve"> sito</w:t></w:r></w:hyperlink>' +
      '<w:hyperlink r:id="rId3"><w:r><w:t xml:space="preserve"> script</w:t></w:r></w:hyperlink>' +
      '<w:r><w:drawing><wp:inline><wp:extent cx="952500" cy="476250"/><wp:docPr id="1" name="a" descr="Logo"/>' +
      '<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:blipFill>' +
      '<a:blip r:embed="rId1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>' +
      '</w:body></w:document>',
  ],
  [
    'word/_rels/document.xml.rels',
    `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">` +
      `<Relationship Id="rId1" Type="${R}/image" Target="media/logo.png"/>` +
      `<Relationship Id="rId2" Type="${R}/hyperlink" Target="https://relybytes.com" TargetMode="External"/>` +
      `<Relationship Id="rId3" Type="${R}/hyperlink" Target="javascript:alert(1)" TargetMode="External"/>` +
      '</Relationships>',
  ],
  ['word/media/logo.png', PNG],
]);

function test(name, body) {
  try {
    body();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

test('names its version', () => {
  assert.match(office.version, /^\d+\.\d+\.\d+/);
  assert.equal(office.version, require(path.join(packagePath, 'package.json')).version);
});

test('detects formats from the bytes', () => {
  assert.equal(office.detectFormat(docx, 'contratto.bin'), 'docx');
  assert.equal(office.detectFormat(Buffer.from('a;b\n1;2\n'), 'dati.csv'), 'csv');
  assert.equal(office.detectFormat(new Uint8Array([1, 2, 3])), 'unknown');
  assert.equal(office.detectFormat(docx.buffer.slice(docx.byteOffset, docx.byteOffset + docx.length)), 'docx');
});

test('extracts text and reports truncation', () => {
  const extracted = office.extractText(docx, 'contratto.docx');
  assert.equal(extracted.format, 'docx');
  assert.equal(extracted.truncated, false);
  assert.match(extracted.text, /^Contratto di servizio\nImportante sito script\n$/);
  const cut = office.extractText(docx, undefined, { maxTextChars: 9 });
  assert.deepEqual(cut, { format: 'docx', text: 'Contratto', truncated: true });
});

test('reads sheets', () => {
  const workbook = office.readSheets(Buffer.from('Nome;Città\nMario;Arona\n'), 'clienti.csv');
  assert.equal(workbook.format, 'csv');
  assert.deepEqual(workbook.sheets[0].rows, [
    ['Nome', 'Città'],
    ['Mario', 'Arona'],
  ]);
  assert.equal(workbook.sheets[0].totalRows, 2);
});

test('reads a document tree with its images', () => {
  const tree = office.readDocumentTree(docx);
  assert.deepEqual(tree.blocks[0], {
    type: 'heading',
    level: 1,
    content: [{ type: 'text', text: 'Contratto di servizio' }],
  });
  assert.deepEqual(tree.blocks[1].content, [
    { type: 'text', text: 'Importante', marks: [{ type: 'bold' }] },
    { type: 'text', text: ' sito', marks: [{ type: 'link', href: 'https://relybytes.com' }] },
    { type: 'text', text: ' script' },
    { type: 'image', media: 0, alt: 'Logo', width: 100, height: 50 },
  ]);
  assert.equal(tree.media.length, 1);
  assert.equal(tree.media[0].index, 0);
  assert.equal(tree.media[0].contentType, 'image/png');
  assert.ok(tree.media[0].bytes instanceof Uint8Array);
  assert.deepEqual(Array.from(tree.media[0].bytes), Array.from(PNG));
  assert.equal(tree.report.unsupportedLinks, 1);
  assert.equal(tree.report.truncated, false);
});

test('reads an odt as a document tree and draws an odp as slides', () => {
  const namespaces =
    'xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" ' +
    'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" ' +
    'xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" ' +
    'xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0"';
  const odt = storedZip([
    ['mimetype', 'application/vnd.oasis.opendocument.text'],
    [
      'content.xml',
      `<office:document-content ${namespaces}><office:body><office:text>` +
        '<text:h text:outline-level="1">Verbale</text:h><text:p>Presenti tutti.</text:p>' +
        '</office:text></office:body></office:document-content>',
    ],
  ]);
  const tree = office.readDocumentTree(odt);
  assert.deepEqual(tree.blocks[0], {
    type: 'heading',
    level: 1,
    content: [{ type: 'text', text: 'Verbale' }],
  });
  assert.deepEqual(tree.blocks[1].content, [{ type: 'text', text: 'Presenti tutti.' }]);

  odp = storedZip([
    ['mimetype', 'application/vnd.oasis.opendocument.presentation'],
    [
      'content.xml',
      `<office:document-content ${namespaces}><office:body><office:presentation>` +
        '<draw:page><draw:frame svg:x="1cm" svg:y="1cm" svg:width="10cm" svg:height="2cm">' +
        '<draw:text-box><text:p>Ordine del giorno</text:p></draw:text-box></draw:frame></draw:page>' +
        '</office:presentation></office:body></office:document-content>',
    ],
  ]);
  const rendered = office.render(odp, 'riunione.odp', { pdf: false, thumbnailWidth: 200 });
  assert.equal(rendered.fidelity, 'layout');
  assert.equal(rendered.pageCount, 1);
});

test('converts a document to docx and a presentation to pptx', () => {
  const converted = office.convertToDocx(docx);
  assert.equal(office.detectFormat(converted), 'docx');
  assert.deepEqual(office.readDocumentTree(converted).blocks, office.readDocumentTree(docx).blocks);
  const namespaces =
    'xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" ' +
    'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" ' +
    'xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" ' +
    'xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0"';
  odp = storedZip([
    ['mimetype', 'application/vnd.oasis.opendocument.presentation'],
    [
      'content.xml',
      `<office:document-content ${namespaces}><office:body><office:presentation>` +
        '<draw:page><draw:frame svg:x="1cm" svg:y="1cm" svg:width="10cm" svg:height="2cm">' +
        '<draw:text-box><text:p>Ordine del giorno</text:p></draw:text-box></draw:frame></draw:page>' +
        '</office:presentation></office:body></office:document-content>',
    ],
  ]);
  const pptx = office.convertToPptx(odp);
  assert.equal(office.detectFormat(pptx), 'pptx');
  assert.match(office.extractText(pptx).text, /Ordine del giorno/);
  assert.throws(() => office.convertToPptx(docx), (error) => error.code === 'UNSUPPORTED_FORMAT');
});

test('edits a presentation and writes it back', () => {
  const presentation = office.readPresentation(office.convertToPptx(odp));
  assert.equal(presentation.slides.length, 1);
  const slide = presentation.slides[0];
  const box = slide.shapes.find((shape) => shape.content.type === 'text');
  box.content.paragraphs[0].runs[0].text = 'Ordine del giorno rivisto';
  slide.notes = {
    paragraphs: [{ ...box.content.paragraphs[0], runs: [{ ...box.content.paragraphs[0].runs[0], text: 'Note' }] }],
    anchor: 'top',
    insetsEmu: [0, 0, 0, 0],
    wrap: true,
  };
  slide.transition = { effect: 'fade', advanceOnClick: true };
  slide.animations = [
    {
      shape: slide.shapes.indexOf(box),
      class: 'entrance',
      preset: 10,
      presetSubtype: 0,
      trigger: 'onClick',
      delayMs: 0,
      durationMs: 500,
    },
  ];
  presentation.slides.push({ shapes: [], hidden: true });
  const written = office.writePptx(presentation);
  const read = office.readPresentation(written);
  assert.equal(read.slides.length, 2);
  assert.match(office.extractText(written).text, /Ordine del giorno rivisto/);
  assert.equal(read.slides[0].transition.effect, 'fade');
  assert.equal(read.slides[0].animations.length, 1);
  assert.equal(read.slides[0].notes.paragraphs[0].runs[0].text, 'Note');
  assert.equal(read.slides[1].hidden, true);
  assert.throws(() => office.writePptx({ slides: [] }), (error) => error.code === 'INVALID_PRESENTATION');
});

test('throws coded errors', () => {
  assert.throws(
    () => office.extractText(Buffer.from('testo'), 'nota.txt'),
    (error) =>
      error instanceof office.RelyOfficeError &&
      error.code === 'UNSUPPORTED_FORMAT' &&
      error.name === 'RelyOfficeError',
  );
  assert.throws(
    () => office.readDocumentTree(docx, { maxInputBytes: 10 }),
    (error) => error.code === 'TOO_LARGE',
  );
  assert.throws(
    () => office.extractText(docx, undefined, { maxRowz: 1 }),
    (error) => error.code === 'INVALID_LIMITS',
  );
  assert.throws(
    () => office.extractText(docx, undefined, { maxTextChars: -1 }),
    (error) => error.code === 'INVALID_LIMITS',
  );
  assert.throws(() => office.extractText('not bytes'), TypeError);
  assert.throws(
    () => office.readSheets(Buffer.from('PK\u0003\u0004broken'), 'a.xlsx'),
    (error) => error instanceof office.RelyOfficeError,
  );
});

test('renders a document to PDF and PNG', () => {
  const rendered = office.render(docx, 'contratto.docx', {
    thumbnailWidth: 160,
    pageImages: { pixelsPerPoint: 0.5, maxPages: 1 },
  });
  assert.equal(rendered.format, 'docx');
  assert.equal(rendered.fidelity, 'layout');
  assert.equal(rendered.pageCount, 1);
  assert.equal(rendered.truncated, false);
  assert.equal(Buffer.from(rendered.pdf.subarray(0, 5)).toString('latin1'), '%PDF-');
  assert.equal(Buffer.from(rendered.thumbnail).readUInt32BE(16), 160);
  assert.equal(rendered.pageImages.length, 1);
  const sheet = office.render(Buffer.from('Nome;Importo\nMario;10\n'), 'clienti.csv', {
    pdf: false,
    limits: { maxRows: 1 },
  });
  assert.equal(sheet.pdf, null);
  assert.equal(sheet.thumbnail, null);
  assert.equal(sheet.truncated, true);
  assert.throws(
    () => office.render(docx, undefined, { colour: 'red' }),
    (error) => error.code === 'INVALID_OPTIONS',
  );
  assert.throws(
    () => office.render(Buffer.from('testo'), 'nota.txt'),
    (error) => error.code === 'UNSUPPORTED_FORMAT',
  );
});

test('writes a docx it reads back', () => {
  const tree = office.readDocumentTree(docx);
  const written = office.writeDocx({
    blocks: tree.blocks,
    media: tree.media.map((item) => item.bytes),
  });
  const read = office.readDocumentTree(written);
  assert.deepEqual(read.blocks, tree.blocks);
  assert.equal(read.media.length, 1);
  assert.throws(
    () => office.writeDocx({ blocks: [{ type: 'nope' }] }),
    (error) => error.code === 'INVALID_TREE',
  );
});

test('writes the fonts of a document and embeds their faces', () => {
  const carlito = fs.readFileSync(path.join(packagePath, 'fonts', 'Carlito-Regular.ttf'));
  const written = office.writeDocx({
    blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'x', marks: [{ type: 'textStyle', fontFamily: 'Consolas' }] }] }],
    fonts: [{ name: 'Carlito', faces: [{ style: 'regular', file: 0 }] }],
    fontFiles: [carlito],
    defaultFont: { family: 'Carlito', sizeHalfPoints: 24 },
  });
  const tree = office.readDocumentTree(written);
  assert.deepEqual(tree.defaultFont, { family: 'Carlito', sizeHalfPoints: 24 });
  assert.equal(Buffer.compare(Buffer.from(tree.fontFiles[0]), carlito), 0);
  assert.equal(tree.fonts.find((font) => font.name === 'Consolas').generic, 'modern');
  assert.equal(office.fontStack('Calibri'), "'Calibri', 'Carlito', sans-serif");
  assert.equal(office.fontSubstitute('Mystery', 'roman'), 'Liberation Serif');
});

test('writes an embedded object it reads back', () => {
  const picture = office.readDocumentTree(docx).media[0].bytes;
  const workbook = Uint8Array.from([0x50, 0x4b, 0x03, 0x04, 1, 2, 3]);
  const written = office.writeDocx({
    blocks: [{ type: 'paragraph', content: [{ type: 'image', media: 0, width: 80, height: 40 }] }],
    media: [picture],
    objects: [{ media: 0, name: 'Foglio.xlsx', program: 'Excel.Sheet.12', bytes: workbook }],
  });
  const read = office.readDocumentTree(written);
  assert.equal(read.objects.length, 1);
  assert.equal(read.objects[0].media, read.blocks[0].content[0].media);
  assert.equal(read.objects[0].program, 'Excel.Sheet.12');
  assert.equal(read.objects[0].package, true);
  assert.deepEqual([...read.objects[0].bytes], [...workbook]);
  assert.equal(office.readDocumentTree(docx).objects, undefined);
});

test('writes comments and tracked changes it reads back', () => {
  const text = (words, marks = []) => ({ type: 'text', text: words, marks });
  const written = office.writeDocx({
    blocks: [
      {
        type: 'paragraph',
        content: [
          text('Visto ', [{ type: 'comment', comment: 0 }]),
          text('aggiunto', [{ type: 'insertion', author: 'Revisore' }]),
          text('tolto', [{ type: 'deletion', author: 'Revisore' }]),
        ],
      },
    ],
    comments: [{ author: 'Anna', blocks: [{ type: 'paragraph', content: [text('Ok')] }], resolved: true }],
  });
  const kept = office.readDocumentTree(written, undefined, { trackedChanges: 'keep' });
  assert.equal(kept.comments[0].author, 'Anna');
  assert.equal(kept.comments[0].resolved, true);
  const marks = kept.blocks[0].content.map((inline) => (inline.marks ?? []).map((mark) => mark.type));
  assert.deepEqual(marks, [['comment'], ['insertion'], ['deletion']]);
  const accepted = office.readDocumentTree(written);
  assert.equal(accepted.blocks[0].content.map((inline) => inline.text).join(''), 'Visto aggiunto');
  assert.throws(
    () => office.readDocumentTree(written, undefined, { trackedChanges: 'maybe' }),
    (error) => error.code === 'INVALID_OPTIONS',
  );
});

test('edits a spreadsheet and saves it as xlsx', () => {
  const sheet = office.createSheet({ locale: 'it' });
  try {
    sheet.setInputs(0, [
      [0, 0, '1,5'],
      [1, 0, '2'],
      [2, 0, '=SOMMA(A1:A2)'],
    ]);
    sheet.setFormat(0, 2, 0, { type: 'decimals', decimals: 2 });
    assert.equal(sheet.cells(0, { top: 2, left: 0, bottom: 2, right: 0 })[0].display, '3,50');
    sheet.insertRows(0, 0);
    assert.equal(sheet.input(0, 3, 0), '=SOMMA(A2:A3)');
    assert.equal(sheet.input(0, 1, 0), '1,5');
    assert.equal(sheet.undo(), true);
    assert.equal(sheet.input(0, 2, 0), '=SOMMA(A1:A2)');
    assert.equal(sheet.canRedo(), true);
    assert.equal(sheet.redo(), true);
    assert.equal(sheet.input(0, 3, 0), '=SOMMA(A2:A3)');
    assert.equal(sheet.addSheet('Riepilogo'), 1);
    sheet.setInput(1, 0, 0, '=Foglio1!A4*2');
    sheet.setToday(new Date(2026, 9, 27));
    sheet.setInput(1, 1, 0, '=OGGI()');
    assert.deepEqual(
      sheet.cells(1, { top: 0, left: 0, bottom: 1, right: 0 }).map((cell) => cell.display),
      ['7', '46322'],
    );
    assert.throws(() => sheet.setInput(0, 0, 0, '=SOMMA('), (error) => error.code === 'INVALID_FORMULA');
    assert.equal(sheet.previewInput('1.234,5'), 'number');
    const saved = sheet.save();
    const opened = office.openSheet(saved, 'conti.xlsx', { locale: 'en' });
    try {
      assert.deepEqual(opened.sheets().map((summary) => summary.name), ['Foglio1', 'Riepilogo']);
      assert.equal(opened.cells(1, { top: 0, left: 0, bottom: 0, right: 0 })[0].display, '7');
    } finally {
      opened.free();
    }
    assert.equal(office.readSheets(saved, 'conti.xlsx').sheets[1].rows[0][0], '7');
  } finally {
    sheet.free();
  }
});

test('styles, merges and sizes the cells of a spreadsheet', () => {
  const sheet = office.createSheet({ locale: 'it' });
  try {
    const title = { top: 0, left: 0, bottom: 0, right: 2 };
    sheet.setInput(0, 0, 0, 'Preventivo');
    sheet.setStyle(0, title, { bold: true, fill: '#1F4E79', fontColour: 'FFFFFF', horizontal: 'center' });
    sheet.setBorders(0, title, 'outline', { style: 'single', widthEighths: 4, colour: 'FF0000' });
    sheet.mergeCells(0, title);
    sheet.setRowHeight(0, 0, 30);
    sheet.setInput(0, 1, 0, '1234,5');
    sheet.setFormat(0, 1, 0, { type: 'code', code: '#,##0.00 "€"' });
    const cells = sheet.cells(0, { top: 0, left: 0, bottom: 1, right: 2 });
    assert.equal(cells.length, 4);
    assert.equal(cells[3].display, '1.234,50 €');
    const look = sheet.styles()[cells[0].style];
    assert.equal(look.font.bold, true);
    assert.equal(look.fill, '1F4E79');
    assert.deepEqual(sheet.merges(0), [{ firstRow: 0, firstColumn: 0, lastRow: 0, lastColumn: 2 }]);
    assert.deepEqual(sheet.sheets()[0].rowHeights, [[0, 30]]);
    assert.throws(() => sheet.setStyle(0, title, { fill: 'giallo' }), (error) => error.code === 'INVALID_STYLE');
    const opened = office.openSheet(sheet.save(), 'preventivo.xlsx', { locale: 'it' });
    try {
      const reopened = opened.cells(0, { top: 0, left: 0, bottom: 1, right: 0 });
      assert.equal(opened.styles()[reopened[0].style].fill, '1F4E79');
      assert.equal(reopened[1].display, '1.234,50 €');
      assert.equal(opened.merges(0).length, 1);
      opened.unmergeCells(0, title);
      opened.clearStyles(0, title);
      assert.equal(opened.merges(0).length, 0);
      assert.equal(opened.cells(0, title).length, 1);
    } finally {
      opened.free();
    }
  } finally {
    sheet.free();
  }
});

test('keeps the conditional formats and validations of a sheet, in xlsx and ods', () => {
  const sheet = office.createSheet({ locale: 'en' });
  try {
    sheet.setInputs(0, [
      [0, 0, '5'],
      [1, 0, '50'],
      [0, 1, 'maybe'],
    ]);
    const column = { firstRow: 0, firstColumn: 0, lastRow: 1, lastColumn: 0 };
    sheet.setConditionalFormats(0, [
      {
        ranges: [column],
        rules: [
          { condition: { kind: 'cellIs', operator: 'greater', formulas: ['10'] }, look: { fill: 'FFC7CE', bold: true }, priority: 1 },
          { condition: { kind: 'dataBar', min: { kind: 'min' }, max: { kind: 'max' }, colour: '638EC6', showValue: true }, priority: 2 },
        ],
      },
    ]);
    sheet.setValidations(0, [
      { ranges: [{ firstRow: 0, firstColumn: 1, lastRow: 9, lastColumn: 1 }], kind: 'list', formulas: ['"yes,no"'], dropdown: true, errorStyle: 'warning' },
    ]);
    const looks = sheet.conditionalLooks(0);
    assert.equal(looks[1].fill, 'FFC7CE');
    assert.equal(looks[1].bar.length, 0.9);
    assert.deepEqual(sheet.validation(0, 0, 1), {
      validation: sheet.validations(0)[0],
      valid: false,
      items: ['yes', 'no'],
    });
    assert.deepEqual(sheet.invalidCells(0), [[0, 1]]);
    assert.throws(
      () => sheet.setConditionalFormats(0, [{ ranges: [column], rules: [{ condition: { kind: 'expression', formula: '=(' } }] }]),
      (error) => error.code === 'INVALID_FORMULA',
    );
    for (const [bytes, name] of [
      [sheet.save(), 'regole.xlsx'],
      [sheet.saveOds(), 'regole.ods'],
    ]) {
      const opened = office.openSheet(bytes, name, { locale: 'en' });
      try {
        assert.deepEqual(opened.conditionalFormats(0), sheet.conditionalFormats(0), name);
        assert.deepEqual(opened.validations(0), sheet.validations(0), name);
        assert.equal(opened.sheets()[0].pivotTables, 0);
      } finally {
        opened.free();
      }
    }
    assert.equal(Buffer.from(sheet.saveOds().subarray(30, 38)).toString(), 'mimetype');
  } finally {
    sheet.free();
  }
});

test('fills, sorts, filters and freezes a sheet, and keeps the filter and the panes in xlsx', () => {
  const sheet = office.createSheet({ locale: 'it' });
  try {
    sheet.setInputs(0, [
      [0, 0, 'Zona'],
      [0, 1, 'Importo'],
      [1, 0, 'Sud'],
      [1, 1, '1'],
      [2, 0, 'Nord'],
      [2, 1, '2'],
    ]);
    sheet.fillRange(0, { top: 1, left: 1, bottom: 2, right: 1 }, { top: 1, left: 1, bottom: 4, right: 1 });
    assert.equal(sheet.input(0, 4, 1), '4');
    sheet.sortRange(0, { top: 0, left: 0, bottom: 2, right: 1 }, [{ column: 0 }], true);
    assert.equal(sheet.input(0, 1, 0), 'Nord');
    sheet.setAutoFilter(0, { top: 0, left: 0, bottom: 2, right: 1 });
    assert.deepEqual(sheet.filterChoices(0, 0).map((choice) => choice.key), ['Nord', 'Sud']);
    sheet.setFilterColumn(0, 0, ['Sud']);
    sheet.setFrozen(0, 1, 0);
    assert.deepEqual(sheet.sheets()[0].autoFilter.hiddenRows, [1]);
    assert.ok(sheet.functionNames().includes('SOMMA'));
    assert.deepEqual(sheet.pictures(0), []);

    const opened = office.openSheet(sheet.save(), 'filtro.xlsx', { locale: 'it' });
    try {
      const summary = opened.sheets()[0];
      assert.equal(summary.frozenRows, 1);
      assert.deepEqual(summary.autoFilter.columns, [{ column: 0, shown: ['Sud'] }]);
    } finally {
      opened.free();
    }
    sheet.setAutoFilter(0, null);
    assert.equal(sheet.sheets()[0].autoFilter, null);
  } finally {
    sheet.free();
  }
});

test('survives many calls without leaking handles', () => {
  for (let round = 0; round < 200; round += 1) {
    office.readDocumentTree(docx);
  }
});

console.log('smoke test passed');
