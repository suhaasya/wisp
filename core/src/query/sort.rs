//! Multi-column sort (server-side ORDER BY).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortKey {
    pub column: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SortModel {
    pub keys: Vec<SortKey>,
}

impl SortModel {
    pub fn toggle_column(&mut self, column: impl Into<String>, extend: bool) {
        let column = column.into();
        if extend {
            if let Some(ix) = self.keys.iter().position(|k| k.column == column) {
                self.keys[ix].direction = match self.keys[ix].direction {
                    SortDirection::Asc => SortDirection::Desc,
                    SortDirection::Desc => SortDirection::Asc,
                };
            } else {
                self.keys.push(SortKey {
                    column,
                    direction: SortDirection::Asc,
                });
            }
            return;
        }
        if self.keys.len() == 1 && self.keys[0].column == column {
            self.keys[0].direction = match self.keys[0].direction {
                SortDirection::Asc => SortDirection::Desc,
                SortDirection::Desc => SortDirection::Asc,
            };
        } else {
            self.keys = vec![SortKey {
                column,
                direction: SortDirection::Asc,
            }];
        }
    }

    pub fn indicator(&self, column: &str) -> Option<char> {
        self.keys
            .iter()
            .find(|k| k.column == column)
            .map(|k| match k.direction {
                SortDirection::Asc => '↑',
                SortDirection::Desc => '↓',
            })
    }
}
