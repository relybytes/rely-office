// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! `LZCOMP`, the compressor of each block of a `MicroType` Express font (the W3C member
//! submission, section 3 and appendix C): a variation of LZ77 whose literals, copy lengths and
//! copy distances go through three adaptive Huffman trees, over a history that starts with 8,192
//! bytes known to both sides, optionally behind a run-length layer.

/// The bytes both sides put before the data, so a copy can reach them from the first byte.
pub(super) const PRELOAD_SIZE: usize = 2 * 32 * 96 + 4 * 256;
/// The first copy distance whose shortest copy is three bytes rather than two.
pub(super) const MAX_TWO_BYTE_DISTANCE: usize = 512;
/// The width, in bits, of each symbol of a distance and of a copy length's first symbol.
pub(super) const DISTANCE_WIDTH: u32 = 3;
/// The root of every adaptive Huffman tree.
const ROOT: usize = 1;

/// The bits of a block, from the most significant bit of each byte.
pub(super) struct BitReader<'a> {
    bytes: &'a [u8],
    index: usize,
    buffer: u8,
    left: u8,
}

impl<'a> BitReader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            index: 0,
            buffer: 0,
            left: 0,
        }
    }

    pub(super) fn bit(&mut self) -> Option<bool> {
        if self.left == 0 {
            self.buffer = *self.bytes.get(self.index)?;
            self.index += 1;
            self.left = 8;
        }
        self.left -= 1;
        Some((self.buffer >> self.left) & 1 == 1)
    }

    pub(super) fn value(&mut self, bits: u32) -> Option<u32> {
        let mut value = 0;
        for _ in 0..bits {
            value = (value << 1) | u32::from(self.bit()?);
        }
        Some(value)
    }
}

/// A node of an adaptive Huffman tree; `code` is the symbol of a leaf, `None` inside.
#[derive(Clone, Copy)]
pub(super) struct Node {
    pub(super) up: usize,
    pub(super) left: usize,
    pub(super) right: usize,
    pub(super) code: Option<u16>,
    pub(super) weight: u32,
}

/// An adaptive Huffman tree over the symbols `0..range`, kept in the sibling order: the nodes by
/// index hold decreasing weights, so a node that gains weight swaps with the first of its equals.
pub(super) struct Ahuff {
    pub(super) tree: Vec<Node>,
    pub(super) leaf: Vec<usize>,
}

impl Ahuff {
    /// The tree both sides start from, its weights moved as `MTX_AHUFF_Create` moves them.
    pub(super) fn new(range: u16) -> Self {
        let range = usize::from(range);
        let mut tree = vec![
            Node {
                up: 0,
                left: 0,
                right: 0,
                code: None,
                weight: 0,
            };
            2 * range
        ];
        for (index, node) in tree.iter_mut().enumerate().skip(2) {
            node.up = index / 2;
            node.weight = 1;
        }
        for (index, node) in tree.iter_mut().enumerate().take(range).skip(1) {
            node.left = 2 * index;
            node.right = 2 * index + 1;
        }
        let mut leaf = Vec::with_capacity(range);
        for symbol in 0..range {
            tree[range + symbol].code = u16::try_from(symbol).ok();
            leaf.push(range + symbol);
        }
        for index in (ROOT..range).rev() {
            tree[index].weight = tree[tree[index].left].weight + tree[tree[index].right].weight;
        }
        let mut ahuff = Self { tree, leaf };
        if range > 256 && range < 512 {
            ahuff.update(ahuff.leaf[256]);
            ahuff.update(ahuff.leaf[257]);
            for _ in 0..12 {
                ahuff.update(ahuff.leaf[range - 3]);
            }
            for _ in 0..6 {
                ahuff.update(ahuff.leaf[range - 2]);
            }
        } else {
            for _ in 0..2 {
                for symbol in 0..range {
                    ahuff.update(ahuff.leaf[symbol]);
                }
            }
        }
        ahuff
    }

    /// The next symbol, read from the root down one bit at a time.
    pub(super) fn read(&mut self, bits: &mut BitReader<'_>) -> Option<u16> {
        let mut node = ROOT;
        loop {
            node = if bits.bit()? {
                self.tree[node].right
            } else {
                self.tree[node].left
            };
            if let Some(symbol) = self.tree.get(node)?.code {
                self.update(node);
                return Some(symbol);
            }
        }
    }

    /// One more of the symbol at `node`, its ancestors and the sibling order kept.
    pub(super) fn update(&mut self, mut node: usize) {
        while node != ROOT {
            let weight = self.tree[node].weight;
            if self.tree[node - 1].weight == weight {
                let mut first = node - 1;
                while first > 0 && self.tree[first - 1].weight == weight {
                    first -= 1;
                }
                if first > ROOT {
                    self.swap(node, first);
                    node = first;
                }
            }
            self.tree[node].weight = weight + 1;
            node = self.tree[node].up;
        }
        self.tree[ROOT].weight += 1;
    }

    fn swap(&mut self, a: usize, b: usize) {
        let (up_a, up_b) = (self.tree[a].up, self.tree[b].up);
        self.tree.swap(a, b);
        self.tree[a].up = up_a;
        self.tree[b].up = up_b;
        for node in [a, b] {
            let Node {
                left, right, code, ..
            } = self.tree[node];
            match code {
                None => {
                    self.tree[left].up = node;
                    self.tree[right].up = node;
                }
                Some(symbol) => self.leaf[usize::from(symbol)] = node,
            }
        }
    }
}

/// How many distance symbols the longest copy of a block needs: the fewest whose reach covers
/// its length.
pub(super) fn distance_ranges(length: usize) -> u32 {
    let mut ranges = 1;
    while (1_usize << (DISTANCE_WIDTH * ranges)) < length {
        ranges += 1;
    }
    ranges
}

/// The block uncompressed, no longer than `max_bytes`; `None` when it is malformed. A block
/// of no bytes is no data.
pub(super) fn decompress(block: &[u8], version: u8, max_bytes: usize) -> Option<Vec<u8>> {
    if block.is_empty() {
        return Some(Vec::new());
    }
    let mut bits = BitReader::new(block);
    let run_length = version != 1 && bits.bit()?;
    let mut distances = Ahuff::new(1 << DISTANCE_WIDTH);
    let mut lengths = Ahuff::new(1 << DISTANCE_WIDTH);
    let length = bits.value(24)? as usize;
    if length > max_bytes {
        return None;
    }
    let ranges = distance_ranges(length);
    let dup2 = 256 + 8 * ranges;
    let mut symbols = Ahuff::new(u16::try_from(dup2 + 3).ok()?);
    let (dup2, dup4, dup6) = (
        u16::try_from(dup2).ok()?,
        u16::try_from(dup2 + 1).ok()?,
        u16::try_from(dup2 + 2).ok()?,
    );

    let mut history = preload();
    history.reserve(length);
    let end = PRELOAD_SIZE + length;
    while history.len() < end {
        let symbol = symbols.read(&mut bits)?;
        let at = history.len();
        let byte = match symbol {
            literal @ 0..=255 => u8::try_from(literal).ok()?,
            _ if symbol == dup2 => history[at - 2],
            _ if symbol == dup4 => history[at - 4],
            _ if symbol == dup6 => history[at - 6],
            _ => {
                let (mut copy, used) = copy_length(symbol, &mut lengths, &mut bits, length)?;
                if used > ranges {
                    return None;
                }
                let mut distance = 0_usize;
                for _ in 0..used {
                    distance =
                        (distance << DISTANCE_WIDTH) | usize::from(distances.read(&mut bits)?);
                }
                distance += 1;
                if distance >= MAX_TWO_BYTE_DISTANCE {
                    copy += 1;
                }
                let start = (at + 1).checked_sub(distance + copy)?;
                if at + copy > end {
                    return None;
                }
                for index in start..start + copy {
                    history.push(history[index]);
                }
                continue;
            }
        };
        history.push(byte);
    }
    let data = history.split_off(PRELOAD_SIZE);
    if run_length {
        run_length_decode(&data, max_bytes)
    } else {
        Some(data)
    }
}

/// The 8,192 bytes before the data: every pair of a byte under 32 and one under 96, then every
/// byte four times.
pub(super) fn preload() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(PRELOAD_SIZE);
    for high in 0..32_u8 {
        for low in 0..96_u8 {
            bytes.extend([high, low]);
        }
    }
    for byte in 0..=255_u8 {
        bytes.extend([byte; 4]);
    }
    bytes
}

/// The length of a copy item and how many distance symbols follow: its first two bits and the
/// count of distance symbols come with the symbol, more bits from the length tree two at a time
/// until the stop bit.
fn copy_length(
    symbol: u16,
    lengths: &mut Ahuff,
    bits: &mut BitReader<'_>,
    limit: usize,
) -> Option<(usize, u32)> {
    let first = usize::from(symbol - 256);
    let ranges = u32::try_from(first / 8 + 1).ok()?;
    let mut chunk = first % 8;
    let mut value = 0_usize;
    loop {
        value = (value << 2) | (chunk & 3);
        if value > limit {
            return None;
        }
        if chunk & 4 == 0 {
            break;
        }
        chunk = usize::from(lengths.read(bits)?);
    }
    Some((value + 2, ranges))
}

/// The run-length layer taken off: the first byte is the escape, an escape and a count of zero
/// is the escape itself, an escape, a count and a byte is that byte repeated.
fn run_length_decode(data: &[u8], max_bytes: usize) -> Option<Vec<u8>> {
    let (&escape, mut rest) = data.split_first()?;
    let mut out = Vec::with_capacity(data.len());
    while let Some((&byte, tail)) = rest.split_first() {
        rest = tail;
        if byte == escape {
            let (&count, tail) = rest.split_first()?;
            rest = tail;
            if count == 0 {
                out.push(escape);
            } else {
                let (&repeated, tail) = rest.split_first()?;
                rest = tail;
                out.extend(std::iter::repeat_n(repeated, usize::from(count)));
            }
        } else {
            out.push(byte);
        }
        if out.len() > max_bytes {
            return None;
        }
    }
    Some(out)
}
