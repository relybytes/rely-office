// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! A `MicroType` Express writer for the tests, following the submission's encoder: TrueType to
//! the Compact Table Format, then each block through `LZCOMP`.

use std::collections::HashMap;

use super::lzcomp::{Ahuff, DISTANCE_WIDTH, MAX_TWO_BYTE_DISTANCE, distance_ranges, preload};

/// How a block is packed.
#[derive(Clone, Copy)]
pub(crate) struct Packing {
    pub(crate) copies: bool,
    pub(crate) run_length: bool,
}

pub(crate) const PACKED: Packing = Packing {
    copies: true,
    run_length: false,
};

/// The font as `MicroType` Express.
pub(crate) fn compress(ttf: &[u8], packing: Packing) -> Vec<u8> {
    let (rest, push, code) = to_ctf(ttf);
    mtx_file(&rest, &push, &code, packing)
}

/// The three blocks behind the ten-byte header.
pub(super) fn mtx_file(rest: &[u8], push: &[u8], code: &[u8], packing: Packing) -> Vec<u8> {
    let blocks = [rest, push, code].map(|block| lz_pack(block, packing));
    let second = 10 + blocks[0].len();
    let third = second + blocks[1].len();
    let mut out = vec![3, 0x0F, 0xFF, 0xFF];
    for offset in [second, third] {
        out.extend(&u32::try_from(offset).unwrap().to_be_bytes()[1..]);
    }
    for block in blocks {
        out.extend(block);
    }
    out
}

struct BitWriter {
    bytes: Vec<u8>,
    buffer: u8,
    count: u8,
}

impl BitWriter {
    fn bit(&mut self, bit: bool) {
        self.buffer = (self.buffer << 1) | u8::from(bit);
        self.count += 1;
        if self.count == 8 {
            self.bytes.push(self.buffer);
            self.buffer = 0;
            self.count = 0;
        }
    }

    fn value(&mut self, value: usize, bits: u32) {
        for bit in (0..bits).rev() {
            self.bit((value >> bit) & 1 == 1);
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.bytes.push(self.buffer << (8 - self.count));
        }
        self.bytes
    }
}

/// The symbol's path from the root, then one more of it, as `MTX_AHUFF_WriteSymbol` does.
fn write_symbol(tree: &mut Ahuff, symbol: usize, bits: &mut BitWriter) {
    let leaf = tree.leaf[symbol];
    let mut path = Vec::new();
    let mut node = leaf;
    while node != 1 {
        let up = tree.tree[node].up;
        path.push(tree.tree[up].right == node);
        node = up;
    }
    for bit in path.into_iter().rev() {
        bits.bit(bit);
    }
    tree.update(leaf);
}

/// One block through `LZCOMP`: copies of two or more bytes found through the pairs seen before,
/// the three one-byte copies, literals otherwise.
pub(super) fn lz_pack(data: &[u8], packing: Packing) -> Vec<u8> {
    let data = if packing.run_length {
        run_length_pack(data)
    } else {
        data.to_vec()
    };
    let mut bits = BitWriter {
        bytes: Vec::new(),
        buffer: 0,
        count: 0,
    };
    bits.bit(packing.run_length);
    let mut distances = Ahuff::new(8);
    let mut lengths = Ahuff::new(8);
    bits.value(data.len(), 24);
    let ranges = distance_ranges(data.len());
    let dup2 = 256 + 8 * ranges as usize;
    let mut symbols = Ahuff::new(u16::try_from(dup2 + 3).unwrap());

    let mut history = preload();
    let mut pairs: HashMap<[u8; 2], Vec<usize>> = HashMap::new();
    for start in 0..history.len() - 1 {
        pairs
            .entry([history[start], history[start + 1]])
            .or_default()
            .push(start);
    }
    let mut at = 0;
    while at < data.len() {
        let here = history.len();
        let best = if packing.copies {
            best_copy(&history, &pairs, &data[at..], ranges)
        } else {
            None
        };
        let taken = if let Some((length, distance)) = best {
            let used = distance_symbols(distance);
            let coded = length
                - if distance >= MAX_TWO_BYTE_DISTANCE {
                    3
                } else {
                    2
                };
            let mut digits = Vec::new();
            let mut rest = coded;
            loop {
                digits.push(rest & 3);
                rest >>= 2;
                if rest == 0 {
                    break;
                }
            }
            digits.reverse();
            let last = digits.len() - 1;
            for (index, digit) in digits.iter().enumerate() {
                let chunk = digit | if index < last { 4 } else { 0 };
                if index == 0 {
                    write_symbol(&mut symbols, 256 + (used - 1) * 8 + chunk, &mut bits);
                } else {
                    write_symbol(&mut lengths, chunk, &mut bits);
                }
            }
            for shift in (0..used).rev() {
                let digit = ((distance - 1) >> (DISTANCE_WIDTH as usize * shift)) & 7;
                write_symbol(&mut distances, digit, &mut bits);
            }
            length
        } else {
            let byte = data[at];
            let symbol = [2, 4, 6]
                .iter()
                .position(|back| history[here - back] == byte)
                .map_or(usize::from(byte), |index| dup2 + index);
            write_symbol(&mut symbols, symbol, &mut bits);
            1
        };
        for offset in 0..taken {
            history.push(data[at + offset]);
            let end = history.len() - 1;
            pairs
                .entry([history[end - 1], history[end]])
                .or_default()
                .push(end - 1);
        }
        at += taken;
    }
    bits.finish()
}

/// How many distance symbols a distance takes.
fn distance_symbols(distance: usize) -> usize {
    let bits = (usize::BITS - (distance - 1).leading_zeros()).max(1) as usize;
    bits.div_ceil(DISTANCE_WIDTH as usize)
}

/// The longest copy for the bytes ahead, with the distance to its last byte.
fn best_copy(
    history: &[u8],
    pairs: &HashMap<[u8; 2], Vec<usize>>,
    ahead: &[u8],
    ranges: u32,
) -> Option<(usize, usize)> {
    let key = [*ahead.first()?, *ahead.get(1)?];
    let here = history.len();
    let mut best: Option<(usize, usize)> = None;
    for &start in pairs.get(&key)?.iter().rev().take(32) {
        let reach = here - start;
        let mut length = 0;
        while length < ahead.len()
            && length < reach
            && length < 4096
            && history[start + length] == ahead[length]
        {
            length += 1;
        }
        let distance = here - (start + length - 1);
        let shortest = if distance >= MAX_TWO_BYTE_DISTANCE {
            3
        } else {
            2
        };
        if length >= shortest
            && distance_symbols(distance) <= ranges as usize
            && best.is_none_or(|(best_length, _)| length > best_length)
        {
            best = Some((length, distance));
        }
    }
    best
}

/// The run-length layer of `MTX_RUNLENGTHCOMP_PackData`.
fn run_length_pack(data: &[u8]) -> Vec<u8> {
    let mut counts = [0_usize; 256];
    for byte in data {
        counts[usize::from(*byte)] += 1;
    }
    let escape = u8::try_from((0..256).min_by_key(|byte| counts[*byte]).unwrap()).unwrap();
    let mut out = vec![escape];
    let mut index = 0;
    while index < data.len() {
        let byte = data[index];
        let run = data[index..]
            .iter()
            .take(255)
            .take_while(|other| **other == byte)
            .count();
        if run > 3 {
            out.extend([escape, u8::try_from(run).unwrap(), byte]);
            index += run;
        } else {
            if byte == escape {
                out.extend([escape, 0]);
            } else {
                out.push(byte);
            }
            index += 1;
        }
    }
    out
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> usize {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

/// The tables of a font file, by its directory.
pub(super) fn tables(ttf: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    (0..usize::from(u16_at(ttf, 4)))
        .map(|index| {
            let entry = 12 + index * 16;
            let offset = u32_at(ttf, entry + 8);
            let length = u32_at(ttf, entry + 12);
            (
                ttf[entry..entry + 4].try_into().unwrap(),
                ttf[offset..offset + length].to_vec(),
            )
        })
        .collect()
}

/// A font file of the tables, in the order given, for the tests that add tables.
pub(super) fn font_of(tables: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0, 1, 0, 0];
    out.extend(u16::try_from(tables.len()).unwrap().to_be_bytes());
    out.extend([0; 6]);
    let mut offset = 12 + 16 * tables.len();
    for (tag, data) in tables {
        out.extend(tag);
        out.extend([0; 4]);
        out.extend(u32::try_from(offset).unwrap().to_be_bytes());
        out.extend(u32::try_from(data.len()).unwrap().to_be_bytes());
        offset += data.len().next_multiple_of(4);
    }
    for (_, data) in tables {
        out.extend(data);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    out
}

/// Each glyph's bytes, by `loca`.
pub(super) fn glyphs(tables: &[([u8; 4], Vec<u8>)]) -> Vec<Vec<u8>> {
    let table = |tag: &[u8; 4]| &tables.iter().find(|(name, _)| name == tag).unwrap().1;
    let (head, maxp, loca, glyf) = (
        table(b"head"),
        table(b"maxp"),
        table(b"loca"),
        table(b"glyf"),
    );
    let long = u16_at(head, 50) == 1;
    let count = usize::from(u16_at(maxp, 4));
    let offset = |index: usize| {
        if long {
            u32_at(loca, index * 4)
        } else {
            usize::from(u16_at(loca, index * 2)) * 2
        }
    };
    (0..count)
        .map(|index| glyf[offset(index)..offset(index + 1)].to_vec())
        .collect()
}

/// A glyph as what it draws and runs: contour ends, points, bounds, pushed values and the rest
/// of the program, or its components.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Glyph {
    Empty,
    Simple {
        ends: Vec<u16>,
        points: Vec<(i16, i16, bool)>,
        bounds: [i16; 4],
        pushes: Vec<i16>,
        code: Vec<u8>,
    },
    Composite {
        bounds: [i16; 4],
        components: Vec<u8>,
        pushes: Vec<i16>,
        code: Vec<u8>,
    },
}

pub(super) fn parse_glyph(data: &[u8]) -> Glyph {
    if data.is_empty() {
        return Glyph::Empty;
    }
    let contours = i16::from_be_bytes([data[0], data[1]]);
    let bounds = [2, 4, 6, 8].map(|at| i16::from_be_bytes([data[at], data[at + 1]]));
    if contours < 0 {
        let (end, last_flags) = components_end(data);
        let (pushes, code) = if last_flags & 0x100 != 0 {
            let length = usize::from(u16_at(data, end));
            split_pushes(&data[end + 2..end + 2 + length])
        } else {
            (Vec::new(), Vec::new())
        };
        return Glyph::Composite {
            bounds,
            components: data[10..end].to_vec(),
            pushes,
            code,
        };
    }
    let contours = usize::try_from(contours).unwrap();
    let ends: Vec<u16> = (0..contours)
        .map(|index| u16_at(data, 10 + index * 2))
        .collect();
    let at = 10 + contours * 2;
    let length = usize::from(u16_at(data, at));
    let (pushes, code) = split_pushes(&data[at + 2..at + 2 + length]);
    let count = ends.last().map_or(0, |end| usize::from(*end) + 1);
    let mut at = at + 2 + length;
    let mut flags = Vec::with_capacity(count);
    while flags.len() < count {
        let flag = data[at];
        at += 1;
        flags.push(flag);
        if flag & 0x08 != 0 {
            for _ in 0..data[at] {
                flags.push(flag);
            }
            at += 1;
        }
    }
    let read = |short: u8, same: u8, flags: &[u8], at: &mut usize| -> Vec<i16> {
        let mut value = 0_i16;
        flags
            .iter()
            .map(|flag| {
                if flag & short != 0 {
                    let delta = i16::from(data[*at]);
                    *at += 1;
                    value += if flag & same != 0 { delta } else { -delta };
                } else if flag & same == 0 {
                    value += i16::from_be_bytes([data[*at], data[*at + 1]]);
                    *at += 2;
                }
                value
            })
            .collect()
    };
    let xs = read(0x02, 0x10, &flags, &mut at);
    let ys = read(0x04, 0x20, &flags, &mut at);
    let points = flags
        .iter()
        .zip(xs.iter().zip(&ys))
        .map(|(flag, (x, y))| (*x, *y, flag & 1 == 1))
        .collect();
    Glyph::Simple {
        ends,
        points,
        bounds,
        pushes,
        code,
    }
}

/// Where a composite glyph's components end, and the flags of the last.
fn components_end(data: &[u8]) -> (usize, u16) {
    let mut at = 10;
    loop {
        let flags = u16_at(data, at);
        let mut size = 4 + if flags & 1 != 0 { 4 } else { 2 };
        if flags & 0x08 != 0 {
            size += 2;
        } else if flags & 0x40 != 0 {
            size += 4;
        } else if flags & 0x80 != 0 {
            size += 8;
        }
        at += size;
        if flags & 0x20 == 0 {
            return (at, flags);
        }
    }
}

/// The values a program pushes before its first other instruction, and the rest.
pub(super) fn split_pushes(program: &[u8]) -> (Vec<i16>, Vec<u8>) {
    let mut values = Vec::new();
    let mut at = 0;
    while let Some(&op) = program.get(at) {
        let (count, words, start) = match op {
            0x40 => (usize::from(program[at + 1]), false, at + 2),
            0x41 => (usize::from(program[at + 1]), true, at + 2),
            0xB0..=0xB7 => (usize::from(op - 0xB0 + 1), false, at + 1),
            0xB8..=0xBF => (usize::from(op - 0xB8 + 1), true, at + 1),
            _ => break,
        };
        at = start;
        for _ in 0..count {
            if words {
                values.push(i16::from_be_bytes([program[at], program[at + 1]]));
                at += 2;
            } else {
                values.push(i16::from(program[at]));
                at += 1;
            }
        }
    }
    (values, program[at..].to_vec())
}

fn write_u255(value: usize, out: &mut Vec<u8>) {
    let byte = |value: usize| u8::try_from(value).unwrap();
    match value {
        0..253 => out.push(byte(value)),
        253..506 => out.extend([255, byte(value - 253)]),
        506..761 => out.extend([254, byte(value - 506)]),
        _ => {
            out.push(253);
            out.extend(u16::try_from(value).unwrap().to_be_bytes());
        }
    }
}

fn write_s255(value: i16, out: &mut Vec<u8>) {
    let magnitude = value.unsigned_abs();
    let sign = if value < 0 { vec![250] } else { Vec::new() };
    match magnitude {
        0..250 => {
            out.extend(sign);
            out.push(u8::try_from(magnitude).unwrap());
        }
        250..500 => {
            out.extend(sign);
            out.extend([255, u8::try_from(magnitude - 250).unwrap()]);
        }
        500..755 => {
            out.extend(sign);
            out.extend([254, u8::try_from(magnitude - 500).unwrap()]);
        }
        _ => {
            out.push(253);
            out.extend(value.to_be_bytes());
        }
    }
}

/// The pushed values with the hop codes, as the submission's encoder writes them.
fn write_pushes(values: &[i16], out: &mut Vec<u8>) {
    let mut index = 0;
    while index < values.len() {
        let a = index.checked_sub(2).map(|back| values[back]);
        let hop = a.is_some_and(|a| values[index] == a && values.get(index + 2) == Some(&a));
        if hop {
            let a = a.unwrap();
            if values.get(index + 4) == Some(&a) && index + 4 < values.len() {
                out.push(252);
                write_s255(values[index + 1], out);
                write_s255(values[index + 3], out);
                index += 5;
            } else {
                out.push(251);
                write_s255(values[index + 1], out);
                index += 3;
            }
        } else {
            write_s255(values[index], out);
            index += 1;
        }
    }
}

/// The triplet of a point's change, as the WOFF 2.0 encoder writes the same table.
fn write_triplet(on_curve: bool, dx: i32, dy: i32, flags: &mut Vec<u8>, data: &mut Vec<u8>) {
    let on = if on_curve { 0 } else { 128 };
    let (abs_x, abs_y) = (dx.unsigned_abs(), dy.unsigned_abs());
    let x_sign = u32::from(dx >= 0);
    let y_sign = if dy >= 0 { 2 } else { 0 };
    let both = x_sign + y_sign;
    let byte = |value: u32| u8::try_from(value).unwrap();
    let flag = if dx == 0 && abs_y < 1280 {
        data.push(byte(abs_y & 0xFF));
        ((abs_y & 0xF00) >> 7) + u32::from(dy >= 0)
    } else if dy == 0 && abs_x < 1280 {
        data.push(byte(abs_x & 0xFF));
        10 + ((abs_x & 0xF00) >> 7) + x_sign
    } else if abs_x < 65 && abs_y < 65 {
        data.push(byte((((abs_x - 1) & 0xF) << 4) | ((abs_y - 1) & 0xF)));
        20 + ((abs_x - 1) & 0x30) + (((abs_y - 1) & 0x30) >> 2) + both
    } else if abs_x < 769 && abs_y < 769 {
        data.extend([byte((abs_x - 1) & 0xFF), byte((abs_y - 1) & 0xFF)]);
        84 + 12 * (((abs_x - 1) & 0x300) >> 8) + (((abs_y - 1) & 0x300) >> 6) + both
    } else if abs_x < 4096 && abs_y < 4096 {
        data.extend([
            byte(abs_x >> 4),
            byte(((abs_x & 0xF) << 4) | (abs_y >> 8)),
            byte(abs_y & 0xFF),
        ]);
        120 + both
    } else {
        data.extend([
            byte(abs_x >> 8),
            byte(abs_x & 0xFF),
            byte(abs_y >> 8),
            byte(abs_y & 0xFF),
        ]);
        124 + both
    };
    flags.push(on + byte(flag));
}

/// The font in the Compact Table Format: the first block, the pushes, the rest of the programs.
pub(super) fn to_ctf(ttf: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let tables = tables(ttf);
    let glyph_bytes = glyphs(&tables);
    let mut push = Vec::new();
    let mut code = Vec::new();
    let mut converted = Vec::new();
    for (tag, data) in &tables {
        let data = match tag {
            b"glyf" => ctf_glyf(&glyph_bytes, &mut push, &mut code),
            b"loca" => Vec::new(),
            b"cvt " => ctf_cvt(data),
            _ => data.clone(),
        };
        converted.push((*tag, data));
    }
    let mut rest = font_of(&converted);
    for (index, (tag, _)) in converted.iter().enumerate() {
        if tag == b"loca" {
            rest[12 + index * 16 + 8..12 + index * 16 + 16].fill(0);
        }
    }
    (rest, push, code)
}

fn ctf_glyf(glyphs: &[Vec<u8>], push: &mut Vec<u8>, code: &mut Vec<u8>) -> Vec<u8> {
    let mut out = Vec::new();
    for data in glyphs {
        match parse_glyph(data) {
            Glyph::Empty => out.extend([0, 0]),
            Glyph::Simple {
                ends,
                points,
                bounds,
                pushes,
                code: rest,
            } => {
                let contours = i16::try_from(ends.len()).unwrap();
                let computed = [
                    points.iter().map(|point| point.0).min().unwrap(),
                    points.iter().map(|point| point.1).min().unwrap(),
                    points.iter().map(|point| point.0).max().unwrap(),
                    points.iter().map(|point| point.1).max().unwrap(),
                ];
                if computed == bounds {
                    out.extend(contours.to_be_bytes());
                } else {
                    out.extend(0x7FFF_i16.to_be_bytes());
                    out.extend(contours.to_be_bytes());
                    for value in bounds {
                        out.extend(value.to_be_bytes());
                    }
                }
                for (index, end) in ends.iter().enumerate() {
                    let value = if index == 0 {
                        *end
                    } else {
                        end - ends[index - 1]
                    };
                    write_u255(usize::from(value), &mut out);
                }
                let mut flags = Vec::new();
                let mut data = Vec::new();
                let (mut x, mut y) = (0_i32, 0_i32);
                for (px, py, on_curve) in &points {
                    write_triplet(
                        *on_curve,
                        i32::from(*px) - x,
                        i32::from(*py) - y,
                        &mut flags,
                        &mut data,
                    );
                    (x, y) = (i32::from(*px), i32::from(*py));
                }
                out.extend(flags);
                out.extend(data);
                write_u255(pushes.len(), &mut out);
                write_u255(rest.len(), &mut out);
                write_pushes(&pushes, push);
                code.extend(rest);
            }
            Glyph::Composite {
                bounds,
                components,
                pushes,
                code: rest,
            } => {
                out.extend((-1_i16).to_be_bytes());
                for value in bounds {
                    out.extend(value.to_be_bytes());
                }
                out.extend(&components);
                let mut whole = vec![0; 10];
                whole.extend(&components);
                if components_end(&whole).1 & 0x100 != 0 {
                    write_u255(pushes.len(), &mut out);
                    write_u255(rest.len(), &mut out);
                    write_pushes(&pushes, push);
                    code.extend(rest);
                }
            }
        }
    }
    out
}

fn ctf_cvt(data: &[u8]) -> Vec<u8> {
    let mut out = u16::try_from(data.len() / 2)
        .unwrap()
        .to_be_bytes()
        .to_vec();
    let mut previous = 0_i16;
    for pair in data.as_chunks::<2>().0 {
        let value = i16::from_be_bytes([pair[0], pair[1]]);
        let delta = value.wrapping_sub(previous);
        previous = value;
        let magnitude = i32::from(delta).unsigned_abs();
        let index = magnitude / 238;
        if delta >= 0 && magnitude < 238 {
            out.push(u8::try_from(magnitude).unwrap());
        } else if delta > 0 && index <= 8 {
            out.extend([
                u8::try_from(247 + index).unwrap(),
                u8::try_from(magnitude - index * 238).unwrap(),
            ]);
        } else if delta < 0 && index <= 8 && delta != i16::MIN {
            out.extend([
                u8::try_from(239 + index).unwrap(),
                u8::try_from(magnitude - index * 238).unwrap(),
            ]);
        } else {
            out.push(238);
            out.extend(delta.to_be_bytes());
        }
    }
    out
}

/// Bits from the low bit of each byte, for the `hdmx` and `VDMX` errors.
pub(super) struct Magnitudes {
    pub(super) bytes: Vec<u8>,
    bit: usize,
}

impl Magnitudes {
    pub(super) fn new() -> Self {
        Self {
            bytes: Vec::new(),
            bit: 0,
        }
    }

    fn bit(&mut self, value: bool) {
        if self.bit.is_multiple_of(8) {
            self.bytes.push(0);
        }
        if value {
            *self.bytes.last_mut().unwrap() |= 1 << (self.bit % 8);
        }
        self.bit += 1;
    }

    pub(super) fn write(&mut self, value: i32) {
        for _ in 0..value.unsigned_abs() {
            self.bit(true);
        }
        self.bit(false);
        if value != 0 {
            self.bit(value < 0);
        }
    }
}
