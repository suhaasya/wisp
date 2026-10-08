//! Result column metadata.

/// Description of one column in a paged result set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub is_pk: bool,
    pub max_length: Option<u32>,
}

impl ColumnMeta {
    pub fn new(name: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            nullable: true,
            is_pk: false,
            max_length: None,
        }
    }
}
