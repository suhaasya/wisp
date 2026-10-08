//! Total row count: exact, estimate, or on-demand exact.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowCount {
    Unknown,
    Estimate { rows: u64, source: &'static str },
    Exact(u64),
}

impl RowCount {
    pub fn best_guess(&self) -> Option<u64> {
        match self {
            Self::Unknown => None,
            Self::Estimate { rows, .. } | Self::Exact(rows) => Some(*rows),
        }
    }

    pub fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }
}
