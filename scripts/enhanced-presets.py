"""Write crates/rely-office-core/src/geometry/enhanced_presets.rs from LibreOffice's
share/filter/oox-drawingml-cs-presets: the DrawingML presets that presets.rs does not define in
guide language, as OpenDocument enhanced geometries.

    python scripts/enhanced-presets.py <oox-drawingml-cs-presets>         crates/rely-office-core/src/geometry/presets.rs         crates/rely-office-core/src/geometry/enhanced_presets.rs

An optional fourth argument lists, comma separated, presets of presets.rs to take from the
table instead."""
import re
import sys

DUMP = sys.argv[1]
PRESETS_RS = sys.argv[2]
OUT = sys.argv[3]
REPLACE = set(sys.argv[4].split(',')) if len(sys.argv) > 4 else set()

text = open(DUMP, encoding='utf-8').read()
ours = open(PRESETS_RS, encoding='utf-8').read()
defined = set()
for head in re.findall(r'^\[([^\]]+)\]', ours, re.M):
    name = head.split('=')[0]
    if name not in REPLACE:
        defined.add(name)

entries = re.split(r'^/\* (\S+) \*/\n', text, flags=re.M)[1:]
shapes = {}
for name, body in zip(entries[0::2], entries[1::2]):
    shapes[name] = body

PARAM = re.compile(r'\{ Value = \(any\) \{ \((?:long|double)\) (-?[0-9.]+) \}, Type = \(short\) ([0-9]+) \}')
PAIR = re.compile(r'\{ First = \(com\.sun\.star\.drawing\.EnhancedCustomShapeParameter\) (\{ Value = \(any\) \{ \((?:long|double)\) -?[0-9.]+ \}, Type = \(short\) [0-9]+ \}), Second = \(com\.sun\.star\.drawing\.EnhancedCustomShapeParameter\) (\{ Value = \(any\) \{ \((?:long|double)\) -?[0-9.]+ \}, Type = \(short\) [0-9]+ \}) \}')
SEGMENT = re.compile(r'\{ Command = \(short\) ([0-9]+), Count = \(short\) ([0-9]+) \}')
LETTER = {1: 'M', 2: 'L', 3: 'C', 4: 'Z', 5: 'N', 6: 'F', 7: 'S', 16: 'Q', 17: 'G', 18: 'H', 19: 'I', 20: 'J', 21: 'K'}
POINTS = {1: 1, 2: 1, 3: 3, 16: 2, 17: 2}


def parameter(raw):
    value, kind = PARAM.match(raw).groups()
    kind = int(kind)
    if kind == 0:
        number = float(value)
        return str(int(number)) if number == int(number) else value
    if kind == 1:
        return f'?f{int(float(value))}'
    if kind == 2:
        return f'${int(float(value))}'
    raise ValueError(f'parameter type {kind}')


def section(body, key):
    match = re.search(r'Name = "' + key + r'", Handle = \(long\) 0, Value = \(any\) \{ (.*?) \}, State = \(com\.sun\.star\.beans\.PropertyState\) DIRECT_VALUE', body)
    return match.group(1) if match else None


out = []
skipped = []
for name in sorted(shapes):
    if name in defined:
        continue
    body = shapes[name]
    lines = body.split('\n')
    values = {}
    for index, line in enumerate(lines):
        if line in ('AdjustmentValues', 'Equations', 'Path'):
            values[line] = lines[index + 1]
    modifiers = re.findall(r'Value = \(any\) \{ \((?:long|double)\) (-?[0-9.]+) \}, State = \(com\.sun\.star\.beans\.PropertyState\) DIRECT_VALUE, Name = "([a-z0-9]+)"', values.get('AdjustmentValues', ''))
    equations = re.findall(r'"((?:[^"\\]|\\.)*)"', values.get('Equations', ''))
    path = values['Path']
    coordinates_raw = section(path, 'Coordinates')
    segments_raw = section(path, 'Segments')
    if coordinates_raw is None or segments_raw is None:
        skipped.append(name)
        continue
    points = [(parameter(a), parameter(b)) for a, b in PAIR.findall(coordinates_raw)]
    segments = [(int(c), int(n)) for c, n in SEGMENT.findall(segments_raw)]
    views = re.findall(r'\{ Width = \(long\) (-?[0-9]+), Height = \(long\) (-?[0-9]+) \}', section(path, 'SubViewSize') or '')
    tokens = []
    cursor = 0
    ok = True
    for command, count in segments:
        letter = LETTER.get(command)
        if letter is None:
            ok = False
            break
        if command in POINTS:
            for _ in range(max(count, 1) if command != 1 else count):
                tokens.append(letter)
                for _ in range(POINTS[command]):
                    if cursor >= len(points):
                        ok = False
                        break
                    x, y = points[cursor]
                    cursor += 1
                    tokens.extend([x, y])
        else:
            tokens.append(letter)
    if not ok or cursor != len(points):
        skipped.append(name)
        continue
    formulas = [re.sub(r'\?(\d+)', r'?f\1', equation).replace('  ', ' ').strip() for equation in equations]
    out.append(f'[{name}]')
    if modifiers:
        out.append('mod ' + ' '.join(f'{key}={value}' for value, key in modifiers))
    for formula in formulas:
        out.append('eq ' + formula)
    if views:
        out.append('view ' + ' '.join(f'{w} {h}' for w, h in views))
    out.append('path ' + ' '.join(tokens))
    out.append('')

header = '''// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The preset shapes of `DrawingML` that `presets.rs` does not write in guide language, as
//! `OpenDocument` enhanced geometries: the modifiers with their `DrawingML` names, one
//! `draw:equation` formula a line (`?fN` is the Nth), the view of each subpath when it has its
//! own, and the `draw:enhanced-path`. Generated from the `oox-drawingml-cs-presets` table of
//! `LibreOffice` (MPL-2.0), itself drawn from the preset definitions of ECMA-376.

pub(super) const ENHANCED_PRESETS: &str = r"
'''
open(OUT, 'w', encoding='utf-8', newline='\n').write(header + '\n'.join(out) + '";\n')
print(len(out), 'lines;', sum(1 for line in out if line.startswith('[')), 'shapes; skipped', skipped)
