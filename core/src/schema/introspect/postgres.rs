//! PostgreSQL schema introspection (LUM-021).

use std::collections::HashMap;

use wisp_drivers::{ColumnMeta, DbDriver, DriverError};
use wisp_store::ConnectionProfile;

use super::page_rows::fetch_text_rows;
use super::super::catalog::{SchemaCatalog, SchemaObject, SchemaObjectId, SchemaObjectKind};
use super::super::metadata_cache::TableKey;

const PAGE: u32 = 500;

pub fn load_schema(
    driver: &mut dyn DbDriver,
    profile: &ConnectionProfile,
    connection_label: &str,
) -> Result<(SchemaCatalog, HashMap<TableKey, Vec<ColumnMeta>>), DriverError> {
    let database = driver
        .server_info()
        .map(|i| i.database)
        .unwrap_or_else(|_| profile.database.clone().unwrap_or_default());

    let schema_rows = fetch_text_rows(
        driver,
        "SELECT schema_name FROM information_schema.schemata \
         WHERE schema_name NOT IN ('pg_catalog', 'information_schema') \
         AND schema_name NOT LIKE 'pg_%' \
         ORDER BY schema_name",
        PAGE,
    )?;
    let schemas: Vec<String> = schema_rows.into_iter().filter_map(|r| r.into_iter().next()).collect();

    let object_rows = fetch_text_rows(
        driver,
        "SELECT table_schema, table_name, table_type FROM information_schema.tables \
         WHERE table_schema NOT IN ('pg_catalog', 'information_schema') \
         AND table_schema NOT LIKE 'pg_%' \
         AND table_type IN ('BASE TABLE', 'VIEW') \
         ORDER BY table_schema, table_name",
        PAGE,
    )?;

    let mut objects = Vec::new();
    let mut next_id = 1u32;
    for row in object_rows {
        let Some((schema, name, kind)) = parse_object_row(row) else {
            continue;
        };
        objects.push(SchemaObject {
            id: SchemaObjectId(next_id),
            schema,
            name,
            kind,
        });
        next_id += 1;
    }

    append_routines(driver, &mut objects, &mut next_id)?;
    append_sequences(driver, &mut objects, &mut next_id)?;

    let columns = load_columns(driver)?;

    let mut schemas = schemas;
    if schemas.is_empty() {
        schemas = objects
            .iter()
            .map(|o| o.schema.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    if schemas.is_empty() {
        if let Some(db) = profile.database.as_ref().filter(|s| !s.is_empty()) {
            schemas.push(db.clone());
        } else {
            schemas.push("public".into());
        }
    }

    Ok((
        SchemaCatalog {
            connection_label: connection_label.to_string(),
            database,
            schemas,
            objects,
        },
        columns,
    ))
}

fn parse_object_row(row: Vec<String>) -> Option<(String, String, SchemaObjectKind)> {
    if row.len() < 3 {
        return None;
    }
    let schema = row[0].clone();
    let name = row[1].clone();
    let kind = match row[2].as_str() {
        "BASE TABLE" => SchemaObjectKind::Tables,
        "VIEW" => SchemaObjectKind::Views,
        _ => return None,
    };
    Some((schema, name, kind))
}

fn append_routines(
    driver: &mut dyn DbDriver,
    objects: &mut Vec<SchemaObject>,
    next_id: &mut u32,
) -> Result<(), DriverError> {
    let rows = fetch_text_rows(
        driver,
        "SELECT routine_schema, routine_name FROM information_schema.routines \
         WHERE routine_schema NOT IN ('pg_catalog', 'information_schema') \
         AND routine_schema NOT LIKE 'pg_%' \
         AND routine_type = 'FUNCTION' \
         ORDER BY routine_schema, routine_name",
        PAGE,
    )?;
    for row in rows {
        if row.len() < 2 {
            continue;
        }
        objects.push(SchemaObject {
            id: SchemaObjectId(*next_id),
            schema: row[0].clone(),
            name: row[1].clone(),
            kind: SchemaObjectKind::Functions,
        });
        *next_id += 1;
    }
    Ok(())
}

fn append_sequences(
    driver: &mut dyn DbDriver,
    objects: &mut Vec<SchemaObject>,
    next_id: &mut u32,
) -> Result<(), DriverError> {
    let rows = fetch_text_rows(
        driver,
        "SELECT sequence_schema, sequence_name FROM information_schema.sequences \
         WHERE sequence_schema NOT IN ('pg_catalog', 'information_schema') \
         AND sequence_schema NOT LIKE 'pg_%' \
         ORDER BY sequence_schema, sequence_name",
        PAGE,
    )?;
    for row in rows {
        if row.len() < 2 {
            continue;
        }
        objects.push(SchemaObject {
            id: SchemaObjectId(*next_id),
            schema: row[0].clone(),
            name: row[1].clone(),
            kind: SchemaObjectKind::Sequences,
        });
        *next_id += 1;
    }
    Ok(())
}

fn load_columns(driver: &mut dyn DbDriver) -> Result<HashMap<TableKey, Vec<ColumnMeta>>, DriverError> {
    let rows = fetch_text_rows(
        driver,
        "SELECT table_schema, table_name, column_name, data_type, is_nullable \
         FROM information_schema.columns \
         WHERE table_schema NOT IN ('pg_catalog', 'information_schema') \
         AND table_schema NOT LIKE 'pg_%' \
         ORDER BY table_schema, table_name, ordinal_position",
        PAGE,
    )?;
    let mut map: HashMap<TableKey, Vec<ColumnMeta>> = HashMap::new();
    for row in rows {
        if row.len() < 5 {
            continue;
        }
        let key = TableKey {
            schema: row[0].clone(),
            name: row[1].clone(),
        };
        let nullable = row[4].eq_ignore_ascii_case("YES");
        map.entry(key).or_default().push(ColumnMeta {
            name: row[2].clone(),
            type_name: row[3].clone(),
            nullable,
            is_pk: false,
            max_length: None,
        });
    }
    Ok(map)
}
