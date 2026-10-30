// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The drawing records Word and `PowerPoint` 97-2003 share ([MS-ODRAW]): records and
//! containers, the image store with its blips, and the property tables of shapes.

use crate::compound::{u16_at, u32_at};

const HEADER_SIZE: usize = 8;
pub(crate) const CONTAINER_VERSION: u16 = 0xF;
pub(crate) const GROUP_CONTAINER: u16 = 0xF000;
pub(crate) const STORE_CONTAINER: u16 = 0xF001;
pub(crate) const DRAWING_CONTAINER: u16 = 0xF002;
pub(crate) const SHAPE_GROUP_CONTAINER: u16 = 0xF003;
pub(crate) const SHAPE_CONTAINER: u16 = 0xF004;
pub(crate) const STORE_ENTRY: u16 = 0xF007;
pub(crate) const GROUP_SHAPE: u16 = 0xF009;
pub(crate) const SHAPE: u16 = 0xF00A;
pub(crate) const PROPERTIES: u16 = 0xF00B;
pub(crate) const CLIENT_TEXTBOX: u16 = 0xF00D;
pub(crate) const CHILD_ANCHOR: u16 = 0xF00F;
pub(crate) const CLIENT_ANCHOR: u16 = 0xF010;
pub(crate) const CLIENT_DATA: u16 = 0xF011;
/// The properties later versions add, the position ones among them.
pub(crate) const TERTIARY_PROPERTIES: u16 = 0xF122;

/// A record: its version, instance, type and body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Record<'a> {
    pub(crate) version: u16,
    pub(crate) instance: u16,
    pub(crate) kind: u16,
    pub(crate) body: &'a [u8],
}

impl Record<'_> {
    pub(crate) fn is_container(&self) -> bool {
        self.version == CONTAINER_VERSION
    }

    /// The length of the record with its header.
    pub(crate) fn length(&self) -> usize {
        HEADER_SIZE + self.body.len()
    }
}

/// The records one after another in `bytes`, up to the first that does not fit.
pub(crate) fn records(bytes: &[u8]) -> impl Iterator<Item = Record<'_>> {
    let mut position = 0;
    std::iter::from_fn(move || {
        let first = u16_at(bytes, position)?;
        let kind = u16_at(bytes, position + 2)?;
        let length = u32_at(bytes, position + 4)? as usize;
        let start = position + HEADER_SIZE;
        let body = bytes.get(start..start.checked_add(length)?)?;
        position = start + length;
        Some(Record {
            version: first & 0x000F,
            instance: first >> 4,
            kind,
            body,
        })
    })
}

/// The first child of a container that has a type.
pub(crate) fn child(body: &[u8], kind: u16) -> Option<Record<'_>> {
    records(body).find(|record| record.kind == kind)
}

/// The image bytes of a blip record; metafiles, which nothing here draws, are left out.
pub(crate) fn blip(record: Record<'_>) -> Option<Vec<u8>> {
    let identifiers = match (record.kind, record.instance) {
        (0xF01D | 0xF02A, 0x46B | 0x6E3) | (0xF01E, 0x6E1) | (0xF01F, 0x7A9) | (0xF029, 0x6E5) => 2,
        (0xF01D | 0xF01E | 0xF01F | 0xF029 | 0xF02A, _) => 1,
        _ => return None,
    };
    let start = 16 * identifiers + 1;
    let bytes = record.body.get(start..)?;
    if record.kind == 0xF01F {
        return Some(bitmap_file(bytes));
    }
    Some(bytes.to_vec())
}

/// A device-independent bitmap with the file header a `.bmp` begins with.
pub(crate) fn bitmap_file(dib: &[u8]) -> Vec<u8> {
    let header_length = u32_at(dib, 0).unwrap_or(40);
    let bit_count = u16_at(dib, 14).unwrap_or(24);
    let colours = match u32_at(dib, 32).unwrap_or(0) {
        0 if bit_count <= 8 => 1_u32 << bit_count,
        used => used,
    };
    let pixels_offset = 14 + header_length + colours * 4;
    let total = u32::try_from(dib.len() + 14).unwrap_or(u32::MAX);
    let mut file = Vec::with_capacity(dib.len() + 14);
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&total.to_le_bytes());
    file.extend_from_slice(&[0; 4]);
    file.extend_from_slice(&pixels_offset.to_le_bytes());
    file.extend_from_slice(dib);
    file
}

/// The image of a store entry (`FBSE`): the blip it embeds, or the one at its offset in the
/// delay stream.
pub(crate) fn store_entry(record: Record<'_>, delay: &[u8]) -> Option<Vec<u8>> {
    let name_length = usize::from(*record.body.get(33)?);
    let embedded = record
        .body
        .get(36 + name_length..)
        .filter(|rest| !rest.is_empty());
    if let Some(found) = embedded
        .and_then(|rest| records(rest).next())
        .and_then(blip)
    {
        return Some(found);
    }
    let size = u32_at(record.body, 20)? as usize;
    let offset = u32_at(record.body, 28)? as usize;
    let delayed = delay.get(offset..offset.checked_add(size)?)?;
    records(delayed).next().and_then(blip)
}

/// The images of a store container, in order, `None` for an entry that holds none.
pub(crate) fn store_images(store: &[u8], delay: &[u8]) -> Vec<Option<Vec<u8>>> {
    records(store)
        .filter(|entry| entry.kind == STORE_ENTRY)
        .map(|entry| store_entry(entry, delay))
        .collect()
}

/// The simple properties of an `OfficeArtFOPT` or `OfficeArtTertiaryFOPT`, by their number
/// without the blip and complex flags.
pub(crate) fn properties(record: Record<'_>) -> impl Iterator<Item = (u16, u32)> + '_ {
    (0..usize::from(record.instance)).map_while(move |index| {
        let identifier = u16_at(record.body, index * 6)?;
        let value = u32_at(record.body, index * 6 + 2)?;
        Some((identifier & 0x3FFF, value))
    })
}

/// The complex properties that hold an `IMsoArray`: the vertices, segments, adjust handles,
/// guides, inscribed rectangles and connection sites of a geometry, the shade colours of a
/// fill, a line's dashes and a wrap polygon.
const ARRAY_PROPERTIES: [u16; 9] = [
    0x0145, 0x0146, 0x0155, 0x0156, 0x0157, 0x0158, 0x0197, 0x01CF, 0x0383,
];

/// The data of each complex property of a table, by its number: the data follows the simple
/// entries in the order the entries name it, each entry's value giving its length.
pub(crate) fn complex_properties(record: Record<'_>) -> Vec<(u16, &[u8])> {
    let count = usize::from(record.instance);
    let mut position = count * 6;
    let mut found = Vec::new();
    for index in 0..count {
        let (Some(identifier), Some(length)) = (
            u16_at(record.body, index * 6),
            u32_at(record.body, index * 6 + 2),
        ) else {
            break;
        };
        if identifier & 0x8000 == 0 {
            continue;
        }
        // An array's length leaves out the six bytes of its header.
        let length = if ARRAY_PROPERTIES.contains(&(identifier & 0x3FFF)) {
            length as usize + 6
        } else {
            length as usize
        };
        let Some(data) = record.body.get(position..position.saturating_add(length)) else {
            break;
        };
        found.push((identifier & 0x3FFF, data));
        position += data.len();
    }
    found
}

/// The elements of an `IMsoArray`: a count, a capacity and the size of each element, which is
/// four when the size says `0xFFF0`.
pub(crate) fn array_elements(data: &[u8]) -> Vec<&[u8]> {
    let (Some(count), Some(size)) = (u16_at(data, 0), u16_at(data, 4)) else {
        return Vec::new();
    };
    let size = if size == 0xFFF0 { 4 } else { usize::from(size) };
    if size == 0 {
        return Vec::new();
    }
    data.get(6..)
        .unwrap_or_default()
        .chunks_exact(size)
        .take(usize::from(count))
        .collect()
}

/// A boolean property set: the flag at `bit`, when its "use" bit sixteen places above is set.
pub(crate) fn flag(value: u32, bit: u32) -> Option<bool> {
    (value & (1 << (bit + 16)) != 0).then_some(value & (1 << bit) != 0)
}

/// The adjust values of the `DrawingML` preset of a shape type from the ones a binary file
/// sets (`adjustValue` to `adjust10Value`, in the type's 21,600 units): `None` when it sets
/// none, or for a type whose handles do not map onto its preset's, which then keeps the
/// preset's defaults. A value left unset takes the type's own default.
#[must_use]
pub(crate) fn drawingml_adjustments(
    kind: u16,
    adjust: &[Option<i64>; 10],
    width: i64,
    height: i64,
) -> Option<Vec<f64>> {
    if adjust.iter().all(Option::is_none) {
        return None;
    }
    let defaults = binary_defaults(kind)?;
    #[expect(clippy::cast_precision_loss, reason = "adjust values are small")]
    let value = |index: usize| adjust[index].unwrap_or(defaults[index]) as f64;
    let share = |units: f64| units * 100_000.0 / 21_600.0;
    #[expect(clippy::cast_precision_loss, reason = "frame sizes fit in f64")]
    let (width, height) = (width.max(1) as f64, height.max(1) as f64);
    let shortest = width.min(height);
    let mean = f64::midpoint(width, height);
    // A length along one side, as a share of the shorter side the preset measures it by.
    let along = |units: f64, side: f64| share(units) * side / shortest;
    // An angle in sixteenths of a sixteenth of a degree, as the preset's 60,000ths of a degree,
    // both measured clockwise from the right.
    let angle = |units: f64| (units / 65_536.0).rem_euclid(360.0) * 60_000.0;
    // The inner radius of a star, from the inset of its inner vertices.
    let star = |units: f64| (10_800.0 - units) / 10_800.0 * 50_000.0;
    Some(match kind {
        61..=63 | 17 | 106 => vec![share(value(0) - 10_800.0), share(value(1) - 10_800.0)],
        13 | 14 | 93 | 94 => vec![
            share(21_600.0 - 2.0 * value(1)),
            along(21_600.0 - value(0), width),
        ],
        66 | 69 => vec![share(21_600.0 - 2.0 * value(1)), along(value(0), width)],
        67 => vec![
            share(21_600.0 - 2.0 * value(1)),
            along(21_600.0 - value(0), height),
        ],
        68 => vec![share(21_600.0 - 2.0 * value(1)), along(value(0), height)],
        70 => vec![share(21_600.0 - 2.0 * value(0)), along(value(1), height)],
        76 => vec![
            along(21_600.0 - 2.0 * value(1), mean),
            along(10_800.0 - value(0), mean),
            along(value(2), mean),
        ],
        77 => vec![
            along(21_600.0 - 2.0 * value(3), height),
            along(10_800.0 - value(1), height),
            along(value(2), width),
            share(21_600.0 - value(0)),
        ],
        78 => vec![
            along(21_600.0 - 2.0 * value(3), height),
            along(10_800.0 - value(1), height),
            along(21_600.0 - value(2), width),
            share(value(0)),
        ],
        79 => vec![
            along(21_600.0 - 2.0 * value(3), width),
            along(10_800.0 - value(1), width),
            along(value(2), height),
            share(21_600.0 - value(0)),
        ],
        80 => vec![
            along(21_600.0 - 2.0 * value(3), width),
            along(10_800.0 - value(1), width),
            along(21_600.0 - value(2), height),
            share(value(0)),
        ],
        81 => vec![
            along(21_600.0 - 2.0 * value(3), height),
            along(10_800.0 - value(1), height),
            along(value(2), width),
            share(21_600.0 - 2.0 * value(0)),
        ],
        82 => vec![
            along(21_600.0 - 2.0 * value(3), width),
            along(10_800.0 - value(1), width),
            along(value(2), height),
            share(21_600.0 - 2.0 * value(0)),
        ],
        // A line callout's points, the far end first in the binary file and last in the
        // preset, each a share of the frame across and down.
        41 | 44 | 47 | 50 | 178..=181 | 42 | 45 | 48 | 51 | 43 | 46 | 49 | 52 => {
            let points = defaults.len() / 2;
            (0..points)
                .rev()
                .flat_map(|point| [share(value(point * 2 + 1)), share(value(point * 2))])
                .collect()
        }
        15 | 55 => vec![along(21_600.0 - value(0), width)],
        8 => vec![along(value(0), width)],
        18 | 58 | 59 | 60 | 92 | 187 => vec![star(value(0))],
        85 | 86 => vec![along(value(0), height)],
        87 | 88 => vec![along(value(0), height), share(value(1))],
        185 | 186 | 57 | 97 | 98 => vec![along(value(0), mean)],
        65 => vec![along(21_600.0 - value(0), mean)],
        96 => vec![(value(0) - 16_515.0) * 4_653.0 / 1_005.0],
        64 | 188 => vec![share(value(0)), share(value(1) - 10_800.0)],
        53 => vec![share(21_600.0 - value(1)), share(21_600.0 - 2.0 * value(0))],
        54 => vec![share(value(1)), share(21_600.0 - 2.0 * value(0))],
        19 => vec![angle(value(0)), angle(value(1))],
        95 => vec![
            angle(value(0)),
            angle(180.0 * 65_536.0 - value(0)),
            share(value(1)),
        ],
        _ => (0..defaults.len())
            .map(|index| share(value(index)))
            .collect(),
    })
}

/// The default adjust values of the shape types whose handles map onto their preset's.
fn binary_defaults(kind: u16) -> Option<&'static [i64]> {
    Some(match kind {
        2 | 21 | 185 => &[3_600],
        5 | 184 | 34 | 38 => &[10_800],
        7 | 9 | 11 | 16 | 22 | 23 | 8 | 183 => &[5_400],
        10 => &[6_326],
        84 | 18 | 59 | 60 | 92 | 57 | 97 | 98 => &[2_700],
        61..=63 | 17 | 106 => &[1_350, 25_920],
        13 | 14 | 67 | 93 | 94 => &[16_200, 5_400],
        66 | 68 => &[5_400, 5_400],
        69 => &[4_320, 5_400],
        70 => &[5_400, 4_320],
        76 => &[6_480, 8_640, 4_320],
        77 | 79 => &[7_200, 5_400, 3_600, 8_100],
        78 | 80 => &[14_400, 5_400, 18_000, 8_100],
        81 | 82 => &[5_400, 5_400, 2_700, 8_100],
        41 | 44 | 47 | 50 | 178..=181 => &[-8_280, 24_300, -1_800, 4_050],
        42 | 45 | 48 | 51 => &[-10_080, 24_300, -3_600, 4_050, -1_800, 4_050],
        43 | 46 | 49 | 52 => &[23_400, 24_400, 25_200, 21_600, 25_200, 4_050, 23_400, 4_050],
        15 | 55 => &[16_200],
        58 => &[2_538],
        187 => &[8_100],
        85 | 86 | 186 => &[1_800],
        87 | 88 => &[1_800, 10_800],
        65 => &[18_900],
        96 => &[17_520],
        64 => &[2_700, 10_800],
        188 => &[1_404, 10_800],
        53 => &[5_400, 18_900],
        54 => &[5_400, 2_700],
        19 => &[-5_898_240, 0],
        95 => &[11_796_480, 5_400],
        35 | 39 => &[10_800, 10_800],
        36 | 40 => &[10_800, 10_800, 10_800],
        _ => return None,
    })
}

/// Whether a shape type is drawn upside down from its `DrawingML` preset: the binary trapezoid
/// narrows at the bottom, the preset's at the top.
#[must_use]
pub(crate) fn upside_down_from_preset(kind: u16) -> bool {
    kind == 8
}

/// The `DrawingML` preset that draws a shape type (`MSOSPT`) the same; `None` for a rectangle
/// and for the types with no outline of their own.
#[expect(
    clippy::too_many_lines,
    reason = "one arm per shape type of the binary format"
)]
pub(crate) fn preset_of_shape_type(kind: u16) -> Option<&'static str> {
    Some(match kind {
        2 => "roundRect",
        4 => "diamond",
        5 => "triangle",
        6 => "rtTriangle",
        7 => "parallelogram",
        8 => "trapezoid",
        9 => "hexagon",
        10 => "octagon",
        11 => "plus",
        12 => "star5",
        13 | 14 => "rightArrow",
        15 => "homePlate",
        16 => "cube",
        17 | 62 => "wedgeRoundRectCallout",
        18 | 59 => "star16",
        19 => "arc",
        21 => "plaque",
        22 => "can",
        23 => "donut",
        33 => "bentConnector2",
        34 => "bentConnector3",
        35 => "bentConnector4",
        36 => "bentConnector5",
        37 => "curvedConnector2",
        38 => "curvedConnector3",
        39 => "curvedConnector4",
        40 => "curvedConnector5",
        41 | 178 => "callout1",
        42 => "callout2",
        43 => "callout3",
        44 | 179 => "accentCallout1",
        45 => "accentCallout2",
        46 => "accentCallout3",
        47 | 180 => "borderCallout1",
        48 => "borderCallout2",
        49 => "borderCallout3",
        50 | 181 => "accentBorderCallout1",
        51 => "accentBorderCallout2",
        52 => "accentBorderCallout3",
        53 => "ribbon2",
        54 => "ribbon",
        55 => "chevron",
        56 => "pentagon",
        57 => "noSmoking",
        58 => "star8",
        60 => "star32",
        61 => "wedgeRectCallout",
        63 => "wedgeEllipseCallout",
        64 => "wave",
        65 => "foldedCorner",
        66 => "leftArrow",
        67 => "downArrow",
        68 => "upArrow",
        69 => "leftRightArrow",
        70 => "upDownArrow",
        71 => "irregularSeal1",
        72 => "irregularSeal2",
        73 => "lightningBolt",
        74 => "heart",
        75 => "frame",
        76 => "quadArrow",
        77 => "leftArrowCallout",
        78 => "rightArrowCallout",
        79 => "upArrowCallout",
        80 => "downArrowCallout",
        81 => "leftRightArrowCallout",
        82 => "upDownArrowCallout",
        83 => "quadArrowCallout",
        84 => "bevel",
        85 => "leftBracket",
        86 => "rightBracket",
        87 => "leftBrace",
        88 => "rightBrace",
        89 => "leftUpArrow",
        90 => "bentUpArrow",
        91 => "bentArrow",
        92 => "star24",
        93 => "stripedRightArrow",
        94 => "notchedRightArrow",
        95 => "blockArc",
        96 => "smileyFace",
        97 => "verticalScroll",
        98 => "horizontalScroll",
        99 | 100 => "circularArrow",
        101 => "uturnArrow",
        102 => "curvedRightArrow",
        103 => "curvedLeftArrow",
        104 => "curvedUpArrow",
        105 => "curvedDownArrow",
        106 => "cloudCallout",
        107 => "ellipseRibbon",
        108 => "ellipseRibbon2",
        110 => "flowChartDecision",
        111 => "flowChartInputOutput",
        112 => "flowChartPredefinedProcess",
        113 => "flowChartInternalStorage",
        114 => "flowChartDocument",
        115 => "flowChartMultidocument",
        116 => "flowChartTerminator",
        117 => "flowChartPreparation",
        118 => "flowChartManualInput",
        119 => "flowChartManualOperation",
        121 => "flowChartPunchedCard",
        122 => "flowChartPunchedTape",
        123 => "flowChartSummingJunction",
        124 => "flowChartOr",
        125 => "flowChartCollate",
        126 => "flowChartSort",
        127 => "flowChartExtract",
        128 => "flowChartMerge",
        129 => "flowChartOfflineStorage",
        130 => "flowChartOnlineStorage",
        131 => "flowChartMagneticTape",
        132 => "flowChartMagneticDisk",
        133 => "flowChartMagneticDrum",
        134 => "flowChartDisplay",
        135 => "flowChartDelay",
        176 => "flowChartAlternateProcess",
        177 => "flowChartOffpageConnector",
        182 => "leftRightUpArrow",
        183 => "sun",
        184 => "moon",
        185 => "bracketPair",
        186 => "bracePair",
        187 => "star4",
        188 => "doubleWave",
        189 => "actionButtonBlank",
        190 => "actionButtonHome",
        191 => "actionButtonHelp",
        192 => "actionButtonInformation",
        193 => "actionButtonForwardNext",
        194 => "actionButtonBackPrevious",
        195 => "actionButtonEnd",
        196 => "actionButtonBeginning",
        197 => "actionButtonReturn",
        198 => "actionButtonDocument",
        199 => "actionButtonSound",
        200 => "actionButtonMovie",
        _ => return None,
    })
}

pub(crate) mod freeform;
mod legacy_shapes;

pub(crate) use legacy_shapes::legacy_shape_paths;

#[cfg(test)]
mod tests;
