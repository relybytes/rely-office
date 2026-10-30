#!/usr/bin/env node
// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

// Build the comparison corpus and measure rely-office-render against LibreOffice.
//
// 1. LibreOffice writes every source of corpus/sources as an office document: HTML as docx,
//    CSV (semicolon separated, UTF-8) as xlsx, Flat ODP as pptx. The documents are LibreOffice's
//    own, with its styles, not hand-made XML.
// 2. LibreOffice prints each document to PDF, and the rasterizer turns each page into a grey PNG
//    at 72 dpi: these are the reference pages.
// 3. rely-office-corpus renders each document at 72 dpi and compares it page by page.
//
// Usage: node scripts/corpus.mjs [--min-pass-rate <share>] [--threshold <distance>]
// SOFFICE names LibreOffice (default soffice). RASTERIZER is pdftoppm (default) or gs, with
// GHOSTSCRIPT naming Ghostscript (default gs). Everything is written under target/corpus/.

import { execFileSync } from 'node:child_process';
import { mkdirSync, readdirSync, rmSync } from 'node:fs';
import { basename, extname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '..', '..');
const sources = join(root, 'corpus', 'sources');
const output = join(root, 'target', 'corpus');
const documents = join(output, 'documents');
const pdfs = join(output, 'pdf');
const references = join(output, 'references');
const soffice = process.env.SOFFICE ?? 'soffice';
const rasterizer = process.env.RASTERIZER ?? 'pdftoppm';
const ghostscript = process.env.GHOSTSCRIPT ?? 'gs';

function run(command, args) {
  console.log(`> ${basename(command)} ${args.join(' ')}`);
  execFileSync(command, args, { cwd: root, stdio: 'inherit' });
}

function libreoffice(args) {
  // A profile of its own, so a LibreOffice the user has open is neither used nor disturbed.
  const profile = pathToFileURL(join(output, 'profile')).href;
  run(soffice, [`-env:UserInstallation=${profile}`, '--headless', '--norestore', ...args]);
}

for (const directory of [documents, pdfs, references]) {
  rmSync(directory, { recursive: true, force: true });
  mkdirSync(directory, { recursive: true });
}

const files = readdirSync(sources).sort();
const html = files.filter((file) => extname(file) === '.html').map((file) => join(sources, file));
const csv = files.filter((file) => extname(file) === '.csv').map((file) => join(sources, file));
const decks = files.filter((file) => extname(file) === '.fodp').map((file) => join(sources, file));
if (html.length > 0) {
  libreoffice([
    '--infilter=HTML (StarWriter)',
    '--convert-to',
    'docx:MS Word 2007 XML',
    '--outdir',
    documents,
    ...html,
  ]);
}
if (csv.length > 0) {
  // Semicolon (59), double quote (34), UTF-8 (76), from the first line.
  libreoffice([
    '--infilter=CSV:59,34,76,1',
    '--convert-to',
    'xlsx:Calc MS Excel 2007 XML',
    '--outdir',
    documents,
    ...csv,
  ]);
}

if (decks.length > 0) {
  libreoffice(['--convert-to', 'pptx:Impress MS PowerPoint 2007 XML', '--outdir', documents, ...decks]);
}

const built = readdirSync(documents).sort().map((file) => join(documents, file));
libreoffice(['--convert-to', 'pdf', '--outdir', pdfs, ...built]);

for (const pdf of readdirSync(pdfs).sort()) {
  const stem = basename(pdf, '.pdf');
  if (rasterizer === 'gs') {
    run(ghostscript, [
      '-dSAFER',
      '-dBATCH',
      '-dNOPAUSE',
      '-dQUIET',
      '-sDEVICE=pnggray',
      '-r72',
      '-dTextAlphaBits=4',
      '-dGraphicsAlphaBits=4',
      `-sOutputFile=${join(references, `${stem}-%d.png`)}`,
      join(pdfs, pdf),
    ]);
  } else {
    run(rasterizer, ['-r', '72', '-gray', '-png', join(pdfs, pdf), join(references, stem)]);
  }
}

const passThrough = process.argv.slice(2);
run('cargo', [
  'run',
  '--quiet',
  '--release',
  '--package',
  'rely-office-corpus',
  '--',
  '--documents',
  documents,
  '--references',
  references,
  '--fonts',
  join(root, 'fonts'),
  '--report',
  join(output, 'report.json'),
  '--pages',
  join(output, 'ours'),
  ...passThrough,
]);
