//! Foreign-key metadata and navigation helpers (LUM-027 fixtures).

use crate::query::{FilterCombine, FilterOperator, FilterTerm, TableDataQuery};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKeyColumn {
    pub local_column: String,
    pub referenced_column: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKey {
    pub target_table: String,
    pub columns: Vec<ForeignKeyColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReverseReference {
    pub from_table: String,
    pub fk: ForeignKey,
    /// Mock count for UI until live introspection exists.
    pub row_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRelationSchema {
    pub table: String,
    pub foreign_keys: Vec<(String, ForeignKey)>,
}

/// Demo shop relations (matches `demo_catalog_shop` object names).
pub fn shop_relations() -> Vec<TableRelationSchema> {
    vec![
        TableRelationSchema {
            table: "orders".into(),
            foreign_keys: vec![(
                "customer_id".into(),
                ForeignKey {
                    target_table: "customers".into(),
                    columns: vec![ForeignKeyColumn {
                        local_column: "customer_id".into(),
                        referenced_column: "id".into(),
                    }],
                },
            )],
        },
        TableRelationSchema {
            table: "order_items".into(),
            foreign_keys: vec![
                (
                    "order_id".into(),
                    ForeignKey {
                        target_table: "orders".into(),
                        columns: vec![ForeignKeyColumn {
                            local_column: "order_id".into(),
                            referenced_column: "id".into(),
                        }],
                    },
                ),
                (
                    "product_id".into(),
                    ForeignKey {
                        target_table: "products".into(),
                        columns: vec![ForeignKeyColumn {
                            local_column: "product_id".into(),
                            referenced_column: "id".into(),
                        }],
                    },
                ),
            ],
        },
        TableRelationSchema {
            table: "shipment_legs".into(),
            foreign_keys: vec![(
                "shipment_id_order_id".into(),
                ForeignKey {
                    target_table: "shipments".into(),
                    columns: vec![
                        ForeignKeyColumn {
                            local_column: "shipment_id".into(),
                            referenced_column: "id".into(),
                        },
                        ForeignKeyColumn {
                            local_column: "order_id".into(),
                            referenced_column: "order_id".into(),
                        },
                    ],
                },
            )],
        },
    ]
}

pub fn relations_for_table(table: &str) -> Option<TableRelationSchema> {
    shop_relations()
        .into_iter()
        .find(|s| s.table == table)
}

pub fn reverse_references(target_table: &str) -> Vec<ReverseReference> {
    let mut out = Vec::new();
    for schema in shop_relations() {
        for (local_col, fk) in &schema.foreign_keys {
            if fk.target_table == target_table {
                out.push(ReverseReference {
                    from_table: schema.table.clone(),
                    fk: ForeignKey {
                        target_table: schema.table.clone(),
                        columns: fk
                            .columns
                            .iter()
                            .map(|c| ForeignKeyColumn {
                                local_column: c.local_column.clone(),
                                referenced_column: c.referenced_column.clone(),
                            })
                            .collect(),
                    },
                    row_count: mock_reverse_count(&schema.table, local_col),
                });
            }
        }
    }
    out
}

fn mock_reverse_count(from_table: &str, _local_col: &str) -> u64 {
    match from_table {
        "orders" => 128,
        "order_items" => 512,
        "shipment_legs" => 24,
        _ => 0,
    }
}

/// Build a query that filters `target_table` to rows matching FK key values.
pub fn query_for_fk_target(
    target_table: &str,
    fk: &ForeignKey,
    key_values: &[String],
) -> TableDataQuery {
    let mut query = TableDataQuery::for_table(target_table);
    query.filter.combine = FilterCombine::And;
    for (col, value) in fk.columns.iter().zip(key_values.iter()) {
        query.filter.terms.push(FilterTerm {
            column: col.referenced_column.clone(),
            operator: FilterOperator::Eq,
            value: value.clone(),
            value_to: String::new(),
        });
    }
    query
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_fk_navigation_builds_and_filter() {
        let schema = relations_for_table("shipment_legs").expect("fixture");
        let fk = schema
            .foreign_keys
            .iter()
            .find(|(c, _)| c == "shipment_id_order_id")
            .map(|(_, fk)| fk)
            .expect("composite fk");
        assert_eq!(fk.columns.len(), 2);
        let query = query_for_fk_target(
            &fk.target_table,
            fk,
            &["42".into(), "9001".into()],
        );
        assert_eq!(query.table, "shipments");
        assert_eq!(query.filter.terms.len(), 2);
        assert_eq!(query.filter.terms[0].column, "id");
        assert_eq!(query.filter.terms[1].column, "order_id");
    }
}
