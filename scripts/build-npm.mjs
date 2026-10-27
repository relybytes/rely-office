#!/usr/bin/env node
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// Build the npm package @relybytes/rely-office: compile rely-office-wasm (reading) and
// rely-office-render-wasm (rendering) for wasm32-unknown-unknown, generate the Node bindings with
// wasm-bindgen, copy the licence files and the fonts, and pack the package into dist/.
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

const packageJson = JSON.parse(readFileSync(join(packageDirectory, 'package.json'), 'utf8'));
const version = workspaceVersion();
if (packageJson.version !== version) {
  fail(`package.json says ${packageJson.version}, the workspace says ${version}: align them`);
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
// The spreadsheet also runs in the browser, for the sheet editor.
const webModules = ['rely_office_sheet_wasm'];
const webDirectory = join(packageDirectory, 'web');

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

for (const module of webModules) {
  for (const generated of readdirSync(webDirectory).filter((file) => file.startsWith(module))) {
    rmSync(join(webDirectory, generated));
  }
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

for (const file of ['LICENSE', 'NOTICE']) {
  copyFileSync(join(root, file), join(packageDirectory, file));
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

if (pack) {
  mkdirSync(distDirectory, { recursive: true });
  for (const old of readdirSync(distDirectory).filter((name) => name.endsWith('.tgz'))) {
    rmSync(join(distDirectory, old));
  }
  console.log('> npm pack');
  // npm is a .cmd script on Windows, which only a shell can start.
  execSync(`npm pack --pack-destination "${distDirectory}"`, {
    cwd: packageDirectory,
    stdio: 'inherit',
  });
  for (const tarball of readdirSync(distDirectory).filter((name) => name.endsWith('.tgz'))) {
    const size = statSync(join(distDirectory, tarball)).size;
    console.log(`package: dist/${tarball} (${(size / 1024).toFixed(0)} KiB)`);
  }
}
