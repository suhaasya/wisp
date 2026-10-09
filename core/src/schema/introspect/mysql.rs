//! MySQL / MariaDB schema introspection (LUM-021).

use std::collections::HashMap;

use wisp_drivers::{ColumnMeta, DbDriver, DriverError};
use wisp_store::ConnectionProfile;

use super::page_rows::fetch_text_rows;
use super::super::catalog::{SchemaCatalog, SchemaObject, SchemaObjectId, SchemaObjectKind};
use super::super::metadata_cache::TableKey;

const PAGE: u32 = 500;
const SYSTEM_DBS: &[&str] = &[
    "information_schema",
    "performance_schema",
    "mysql",
    "sys",
];

pub fn load_schema(
    driver: &mut dyn DbDriver,
    profile: &ConnectionProfile,
    connection_label: &str,
) -> Result<(SchemaCatalog, HashMap<TableKey, Vec<ColumnMeta>>), DriverError> {
    let database = driver
        .server_info()
        .map(|i| i.database)
        .unwrap_or_else(|_| profile.database.clone().unwrap_or_default());

    // Must use SELECT (not SHOW DATABASES): MySQL driver pages via a derived table subquery.
    let db_rows = fetch_text_rows(
        driver,
        "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA \
         WHERE SCHEMA_NAME NOT IN ('information_schema', 'performance_schema', 'mysql', 'sys') \
         ORDER BY SCHEMA_NAME",
        PAGE,
    )?;
    let schemas: Vec<String> = db_rows
        .into_iter()
        .filter_map(|r| r.into_iter().next())
        .filter(|name| !SYSTEM_DBS.contains(&name.as_str()))
        .collect();

    let object_rows = fetch_text_rows(
        driver,
        "SELECT TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE FROM information_schema.tables \
         WHERE TABLE_SCHEMA NOT IN ('information_schema', 'performance_schema', 'mysql', 'sys') \
         ORDER BY TABLE_SCHEMA, TABLE_NAME",
        PAGE,
    )?;

    let mut objects = Vec::new();
    let mut next_id = 1u32;
    for row in object_rows {
        if row.len() < 3 {
            continue;
        }
        let schema = row[0].clone();
        let name = row[1].clone();
        let kind = match row[2].as_str() {
            "BASE TABLE" => SchemaObjectKind::Tables,
            "VIEW" => SchemaObjectKind::Views,
            _ => continue,
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

    let columns = load_columns(driver)?;

    let schemas = if schemas.is_empty() && !database.is_empty() {
        vec![database.clone()]
    } else {
        schemas
    };

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

fn append_routines(
    driver: &mut dyn DbDriver,
    objects: &mut Vec<SchemaObject>,
    next_id: &mut u32,
) -> Result<(), DriverError> {
    let rows = fetch_text_rows(
        driver,
        "SELECT ROUTINE_SCHEMA, ROUTINE_NAME, ROUTINE_TYPE FROM information_schema.routines \
         WHERE ROUTINE_SCHEMA NOT IN ('information_schema', 'performance_schema', 'mysql', 'sys') \
         ORDER BY ROUTINE_SCHEMA, ROUTINE_NAME",
        PAGE,
    )?;
    for row in rows {
        if row.len() < 3 {
            continue;
        }
        if row[2] != "FUNCTION" && row[2] != "PROCEDURE" {
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

fn load_columns(driver: &mut dyn DbDriver) -> Result<HashMap<TableKey, Vec<ColumnMeta>>, DriverError> {
    let rows = fetch_text_rows(
        driver,
        "SELECT TABLE_SCHEMA, TABLE_NAME, COLUMN_NAME, DATA_TYPE, IS_NULLABLE \
         FROM information_schema.columns \
         WHERE TABLE_SCHEMA NOT IN ('information_schema', 'performance_schema', 'mysql', 'sys') \
         ORDER BY TABLE_SCHEMA, TABLE_NAME, ORDINAL_POSITION",
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
