#!/usr/bin/env node
// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

// Build the two npm packages. @relybytes/rely-office, for the backend: rely-office-wasm (reading
// and writing), rely-office-render-wasm (rendering) and rely-office-sheet-wasm (the spreadsheet)
// bound for Node, with the licence files and the fonts. @relybytes/rely-office-react, for the
// frontend: the reading and the spreadsheet modules bound for the web, and the editor components
// compiled from TypeScript. Both are packed into dist/.
//
// Usage: node scripts/build-npm.mjs [--no-pack | --print-bindgen-version]
// Needs cargo with the wasm32-unknown-unknown target, and wasm-bindgen-cli at exactly the
// version of the wasm-bindgen crate in Cargo.lock. CARGO and WASM_BINDGEN override the commands.

import { execFileSync, execSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const packageDirectory = join(root, 'packages', 'rely-office-node');
const reactDirectory = join(root, 'packages', 'rely-office-react');
const bindingsDirectory = join(packageDirectory, 'wasm');
const distDirectory = join(root, 'dist');
const cargo = process.env.CARGO ?? 'cargo';
const wasmBindgen = process.env.WASM_BINDGEN ?? 'wasm-bindgen';
const pack = !process.argv.includes('--no-pack');

function fail(message) {
  console.error(`build-npm: ${message}`);
  process.exit(1);
}

function run(command, args, options = {}) {
  console.log(`> ${command} ${args.join(' ')}`);
  try {
    execFileSync(command, args, { cwd: root, stdio: 'inherit', ...options });
  } catch (error) {
    fail(`${command} failed${error.status === undefined ? `: ${error.message}` : ''}`);
  }
}

function lockedVersion(crate) {
  const lock = readFileSync(join(root, 'Cargo.lock'), 'utf8');
  const match = new RegExp(`name = "${crate}"\\r?\\nversion = "([^"]+)"`).exec(lock);
  if (!match) {
    fail(`${crate} is not in Cargo.lock`);
  }
  return match[1];
}

function workspaceVersion() {
  const manifest = readFileSync(join(root, 'Cargo.toml'), 'utf8');
  const section = manifest.split('[workspace.package]')[1] ?? '';
  const match = /^version = "([^"]+)"/m.exec(section);
  if (!match) {
    fail('no version in [workspace.package] of Cargo.toml');
  }
  return match[1];
}

const expectedBindgen = lockedVersion('wasm-bindgen');
if (process.argv.includes('--print-bindgen-version')) {
  console.log(expectedBindgen);
  process.exit(0);
}

const version = workspaceVersion();
for (const directory of [packageDirectory, reactDirectory]) {
  const packageJson = JSON.parse(readFileSync(join(directory, 'package.json'), 'utf8'));
  if (packageJson.version !== version) {
    fail(`${packageJson.name} says ${packageJson.version}, the workspace says ${version}: align them`);
  }
}

let installedBindgen;
try {
  installedBindgen = execFileSync(wasmBindgen, ['--version'], { encoding: 'utf8' }).trim();
} catch {
  fail(
    `${wasmBindgen} not found: cargo install wasm-bindgen-cli --version ${expectedBindgen} --locked`,
  );
}
if (installedBindgen !== `wasm-bindgen ${expectedBindgen}`) {
  fail(
    `${installedBindgen} does not match the crate ${expectedBindgen}: ` +
      `cargo install wasm-bindgen-cli --version ${expectedBindgen} --locked --force`,
  );
}

// Separate modules, so a process that only reads documents never loads the typesetter.
const modules = ['rely_office_wasm', 'rely_office_render_wasm', 'rely_office_sheet_wasm'];
// The browser gets reading, writing and the spreadsheet; the typesetter stays on the backend.
const webModules = ['rely_office_wasm', 'rely_office_sheet_wasm'];
const webDirectory = join(reactDirectory, 'wasm');

run(cargo, [
  'build',
  '--package',
  'rely-office-wasm',
  '--package',
  'rely-office-render-wasm',
  '--package',
  'rely-office-sheet-wasm',
  '--target',
  'wasm32-unknown-unknown',
  '--profile',
  'wasm-release',
  '--locked',
]);

rmSync(bindingsDirectory, { recursive: true, force: true });
for (const module of modules) {
  run(wasmBindgen, [
    '--target',
    'nodejs',
    '--out-dir',
    bindingsDirectory,
    '--out-name',
    module,
    join(root, 'target', 'wasm32-unknown-unknown', 'wasm-release', `${module}.wasm`),
  ]);
}

rmSync(webDirectory, { recursive: true, force: true });
for (const module of webModules) {
  run(wasmBindgen, [
    '--target',
    'web',
    '--out-dir',
    webDirectory,
    '--out-name',
    module,
    join(root, 'target', 'wasm32-unknown-unknown', 'wasm-release', `${module}.wasm`),
  ]);
}

for (const directory of [packageDirectory, reactDirectory]) {
  for (const file of ['LICENSE', 'NOTICE']) {
    copyFileSync(join(root, file), join(directory, file));
  }
}

const fontsDirectory = join(packageDirectory, 'fonts');
rmSync(fontsDirectory, { recursive: true, force: true });
mkdirSync(fontsDirectory, { recursive: true });
for (const font of readdirSync(join(root, 'fonts'))) {
  copyFileSync(join(root, 'fonts', font), join(fontsDirectory, font));
}

for (const module of modules) {
  const size = statSync(join(bindingsDirectory, `${module}_bg.wasm`)).size;
  console.log(`wasm module ${module}: ${(size / 1024).toFixed(0)} KiB`);
}

// npm is a .cmd script on Windows, which only a shell can start.
function npm(command, directory) {
  console.log(`> npm ${command} (${directory})`);
  try {
    execSync(`npm ${command}`, { cwd: directory, stdio: 'inherit' });
  } catch (error) {
    fail(`npm ${command} failed${error.status === undefined ? `: ${error.message}` : ''}`);
  }
}

npm('ci --no-audit --no-fund', reactDirectory);
npm('run build', reactDirectory);

if (pack) {
  mkdirSync(distDirectory, { recursive: true });
  for (const old of readdirSync(distDirectory).filter((name) => name.endsWith('.tgz'))) {
    rmSync(join(distDirectory, old));
  }
  for (const directory of [packageDirectory, reactDirectory]) {
    npm(`pack --pack-destination "${distDirectory}"`, directory);
  }
  for (const tarball of readdirSync(distDirectory).filter((name) => name.endsWith('.tgz'))) {
    const size = statSync(join(distDirectory, tarball)).size;
    console.log(`package: dist/${tarball} (${(size / 1024).toFixed(0)} KiB)`);
  }
}
