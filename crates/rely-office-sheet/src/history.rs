// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! Undo and redo for an editor: the workbook as it was before each edit.
//!
//! A step is a copy of the whole workbook rather than the inverse of an edit, because an edit
//! that moves rows rewrites formulas on every sheet and a copy is the only inverse that cannot be
//! wrong. The copies are bounded: by count, and by the cells they hold together, so a large
//! workbook keeps fewer steps instead of holding its size many times over.

use std::collections::VecDeque;

use crate::model::{Sheet, Workbook};

/// The most steps an editor can undo.
pub const MAX_UNDO_STEPS: usize = 100;
/// The most cells the undo steps hold together; the oldest steps go first past it.
pub const MAX_UNDO_CELLS: usize = 2_000_000;

/// The steps an editor can undo and redo.
#[derive(Debug, Clone, Default)]
pub struct EditHistory {
    undo: VecDeque<(Workbook, usize)>,
    redo: Vec<(Workbook, usize)>,
    cells: usize,
}

fn cells_of(workbook: &Workbook) -> usize {
    workbook
        .sheets()
        .iter()
        .map(Sheet::cell_count)
        .sum::<usize>()
        .max(1)
}

impl EditHistory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Keep the workbook as it is before an edit; an edit after an undo drops what could be
    /// redone, as every editor does.
    pub fn record(&mut self, before: &Workbook) {
        let cells = cells_of(before);
        self.undo.push_back((before.clone(), cells));
        self.cells += cells;
        self.redo.clear();
        while self.undo.len() > MAX_UNDO_STEPS
            || (self.cells > MAX_UNDO_CELLS && self.undo.len() > 1)
        {
            if let Some((_, dropped)) = self.undo.pop_front() {
                self.cells -= dropped;
            }
        }
    }

    /// Forget the step `record` kept, for an edit that was refused and changed nothing.
    pub fn discard_last(&mut self) {
        if let Some((_, cells)) = self.undo.pop_back() {
            self.cells -= cells;
        }
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Put back the workbook as it was before the last edit; false when there is none.
    pub fn undo(&mut self, workbook: &mut Workbook) -> bool {
        let Some((before, cells)) = self.undo.pop_back() else {
            return false;
        };
        self.cells -= cells;
        let now = std::mem::replace(workbook, before);
        let now_cells = cells_of(&now);
        self.redo.push((now, now_cells));
        true
    }

    /// Put back the edit the last undo took away; false when there is none.
    pub fn redo(&mut self, workbook: &mut Workbook) -> bool {
        let Some((after, _)) = self.redo.pop() else {
            return false;
        };
        let now = std::mem::replace(workbook, after);
        let now_cells = cells_of(&now);
        self.undo.push_back((now, now_cells));
        self.cells += now_cells;
        true
    }

    /// Forget every step, for a workbook opened again.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.cells = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::CellRef;
    use crate::value::Locale;

    fn typed(workbook: &mut Workbook, history: &mut EditHistory, text: &str) {
        history.record(workbook);
        workbook
            .set_input(0, CellRef::new(0, 0), text, Locale::Italian)
            .unwrap();
    }

    fn a1(workbook: &Workbook) -> String {
        workbook.display(0, CellRef::new(0, 0), Locale::Italian)
    }

    #[test]
    fn undoes_and_redoes_edits_in_order() {
        let mut workbook = Workbook::new();
        let mut history = EditHistory::new();
        typed(&mut workbook, &mut history, "1");
        typed(&mut workbook, &mut history, "2");

        assert!(history.undo(&mut workbook));
        assert_eq!(a1(&workbook), "1");
        assert!(history.undo(&mut workbook));
        assert_eq!(a1(&workbook), "");
        assert!(!history.undo(&mut workbook));
        assert!(history.redo(&mut workbook));
        assert_eq!(a1(&workbook), "1");
        assert!(history.can_redo());
    }

    #[test]
    fn a_new_edit_drops_what_could_be_redone() {
        let mut workbook = Workbook::new();
        let mut history = EditHistory::new();
        typed(&mut workbook, &mut history, "1");
        history.undo(&mut workbook);

        typed(&mut workbook, &mut history, "3");

        assert!(!history.can_redo());
        assert!(!history.redo(&mut workbook));
        assert_eq!(a1(&workbook), "3");
    }

    #[test]
    fn a_refused_edit_leaves_no_step() {
        let workbook = Workbook::new();
        let mut history = EditHistory::new();
        history.record(&workbook);
        history.discard_last();

        assert!(!history.can_undo());
        assert_eq!(history.cells, 0);
    }

    #[test]
    fn keeps_at_most_the_last_steps() {
        let mut workbook = Workbook::new();
        let mut history = EditHistory::new();
        for step in 0..=MAX_UNDO_STEPS + 5 {
            typed(&mut workbook, &mut history, &step.to_string());
        }

        assert_eq!(history.undo.len(), MAX_UNDO_STEPS);
        while history.undo(&mut workbook) {}
        assert_eq!(a1(&workbook), "5");
    }

    #[test]
    fn clear_forgets_everything() {
        let mut workbook = Workbook::new();
        let mut history = EditHistory::new();
        typed(&mut workbook, &mut history, "1");
        history.undo(&mut workbook);

        history.clear();

        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }
}
