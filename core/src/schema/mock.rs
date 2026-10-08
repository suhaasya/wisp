//! Mock introspection catalogs for UI and tests.

use super::catalog::{SchemaCatalog, SchemaObject, SchemaObjectId, SchemaObjectKind};

pub fn demo_catalog_shop() -> SchemaCatalog {
    let schema = String::from("public");
    let mut objects = vec![
        obj(1, &schema, "customers", SchemaObjectKind::Tables),
        obj(2, &schema, "orders", SchemaObjectKind::Tables),
        obj(3, &schema, "order_items", SchemaObjectKind::Tables),
        obj(4, &schema, "products", SchemaObjectKind::Tables),
        obj(5, &schema, "daily_revenue", SchemaObjectKind::Views),
        obj(6, &schema, "active_customers", SchemaObjectKind::Views),
        obj(7, &schema, "set_updated_at", SchemaObjectKind::Functions),
        obj(8, &schema, "orders_id_seq", SchemaObjectKind::Sequences),
        obj(9, &schema, "order_status", SchemaObjectKind::Types),
    ];
    objects.sort_by(|a, b| a.name.cmp(&b.name));
    SchemaCatalog {
        connection_label: "shop_prod".into(),
        database: "shop_prod".into(),
        schemas: vec!["public".into(), "analytics".into()],
        objects,
    }
}

pub fn demo_catalog_large_tables(table_count: usize) -> SchemaCatalog {
    let schema = String::from("public");
    let mut objects = Vec::with_capacity(table_count);
    for i in 0..table_count {
        objects.push(obj(
            i as u32 + 1,
            &schema,
            format!("tbl_{i:05}"),
            SchemaObjectKind::Tables,
        ));
    }
    SchemaCatalog {
        connection_label: "bench".into(),
        database: "bench".into(),
        schemas: vec!["public".into()],
        objects,
    }
}

fn obj(id: u32, schema: &str, name: impl Into<String>, kind: SchemaObjectKind) -> SchemaObject {
    SchemaObject {
        id: SchemaObjectId(id),
        schema: schema.into(),
        name: name.into(),
        kind,
    }
}
