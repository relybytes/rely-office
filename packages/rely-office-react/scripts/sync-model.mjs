// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

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
