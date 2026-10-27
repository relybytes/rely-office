// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// Smoke test of the built package, with no dependency: it writes a docx in memory as a stored
// zip and reads it back through every export.
//
// Usage: node packages/rely-office-node/test/smoke.cjs [path to the package or to a .tgz-extracted
// folder]; by default the package folder next to this test.

'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');

const packagePath = path.resolve(process.argv[2] ?? path.join(__dirname, '..'));
const office = require(packagePath);

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
    assert.equal(sheet.input(0, 3, 0), '=SUM(A2:A3)');
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

test('survives many calls without leaking handles', () => {
  for (let round = 0; round < 200; round += 1) {
    office.readDocumentTree(docx);
  }
});

console.log('smoke test passed');
