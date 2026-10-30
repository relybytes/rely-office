// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The fill handle: dragging a selection over its neighbours continues it as a spreadsheet does.
//! Numbers follow their trend, a date goes on a day at a time, a text ending in a number counts
//! on, month and weekday names follow the calendar, formulas move their relative references and
//! anything else repeats.

use std::collections::BTreeMap;

use crate::address::{CellRef, MAX_COLUMNS, MAX_ROWS};
use crate::model::{Cell, EditError, MAX_CELLS, NumberFormat, Workbook};
use crate::value::Input;

/// The most cells one fill writes.
pub const MAX_FILL_CELLS: usize = 1_000_000;

const NAME_LISTS: [&[&str]; 8] = [
    &[
        "gennaio",
        "febbraio",
        "marzo",
        "aprile",
        "maggio",
        "giugno",
        "luglio",
        "agosto",
        "settembre",
        "ottobre",
        "novembre",
        "dicembre",
    ],
    &[
        "gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic",
    ],
    &[
        "lunedì",
        "martedì",
        "mercoledì",
        "giovedì",
        "venerdì",
        "sabato",
        "domenica",
    ],
    &["lun", "mar", "mer", "gio", "ven", "sab", "dom"],
    &[
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ],
    &[
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ],
    &[
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
    ],
    &["mon", "tue", "wed", "thu", "fri", "sat", "sun"],
];

/// Which way a fill runs from its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Down,
    Up,
    Right,
    Left,
}

/// How the cells of one line of the source go on.
#[derive(Debug, Clone, PartialEq)]
enum Series {
    /// A straight line through the numbers: the value at a step counted from the first.
    Trend { start: f64, step: f64 },
    /// A text before and after a whole number that counts on.
    Counter {
        prefix: String,
        suffix: String,
        start: i64,
        step: i64,
        width: usize,
    },
    /// The names of a list, from an index of it.
    Names {
        list: usize,
        start: i64,
        step: i64,
        case: NameCase,
    },
    /// The cells as they are, again and again.
    Repeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameCase {
    Lower,
    Capital,
    Upper,
}

fn name_case(text: &str) -> NameCase {
    if text.chars().all(|character| !character.is_lowercase()) {
        NameCase::Upper
    } else if text.chars().next().is_some_and(char::is_uppercase) {
        NameCase::Capital
    } else {
        NameCase::Lower
    }
}

fn cased(name: &str, case: NameCase) -> String {
    match case {
        NameCase::Lower => name.to_owned(),
        NameCase::Upper => name.to_uppercase(),
        NameCase::Capital => {
            let mut characters = name.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        }
    }
}

/// A text split around its last whole number: `Lotto 07` is `("Lotto ", 7, "", 2)`.
fn split_counter(text: &str) -> Option<(String, i64, String, usize)> {
    let end = text.rfind(|character: char| character.is_ascii_digit())? + 1;
    let start = text[..end]
        .rfind(|character: char| !character.is_ascii_digit())
        .map_or(0, |index| index + 1);
    let digits = &text[start..end];
    if digits.len() > 15 {
        return None;
    }
    Some((
        text[..start].to_owned(),
        digits.parse().ok()?,
        text[end..].to_owned(),
        digits.len(),
    ))
}

/// The list and index of a month or weekday name; the first list that knows it wins.
fn find_name(text: &str) -> Option<(usize, i64)> {
    let lower = text.trim().to_lowercase();
    NAME_LISTS.iter().enumerate().find_map(|(list, names)| {
        names
            .iter()
            .position(|name| *name == lower)
            .map(|index| (list, i64::try_from(index).unwrap_or(0)))
    })
}

/// The series a line of source cells makes; dated numbers count days.
fn series_of(cells: &[Option<&Cell>]) -> Series {
    let inputs: Vec<&Input> = cells.iter().flatten().map(|cell| &cell.input).collect();
    if inputs.len() != cells.len() || inputs.is_empty() {
        return Series::Repeat;
    }
    if let Some(numbers) = inputs
        .iter()
        .map(|input| match input {
            Input::Number(number) => Some(*number),
            _ => None,
        })
        .collect::<Option<Vec<f64>>>()
    {
        if numbers.len() == 1 {
            let dated = cells[0].is_some_and(|cell| {
                matches!(cell.format, NumberFormat::Date | NumberFormat::DateTime)
            });
            return if dated {
                Series::Trend {
                    start: numbers[0],
                    step: 1.0,
                }
            } else {
                Series::Repeat
            };
        }
        let (start, step) = trend(&numbers);
        return Series::Trend { start, step };
    }
    let texts: Option<Vec<&str>> = inputs
        .iter()
        .map(|input| match input {
            Input::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let Some(texts) = texts else {
        return Series::Repeat;
    };
    if let Some(found) = texts
        .iter()
        .map(|text| find_name(text))
        .collect::<Option<Vec<_>>>()
        && found.iter().all(|(list, _)| *list == found[0].0)
    {
        let step = match found.as_slice() {
            [.., before, last] => last.1 - before.1,
            _ => 1,
        };
        return Series::Names {
            list: found[0].0,
            start: found[0].1,
            step,
            case: name_case(texts[0]),
        };
    }
    if let Some(split) = texts
        .iter()
        .map(|text| split_counter(text))
        .collect::<Option<Vec<_>>>()
        && split
            .iter()
            .all(|(prefix, _, suffix, _)| *prefix == split[0].0 && *suffix == split[0].2)
    {
        let step = match split.as_slice() {
            [.., before, last] => last.1 - before.1,
            _ => 1,
        };
        return Series::Counter {
            prefix: split[0].0.clone(),
            suffix: split[0].2.clone(),
            start: split[0].1,
            step,
            width: split[0].3,
        };
    }
    Series::Repeat
}

/// The least-squares line through values at steps 0, 1, 2...: its value at 0 and its slope.
fn trend(values: &[f64]) -> (f64, f64) {
    #[expect(clippy::cast_precision_loss, reason = "a fill line is short")]
    let count = values.len() as f64;
    let mean_step = (count - 1.0) / 2.0;
    let mean_value = values.iter().sum::<f64>() / count;
    let (mut covariance, mut variance) = (0.0, 0.0);
    for (index, value) in values.iter().enumerate() {
        #[expect(clippy::cast_precision_loss, reason = "a fill line is short")]
        let offset = index as f64 - mean_step;
        covariance += offset * (value - mean_value);
        variance += offset * offset;
    }
    let slope = if variance == 0.0 {
        0.0
    } else {
        covariance / variance
    };
    (mean_value - slope * mean_step, slope)
}

/// The input at a step of a series counted from the first source cell; `None` repeats.
fn series_input(series: &Series, step: i64) -> Option<Input> {
    match series {
        Series::Trend { start, step: slope } => {
            #[expect(clippy::cast_precision_loss, reason = "a fill step is small")]
            let value = start + slope * step as f64;
            // A trend of whole numbers stays whole, whatever the float error.
            let rounded = (value * 1e9).round() / 1e9;
            Some(Input::Number(rounded))
        }
        Series::Counter {
            prefix,
            suffix,
            start,
            step: increment,
            width,
        } => {
            let value = start.checked_add(increment.checked_mul(step)?)?;
            let digits = if value < 0 {
                format!("-{:0width$}", value.unsigned_abs())
            } else {
                format!("{value:0width$}")
            };
            Some(Input::Text(format!("{prefix}{digits}{suffix}")))
        }
        Series::Names {
            list,
            start,
            step: increment,
            case,
        } => {
            let names = NAME_LISTS[*list];
            let length = i64::try_from(names.len()).ok()?;
            let index = (start + increment * step).rem_euclid(length);
            Some(Input::Text(cased(
                names[usize::try_from(index).ok()?],
                *case,
            )))
        }
        Series::Repeat => None,
    }
}

fn corners(from: (CellRef, CellRef)) -> (u32, u32, u32, u32) {
    let (start, end) = from;
    (
        start.row.min(end.row),
        start.column.min(end.column),
        start.row.max(end.row),
        start.column.max(end.column),
    )
}

/// Which way a fill runs, true when down or up, and the places it writes along that way, as
/// steps from the first source cell; `None` when the target adds nothing.
fn fill_steps(
    source: (u32, u32, u32, u32),
    target: (u32, u32, u32, u32),
) -> Result<Option<(bool, Vec<i64>)>, EditError> {
    let (top, left, bottom, right) = source;
    let (target_top, target_left, target_bottom, target_right) = target;
    if target_bottom >= MAX_ROWS
        || target_right >= MAX_COLUMNS
        || target_top > top
        || target_left > left
        || target_bottom < bottom
        || target_right < right
    {
        return Err(EditError::OutOfBounds);
    }
    let sides = [
        (target_bottom > bottom, Direction::Down),
        (target_top < top, Direction::Up),
        (target_right > right, Direction::Right),
        (target_left < left, Direction::Left),
    ];
    let mut grows = sides.iter().filter(|(grows, _)| *grows);
    let Some((_, direction)) = grows.next() else {
        return Ok(None);
    };
    if grows.next().is_some() {
        return Err(EditError::OutOfBounds);
    }
    let vertical = matches!(direction, Direction::Down | Direction::Up);
    let (length, width) = if vertical {
        (bottom - top + 1, right - left + 1)
    } else {
        (right - left + 1, bottom - top + 1)
    };
    let added = match direction {
        Direction::Down => target_bottom - bottom,
        Direction::Up => top - target_top,
        Direction::Right => target_right - right,
        Direction::Left => left - target_left,
    };
    if u64::from(added) * u64::from(width) > MAX_FILL_CELLS as u64 {
        return Err(EditError::OutOfBounds);
    }
    let steps = (1..=i64::from(added))
        .map(|step| match direction {
            Direction::Down | Direction::Right => step + i64::from(length) - 1,
            Direction::Up | Direction::Left => -step,
        })
        .collect();
    Ok(Some((vertical, steps)))
}

impl Workbook {
    /// Continue the cells of `source` over the rest of `target`, which holds it and reaches past
    /// it on one side only, as the fill handle does: each row or column of the source makes its
    /// own series, and the looks of the source repeat along.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] for a target that does not hold
    /// the source, reaches past it on more than one side, runs off the grid or writes more than
    /// [`MAX_FILL_CELLS`].
    pub fn fill_range(
        &mut self,
        sheet: usize,
        source: (CellRef, CellRef),
        target: (CellRef, CellRef),
    ) -> Result<(), EditError> {
        let content = self
            .sheets
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        let (top, left, bottom, right) = corners(source);
        let Some((vertical, steps)) = fill_steps(corners(source), corners(target))? else {
            return Ok(());
        };
        let (length, lines) = if vertical {
            (bottom - top + 1, left..=right)
        } else {
            (right - left + 1, top..=bottom)
        };
        let place = |line: u32, step: i64| -> Option<CellRef> {
            let along = i64::from(if vertical { top } else { left }) + step;
            let along = u32::try_from(along).ok()?;
            Some(if vertical {
                CellRef::new(along, line)
            } else {
                CellRef::new(line, along)
            })
        };
        let length = i64::from(length);
        let mut written: BTreeMap<CellRef, Option<Cell>> = BTreeMap::new();
        let mut styles: BTreeMap<CellRef, Option<u32>> = BTreeMap::new();
        for line in lines {
            let sources: Vec<CellRef> = (0..length).filter_map(|step| place(line, step)).collect();
            let cells: Vec<Option<&Cell>> =
                sources.iter().map(|cell| content.cells.get(cell)).collect();
            let series = series_of(&cells);
            for step in &steps {
                let Some(cell) = place(line, *step) else {
                    continue;
                };
                let origin_index = usize::try_from(step.rem_euclid(length)).unwrap_or(0);
                let origin = sources[origin_index];
                styles.insert(cell, content.styles.get(&origin).copied());
                let from = cells[origin_index];
                let filled = match series_input(&series, *step) {
                    Some(input) => Some(Cell {
                        input,
                        format: cells[0].map_or(NumberFormat::General, |cell| cell.format),
                    }),
                    None => from.map(|repeated| {
                        let input = match &repeated.input {
                            Input::Formula(expression) => {
                                Input::Formula(Box::new(crate::edit::offset_relative(
                                    expression,
                                    i64::from(cell.row) - i64::from(origin.row),
                                    i64::from(cell.column) - i64::from(origin.column),
                                )))
                            }
                            other => other.clone(),
                        };
                        Cell {
                            input,
                            format: repeated.format,
                        }
                    }),
                };
                written.insert(cell, filled);
            }
        }
        let added = written
            .iter()
            .filter(|(cell, filled)| filled.is_some() && !content.cells.contains_key(cell))
            .count();
        if self.cell_count() + added > MAX_CELLS {
            return Err(EditError::OutOfBounds);
        }
        let target = &mut self.sheets[sheet];
        for (cell, filled) in written {
            match filled {
                Some(filled) => target.cells.insert(cell, filled),
                None => target.cells.remove(&cell),
            };
        }
        for (cell, style) in styles {
            match style {
                Some(style) => target.styles.insert(cell, style),
                None => target.styles.remove(&cell),
            };
        }
        self.recalculate();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
