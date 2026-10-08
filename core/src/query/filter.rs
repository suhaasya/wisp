//! Server-side filter model (values bound as parameters in SQL).

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FilterCombine {
    #[default]
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOperator {
    Eq,
    Ne,
    Lt,
    Gt,
    Lte,
    Gte,
    Like,
    ILike,
    In,
    IsNull,
    IsNotNull,
    Between,
}

impl FilterOperator {
    pub const UI_OPTIONS: &'static [(&'static str, FilterOperator)] = &[
        ("=", FilterOperator::Eq),
        ("!=", FilterOperator::Ne),
        ("<", FilterOperator::Lt),
        (">", FilterOperator::Gt),
        ("<=", FilterOperator::Lte),
        (">=", FilterOperator::Gte),
        ("LIKE", FilterOperator::Like),
        ("ILIKE", FilterOperator::ILike),
        ("IN", FilterOperator::In),
        ("IS NULL", FilterOperator::IsNull),
        ("IS NOT NULL", FilterOperator::IsNotNull),
        ("BETWEEN", FilterOperator::Between),
    ];

    pub fn label(self) -> &'static str {
        Self::UI_OPTIONS
            .iter()
            .find(|(_, op)| *op == self)
            .map(|(l, _)| *l)
            .unwrap_or("?")
    }

    pub fn needs_value(self) -> bool {
        !matches!(
            self,
            FilterOperator::IsNull | FilterOperator::IsNotNull
        )
    }

    pub fn needs_second_value(self) -> bool {
        matches!(self, FilterOperator::Between)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterTerm {
    pub column: String,
    pub operator: FilterOperator,
    pub value: String,
    pub value_to: String,
}

impl Default for FilterTerm {
    fn default() -> Self {
        Self {
            column: String::new(),
            operator: FilterOperator::Eq,
            value: String::new(),
            value_to: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterModel {
    pub combine: FilterCombine,
    pub terms: Vec<FilterTerm>,
    /// Expert mode: appended as `(raw_where)` — not mixed with bound term values.
    pub raw_where: Option<String>,
}

impl FilterModel {
    pub fn is_active(&self) -> bool {
        self.raw_where.as_ref().is_some_and(|s| !s.trim().is_empty())
            || self
                .terms
                .iter()
                .any(|t| !t.column.is_empty() && (t.operator.needs_value() && !t.value.is_empty() || !t.operator.needs_value()))
    }
}
