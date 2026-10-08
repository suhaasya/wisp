//! Column layout: width, order, visibility.

use wisp_core::ColumnMeta;

pub const DEFAULT_COL_WIDTH: f32 = 140.0;
pub const MIN_COL_WIDTH: f32 = 48.0;
pub const ROW_NUMBER_WIDTH: f32 = 44.0;

#[derive(Debug, Clone)]
pub struct GridColumn {
    pub meta: ColumnMeta,
    pub width: f32,
    pub hidden: bool,
}

#[derive(Debug, Clone)]
pub struct ColumnLayout {
    pub columns: Vec<GridColumn>,
}

impl ColumnLayout {
    pub fn from_meta(columns: Vec<ColumnMeta>) -> Self {
        Self {
            columns: columns
                .into_iter()
                .map(|meta| GridColumn {
                    meta,
                    width: DEFAULT_COL_WIDTH,
                    hidden: false,
                })
                .collect(),
        }
    }

    pub fn visible_indices(&self) -> Vec<usize> {
        self.columns
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.hidden)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn total_width(&self, indices: &[usize]) -> f32 {
        indices.iter().map(|&i| self.columns[i].width).sum()
    }

    pub fn column_at_x(&self, indices: &[usize], mut x: f32) -> Option<usize> {
        for &i in indices {
            let w = self.columns[i].width;
            if x < w {
                return Some(i);
            }
            x -= w;
        }
        None
    }

    pub fn first_visible_column(&self, indices: &[usize], scroll_x: f32) -> usize {
        let mut acc = 0.0f32;
        for &i in indices {
            let w = self.columns[i].width;
            if acc + w > scroll_x {
                return i;
            }
            acc += w;
        }
        indices.last().copied().unwrap_or(0)
    }
}
