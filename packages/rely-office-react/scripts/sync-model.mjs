// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// The model is declared once, in the backend package's model.d.ts; this package compiles a copy
// of it, so the two packages can never disagree on the shape of a tree, a slide or a cell.

import { copyFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageDirectory = resolve(dirname(fileURLToPath(import.meta.url)), '..');
copyFileSync(
  join(packageDirectory, '..', 'rely-office-node', 'model.d.ts'),
  join(packageDirectory, 'src', 'model.ts'),
);
