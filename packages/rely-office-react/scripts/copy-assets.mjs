// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

// What tsc does not emit: the stylesheet of the components, and the bundled fonts with their
// licences for the opt-in fonts.css.

import { copyFileSync, mkdirSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageDirectory = resolve(dirname(fileURLToPath(import.meta.url)), '..');
copyFileSync(join(packageDirectory, 'src', 'styles.css'), join(packageDirectory, 'dist', 'styles.css'));
copyFileSync(join(packageDirectory, 'src', 'fonts.css'), join(packageDirectory, 'dist', 'fonts.css'));
const fonts = join(packageDirectory, '..', '..', 'fonts');
const target = join(packageDirectory, 'dist', 'fonts');
mkdirSync(target, { recursive: true });
for (const file of readdirSync(fonts)) {
  if (/^(Carlito|Caladea|Liberation)[A-Za-z]*-[A-Za-z]+\.ttf$/.test(file) || /^OFL-.*\.txt$/.test(file)) {
    copyFileSync(join(fonts, file), join(target, file));
  }
}
