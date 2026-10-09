//! Grid selection (cell, row, range).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellCoord {
    pub row: u64,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridSelection {
    None,
    Cell(CellCoord),
    Row(u64),
    Range { start: CellCoord, end: CellCoord },
}

impl GridSelection {
    pub fn normalize_range(a: CellCoord, b: CellCoord) -> Self {
        let (start, end) = if (a.row, a.col) <= (b.row, b.col) {
            (a, b)
        } else {
            (b, a)
        };
        Self::Range { start, end }
    }

    pub fn primary_row(&self) -> Option<u64> {
        match self {
            Self::None => None,
            Self::Cell(c) => Some(c.row),
            Self::Row(r) => Some(*r),
            Self::Range { start, .. } => Some(start.row),
        }
    }

    pub fn contains(&self, row: u64, col: usize) -> bool {
        match self {
            Self::None => false,
            Self::Cell(c) => c.row == row && c.col == col,
            Self::Row(r) => *r == row,
            Self::Range { start, end } => {
                row >= start.row
                    && row <= end.row
                    && col >= start.col
                    && col <= end.col
            }
        }
    }
}
