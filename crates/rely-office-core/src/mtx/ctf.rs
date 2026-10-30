// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The Compact Table Format back to TrueType (sections 4 to 7 of the submission): the font's
//! tables as they were, but for `glyf` (outlines as triplets, the initial pushes and the rest
//! of each glyph program in the other two blocks), `loca` (left out, rebuilt), `cvt ` (deltas),
//! `hdmx` and `VDMX` (the error of a prediction, a few bits each).

/// The most glyphs, tables and components read, past which a font is refused.
const MAX_TABLES: usize = 64;
const MAX_COMPONENTS: usize = 1024;
/// The most points of a glyph, and the most ones of a magnitude-dependent value.
const MAX_POINTS: usize = 0xFFFF;
const MAX_MAGNITUDE: u32 = 4096;

const HOP3: u8 = 251;
const HOP4: u8 = 252;

const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
const WE_HAVE_A_SCALE: u16 = 0x0008;
const MORE_COMPONENTS: u16 = 0x0020;
const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;
const WE_HAVE_INSTRUCTIONS: u16 = 0x0100;

/// Big-endian reads over one of the three streams, failing past its end.
pub(super) struct Stream<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Stream<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let bytes = self.bytes.get(self.at..self.at.checked_add(count)?)?;
        self.at += count;
        Some(bytes)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn u16(&mut self) -> Option<u16> {
        let bytes = self.take(2)?;
        Some(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn i16(&mut self) -> Option<i16> {
        self.u16()
            .map(|value| i16::from_be_bytes(value.to_be_bytes()))
    }

    /// `255USHORT` (6.1.1): one byte, a byte past 253 or 506, or a word.
    fn u255(&mut self) -> Option<u16> {
        Some(match self.u8()? {
            253 => self.u16()?,
            255 => 253 + u16::from(self.u8()?),
            254 => 506 + u16::from(self.u8()?),
            code => u16::from(code),
        })
    }

    /// `255SHORT` (6.1.2): as `255USHORT` past 250 and 500, with 250 flipping the sign.
    fn s255(&mut self) -> Option<i16> {
        let code = self.u8()?;
        if code == 253 {
            return self.i16();
        }
        let (sign, code) = if code == 250 {
            (-1, self.u8()?)
        } else {
            (1, code)
        };
        let value = match code {
            255 => 250 + i16::from(self.u8()?),
            254 => 500 + i16::from(self.u8()?),
            code => i16::from(code),
        };
        Some(sign * value)
    }
}

/// The TrueType font of the three blocks, no larger than `max_bytes`.
pub(super) fn to_truetype(
    rest: &[u8],
    push: &[u8],
    code: &[u8],
    max_bytes: usize,
) -> Option<Vec<u8>> {
    let tables = directory(rest)?;
    let table = |tag: &[u8; 4]| {
        tables
            .iter()
            .find(|(name, _)| name == tag)
            .map(|(_, data)| *data)
    };
    let head = table(b"head").filter(|head| head.len() >= 54)?;
    let maxp = table(b"maxp").filter(|maxp| maxp.len() >= 6)?;
    let glyphs = usize::from(u16::from_be_bytes([maxp[4], maxp[5]]));
    let units_per_em = u16::from_be_bytes([head[18], head[19]]);

    let mut out_tables: Vec<([u8; 4], Vec<u8>)> = Vec::with_capacity(tables.len());
    let mut long_loca = false;
    let mut budget = max_bytes;
    for (tag, data) in &tables {
        let converted = match tag {
            b"glyf" => {
                let (glyf, offsets) = glyf(data, push, code, glyphs, budget)?;
                let (loca, long) = loca(&offsets);
                long_loca = long;
                out_tables.push((*b"loca", loca));
                Some(glyf)
            }
            b"loca" => None,
            b"cvt " => Some(cvt(data)?),
            b"hdmx" => hdmx(data, glyphs, units_per_em, &advances(&table)),
            b"VDMX" => vdmx(data),
            _ => Some(data.to_vec()),
        };
        if let Some(converted) = converted {
            budget = budget.checked_sub(converted.len())?;
            out_tables.push((*tag, converted));
        }
    }
    if let Some((_, head)) = out_tables.iter_mut().find(|(tag, _)| tag == b"head") {
        head[50..52].copy_from_slice(&u16::from(long_loca).to_be_bytes());
    }
    font_file(rest.get(0..4)?.try_into().ok()?, out_tables, max_bytes)
}

/// The tables of the first block, by the table directory it keeps as it was.
fn directory(rest: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
    let mut stream = Stream::new(rest);
    stream.take(4)?;
    let count = usize::from(stream.u16()?);
    if count > MAX_TABLES {
        return None;
    }
    stream.take(6)?;
    let mut tables = Vec::with_capacity(count);
    for _ in 0..count {
        let tag: [u8; 4] = stream.take(4)?.try_into().ok()?;
        stream.take(4)?;
        let offset = u32::from_be_bytes(stream.take(4)?.try_into().ok()?) as usize;
        let length = u32::from_be_bytes(stream.take(4)?.try_into().ok()?) as usize;
        let data = if &tag == b"loca" {
            &[][..]
        } else {
            rest.get(offset..offset.checked_add(length)?)?
        };
        tables.push((tag, data));
    }
    Some(tables)
}

/// The advance width of each glyph from `hhea` and `hmtx`, the last one repeated.
fn advances<'a>(table: &impl Fn(&[u8; 4]) -> Option<&'a [u8]>) -> Vec<u16> {
    let Some(metrics) = table(b"hhea")
        .and_then(|hhea| hhea.get(34..36))
        .map(|count| usize::from(u16::from_be_bytes([count[0], count[1]])))
    else {
        return Vec::new();
    };
    table(b"hmtx")
        .unwrap_or_default()
        .as_chunks::<4>()
        .0
        .iter()
        .take(metrics)
        .map(|metric| u16::from_be_bytes([metric[0], metric[1]]))
        .collect()
}

/// The glyphs in TrueType and where each starts, the last offset the end.
fn glyf(
    data: &[u8],
    push: &[u8],
    code: &[u8],
    glyphs: usize,
    max_bytes: usize,
) -> Option<(Vec<u8>, Vec<usize>)> {
    let mut glyf_stream = Stream::new(data);
    let mut push_stream = Stream::new(push);
    let mut code_stream = Stream::new(code);
    let mut out = Vec::with_capacity(data.len() * 2);
    let mut offsets = Vec::with_capacity(glyphs + 1);
    for _ in 0..glyphs {
        offsets.push(out.len());
        let contours = glyf_stream.i16()?;
        if contours < 0 {
            composite(
                &mut glyf_stream,
                &mut push_stream,
                &mut code_stream,
                &mut out,
            )?;
        } else if contours > 0 {
            simple(
                contours,
                &mut glyf_stream,
                &mut push_stream,
                &mut code_stream,
                &mut out,
            )?;
        }
        out.resize(out.len().next_multiple_of(4), 0);
        if out.len() > max_bytes {
            return None;
        }
    }
    offsets.push(out.len());
    Some((out, offsets))
}

fn simple(
    contours: i16,
    glyf: &mut Stream<'_>,
    push: &mut Stream<'_>,
    code: &mut Stream<'_>,
    out: &mut Vec<u8>,
) -> Option<()> {
    let (contours, bounds) = if contours == 0x7FFF {
        let contours = glyf.i16().filter(|contours| *contours > 0)?;
        (
            contours,
            Some([glyf.i16()?, glyf.i16()?, glyf.i16()?, glyf.i16()?]),
        )
    } else {
        (contours, None)
    };
    let mut ends = Vec::with_capacity(usize::try_from(contours).ok()?);
    let mut last = 0_usize;
    for index in 0..contours {
        let value = usize::from(glyf.u255()?);
        last = if index == 0 { value } else { last + value };
        if last >= MAX_POINTS {
            return None;
        }
        ends.push(u16::try_from(last).ok()?);
    }
    let flags = glyf.take(last + 1)?;
    let mut points = Vec::with_capacity(flags.len());
    let (mut x, mut y) = (0_i32, 0_i32);
    for &flag in flags {
        let (dx, dy) = triplet(flag & 0x7F, glyf)?;
        x += dx;
        y += dy;
        points.push((
            i16::try_from(x).ok()?,
            i16::try_from(y).ok()?,
            flag & 0x80 == 0,
        ));
    }
    let program = program(glyf, push, code)?;

    let [x_min, y_min, x_max, y_max] = bounds.unwrap_or_else(|| {
        let xs = points.iter().map(|point| point.0);
        let ys = points.iter().map(|point| point.1);
        [
            xs.clone().min().unwrap_or(0),
            ys.clone().min().unwrap_or(0),
            xs.max().unwrap_or(0),
            ys.max().unwrap_or(0),
        ]
    });
    for value in [contours, x_min, y_min, x_max, y_max] {
        out.extend(value.to_be_bytes());
    }
    for end in ends {
        out.extend(end.to_be_bytes());
    }
    out.extend(u16::try_from(program.len()).ok()?.to_be_bytes());
    out.extend(program);
    outline(&points, out);
    Some(())
}

/// The change from the previous point and its on-curve bit, from the triplet table of 5.11:
/// the flag says how many bytes follow, how many bits each coordinate takes, what to add and the
/// signs.
fn triplet(flag: u8, glyf: &mut Stream<'_>) -> Option<(i32, i32)> {
    let flag_value = i32::from(flag);
    let with_sign = |sign: i32, value: i32| if sign & 1 == 1 { value } else { -value };
    Some(match flag {
        0..10 => (
            0,
            with_sign(
                flag_value,
                ((flag_value & 0b1110) << 7) + i32::from(glyf.u8()?),
            ),
        ),
        10..20 => (
            with_sign(
                flag_value,
                (((flag_value - 10) & 0b1110) << 7) + i32::from(glyf.u8()?),
            ),
            0,
        ),
        20..84 => {
            let base = flag_value - 20;
            let byte = i32::from(glyf.u8()?);
            (
                with_sign(flag_value, 1 + (base & 0x30) + (byte >> 4)),
                with_sign(flag_value >> 1, 1 + ((base & 0x0C) << 2) + (byte & 0x0F)),
            )
        }
        84..120 => {
            let base = flag_value - 84;
            let (first, second) = (i32::from(glyf.u8()?), i32::from(glyf.u8()?));
            (
                with_sign(flag_value, 1 + ((base / 12) << 8) + first),
                with_sign(flag_value >> 1, 1 + (((base % 12) >> 2) << 8) + second),
            )
        }
        120..124 => {
            let bytes = glyf.take(3)?;
            let (a, b, c) = (
                i32::from(bytes[0]),
                i32::from(bytes[1]),
                i32::from(bytes[2]),
            );
            (
                with_sign(flag_value, (a << 4) + (b >> 4)),
                with_sign(flag_value >> 1, ((b & 0x0F) << 8) + c),
            )
        }
        _ => {
            let bytes = glyf.take(4)?;
            (
                with_sign(
                    flag_value,
                    i32::from(u16::from_be_bytes([bytes[0], bytes[1]])),
                ),
                with_sign(
                    flag_value >> 1,
                    i32::from(u16::from_be_bytes([bytes[2], bytes[3]])),
                ),
            )
        }
    })
}

/// The flags and the coordinates of a simple glyph in TrueType: each change in a byte with its
/// sign in the flag when it fits, none when it is nought, a word otherwise; equal flags repeated.
fn outline(points: &[(i16, i16, bool)], out: &mut Vec<u8>) {
    let mut flags = Vec::with_capacity(points.len());
    let mut xs = Vec::with_capacity(points.len());
    let mut ys = Vec::with_capacity(points.len());
    let (mut x, mut y) = (0_i16, 0_i16);
    for &(px, py, on_curve) in points {
        let mut flag = u8::from(on_curve);
        flag |= coordinate(px.wrapping_sub(x), 0x02, 0x10, &mut xs);
        flag |= coordinate(py.wrapping_sub(y), 0x04, 0x20, &mut ys);
        flags.push(flag);
        (x, y) = (px, py);
    }
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index];
        let mut repeat = 0_u8;
        while repeat < 255 && flags.get(index + 1 + usize::from(repeat)) == Some(&flag) {
            repeat += 1;
        }
        if repeat > 0 {
            out.extend([flag | 0x08, repeat]);
        } else {
            out.push(flag);
        }
        index += 1 + usize::from(repeat);
    }
    out.extend(xs);
    out.extend(ys);
}

fn coordinate(delta: i16, short: u8, same_or_positive: u8, bytes: &mut Vec<u8>) -> u8 {
    if delta == 0 {
        same_or_positive
    } else if let Ok(small) = u8::try_from(delta.unsigned_abs()) {
        bytes.push(small);
        short | if delta > 0 { same_or_positive } else { 0 }
    } else {
        bytes.extend(delta.to_be_bytes());
        0
    }
}

fn composite(
    glyf: &mut Stream<'_>,
    push: &mut Stream<'_>,
    code: &mut Stream<'_>,
    out: &mut Vec<u8>,
) -> Option<()> {
    out.extend((-1_i16).to_be_bytes());
    out.extend(glyf.take(8)?);
    let mut flags = MORE_COMPONENTS;
    let mut components = 0;
    while flags & MORE_COMPONENTS != 0 {
        components += 1;
        if components > MAX_COMPONENTS {
            return None;
        }
        flags = glyf.u16()?;
        let mut size = 2 + if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            4
        } else {
            2
        };
        if flags & WE_HAVE_A_SCALE != 0 {
            size += 2;
        } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
            size += 4;
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            size += 8;
        }
        out.extend(flags.to_be_bytes());
        out.extend(glyf.take(size)?);
    }
    if flags & WE_HAVE_INSTRUCTIONS != 0 {
        let program = program(glyf, push, code)?;
        out.extend(u16::try_from(program.len()).ok()?.to_be_bytes());
        out.extend(program);
    }
    Some(())
}

/// A glyph program: its initial pushes from the second block made instructions again, then the
/// rest from the third.
fn program(glyf: &mut Stream<'_>, push: &mut Stream<'_>, code: &mut Stream<'_>) -> Option<Vec<u8>> {
    let count = usize::from(glyf.u255()?);
    let size = usize::from(glyf.u255()?);
    let values = push_values(push, count)?;
    let mut program = push_instructions(&values);
    program.extend(code.take(size)?);
    Some(program)
}

/// The pushed values, with the hop codes (6.2.4) put back: a value two back repeated between
/// the values read.
fn push_values(push: &mut Stream<'_>, count: usize) -> Option<Vec<i16>> {
    let mut values = Vec::with_capacity(count);
    while values.len() < count {
        match push.peek()? {
            HOP3 | HOP4 => {
                let hop = push.u8()?;
                let a = *values.get(values.len().checked_sub(2)?)?;
                values.extend([a, push.s255()?, a]);
                if hop == HOP4 {
                    values.extend([push.s255()?, a]);
                }
            }
            _ => values.push(push.s255()?),
        }
    }
    (values.len() == count).then_some(values)
}

/// TrueType instructions pushing the values: bytes with `PUSHB` or `NPUSHB` while they fit a
/// byte, words with `PUSHW` or `NPUSHW` otherwise.
fn push_instructions(values: &[i16]) -> Vec<u8> {
    let mut program = Vec::new();
    let mut index = 0;
    while index < values.len() {
        let is_byte = |value: &i16| (0..=255).contains(value);
        let bytes = is_byte(&values[index]);
        let run = values[index..]
            .iter()
            .take(255)
            .take_while(|value| is_byte(value) == bytes)
            .count();
        let count = u8::try_from(run).unwrap_or(u8::MAX);
        match (bytes, run <= 8) {
            (true, true) => program.push(0xB0 + count - 1),
            (true, false) => program.extend([0x40, count]),
            (false, true) => program.push(0xB8 + count - 1),
            (false, false) => program.extend([0x41, count]),
        }
        for value in &values[index..index + run] {
            if bytes {
                program.push(value.to_be_bytes()[1]);
            } else {
                program.extend(value.to_be_bytes());
            }
        }
        index += run;
    }
    program
}

/// The offsets as a `loca` table, and whether it takes the long form.
fn loca(offsets: &[usize]) -> (Vec<u8>, bool) {
    let long = offsets.last().is_some_and(|end| *end > 0x1FFFE);
    let mut table = Vec::with_capacity(offsets.len() * if long { 4 } else { 2 });
    for &offset in offsets {
        if long {
            table.extend(u32::try_from(offset).unwrap_or(u32::MAX).to_be_bytes());
        } else {
            table.extend(u16::try_from(offset / 2).unwrap_or(u16::MAX).to_be_bytes());
        }
    }
    (table, long)
}

/// The control values: each the change from the previous one, in one to three bytes (5.2).
fn cvt(data: &[u8]) -> Option<Vec<u8>> {
    let mut stream = Stream::new(data);
    let count = stream.u16()?;
    let mut out = Vec::with_capacity(usize::from(count) * 2);
    let mut value = 0_i16;
    for _ in 0..count {
        let code = stream.u8()?;
        let delta = match code {
            0..238 => i16::from(code),
            238 => stream.i16()?,
            239..=247 => -(i16::from(stream.u8()?) + i16::from(code - 239) * 238),
            248.. => i16::from(stream.u8()?) + i16::from(code - 247) * 238,
        };
        value = value.wrapping_add(delta);
        out.extend(value.to_be_bytes());
    }
    Some(out)
}

/// Values written with a number of one bits for their size, a nought, and a sign bit after a
/// value that is not nought (5.3), from the low bit of each byte.
struct Magnitudes<'a> {
    bytes: &'a [u8],
    bit: usize,
}

impl Magnitudes<'_> {
    fn bit(&mut self) -> Option<bool> {
        let byte = *self.bytes.get(self.bit / 8)?;
        let value = (byte >> (self.bit % 8)) & 1 == 1;
        self.bit += 1;
        Some(value)
    }

    fn next(&mut self) -> Option<i32> {
        let mut magnitude = 0;
        while self.bit()? {
            magnitude += 1;
            if magnitude > MAX_MAGNITUDE {
                return None;
            }
        }
        let magnitude = i32::try_from(magnitude).ok()?;
        if magnitude == 0 || !self.bit()? {
            Some(magnitude)
        } else {
            Some(-magnitude)
        }
    }

    /// The next byte after the values read so far.
    fn byte(&self) -> usize {
        self.bit.div_ceil(8)
    }
}

/// The device widths, each the error of the width the advance scales to (5.4); a table kept as
/// it was carries its version subtracted from `0xFFFF`. `None` leaves the table out.
fn hdmx(data: &[u8], glyphs: usize, units_per_em: u16, advances: &[u16]) -> Option<Vec<u8>> {
    let mut stream = Stream::new(data);
    let version = stream.u16()?;
    if version > 0x7FFF {
        let mut kept = data.to_vec();
        kept[..2].copy_from_slice(&(0xFFFF - version).to_be_bytes());
        return Some(kept);
    }
    let records = stream.u16()?;
    stream.take(4)?;
    let last = *advances.last()?;
    if units_per_em == 0 {
        return None;
    }
    let mut sizes = Vec::with_capacity(usize::from(records));
    for _ in 0..records {
        sizes.push((stream.u8()?, stream.u8()?));
    }
    let record_size = (2 + glyphs).next_multiple_of(4);
    let mut out = Vec::with_capacity(8 + record_size * sizes.len());
    out.extend(version.to_be_bytes());
    out.extend(records.to_be_bytes());
    out.extend(u32::try_from(record_size).ok()?.to_be_bytes());
    let mut bits = Magnitudes {
        bytes: data.get(stream.at..)?,
        bit: 0,
    };
    let em = i64::from(units_per_em);
    for (ppem, max_width) in sizes {
        let start = out.len();
        out.extend([ppem, max_width]);
        for glyph in 0..glyphs {
            let advance = i64::from(*advances.get(glyph).unwrap_or(&last));
            let predicted = ((64 * i64::from(ppem) * advance + em / 2) / em + 32) / 64;
            let width = predicted + i64::from(bits.next()?);
            out.push(u8::try_from(width).ok()?);
        }
        out.resize(start + record_size, 0);
    }
    Some(out)
}

/// The vertical extents per size (5.5): the size one past the previous, the extents scaled by
/// the multipliers each group keeps, each plus its error; groups start on a byte.
fn vdmx(data: &[u8]) -> Option<Vec<u8>> {
    let mut stream = Stream::new(data);
    let version = stream.u16()?;
    if version > 0x7FFF {
        let mut kept = data.to_vec();
        kept[..2].copy_from_slice(&(0xFFFF - version).to_be_bytes());
        return Some(kept);
    }
    let groups = stream.u16()?;
    let ratios = stream.u16()?;
    let ratio_records = stream.take(4 * usize::from(ratios))?;
    let mut offsets = Vec::with_capacity(usize::from(ratios));
    for _ in 0..ratios {
        offsets.push(usize::from(stream.u16()?));
    }
    let mut at = *offsets.first()?;
    let mut starts = Vec::with_capacity(usize::from(groups));
    let mut body = Vec::new();
    let mut body_starts = Vec::with_capacity(usize::from(groups));
    for _ in 0..groups {
        starts.push(at);
        body_starts.push(body.len());
        let mut group = Stream::new(data.get(at..)?);
        let records = group.u16()?;
        let y_max_multiplier = i32::from(group.i16()?);
        let y_min_multiplier = i32::from(group.i16()?);
        let mut bits = Magnitudes {
            bytes: data.get(at + 6..)?,
            bit: 0,
        };
        let mut entries = Vec::with_capacity(usize::from(records) * 6);
        let mut predicted = 8;
        let (mut first, mut last) = (0, 0);
        for index in 0..records {
            let ppem = bits.next()? + predicted;
            let y_max = (ppem * y_max_multiplier + 1024) / 2048 + bits.next()?;
            let y_min = -((ppem * y_min_multiplier + 1024) / 2048) + bits.next()?;
            entries.extend(u16::try_from(ppem).ok()?.to_be_bytes());
            entries.extend(i16::try_from(y_max).ok()?.to_be_bytes());
            entries.extend(i16::try_from(y_min).ok()?.to_be_bytes());
            if index == 0 {
                first = ppem;
            }
            last = ppem;
            predicted = ppem + 1;
        }
        body.extend(records.to_be_bytes());
        body.push(u8::try_from(first.clamp(0, 255)).ok()?);
        body.push(u8::try_from(last.clamp(0, 255)).ok()?);
        body.extend(entries);
        at += 6 + bits.byte();
    }
    let header = 6 + 6 * usize::from(ratios);
    let mut out = Vec::with_capacity(header + body.len());
    out.extend(version.to_be_bytes());
    out.extend(groups.to_be_bytes());
    out.extend(ratios.to_be_bytes());
    out.extend(ratio_records);
    for offset in offsets {
        let group = starts.iter().position(|start| *start == offset)?;
        out.extend(
            u16::try_from(header + body_starts[group])
                .ok()?
                .to_be_bytes(),
        );
    }
    out.extend(body);
    Some(out)
}

/// A TrueType file of the tables: sorted by tag, each on four bytes with its checksum, and the
/// whole file's checksum in `head`.
fn font_file(
    version: [u8; 4],
    mut tables: Vec<([u8; 4], Vec<u8>)>,
    max_bytes: usize,
) -> Option<Vec<u8>> {
    tables.sort_by_key(|(tag, _)| *tag);
    let count = u16::try_from(tables.len()).ok()?;
    let selector = count.max(1).ilog2();
    let search = 16_u16 << selector;
    let mut out = Vec::new();
    out.extend(version);
    out.extend(count.to_be_bytes());
    out.extend(search.to_be_bytes());
    out.extend(u16::try_from(selector).ok()?.to_be_bytes());
    out.extend((count * 16 - search).to_be_bytes());
    let mut offset = 12 + 16 * tables.len();
    let mut head_at = None;
    for (tag, data) in &mut tables {
        if tag == b"head" {
            data[8..12].fill(0);
            head_at = Some(offset);
        }
        out.extend(*tag);
        out.extend(checksum(data).to_be_bytes());
        out.extend(u32::try_from(offset).ok()?.to_be_bytes());
        out.extend(u32::try_from(data.len()).ok()?.to_be_bytes());
        offset += data.len().next_multiple_of(4);
    }
    if offset > max_bytes {
        return None;
    }
    for (_, data) in &tables {
        out.extend(data);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    if let Some(head) = head_at {
        let adjustment = 0xB1B0_AFBA_u32.wrapping_sub(checksum(&out));
        out[head + 8..head + 12].copy_from_slice(&adjustment.to_be_bytes());
    }
    Some(out)
}

/// The sum of the table's big-endian words, the last padded with noughts.
pub(super) fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0_u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}
