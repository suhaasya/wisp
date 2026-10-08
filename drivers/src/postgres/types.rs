//! Map PostgreSQL wire types into [`Value`].

use std::fmt::Write as _;

use postgres_types::Type;
use tokio_postgres::Row;

use crate::{
    column::ColumnMeta,
    page::PageArena,
    value::{BytesPreview, Value, BYTES_PREVIEW_MAX},
};

pub fn column_meta(name: &str, ty: &Type) -> ColumnMeta {
    ColumnMeta::new(name, ty.name())
}

pub fn row_to_values(row: &Row, arena: &mut PageArena) -> Vec<Value> {
    (0..row.len())
        .map(|col| cell_to_value(row, col, arena))
        .collect()
}

fn cell_to_value(row: &Row, col: usize, arena: &mut PageArena) -> Value {
    let ty = row.columns()[col].type_();
    match *ty {
        Type::BOOL => match row.try_get::<_, Option<bool>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(v)) => Value::Bool(v),
            Err(_) => Value::Null,
        },
        Type::INT2 => match row.try_get::<_, Option<i16>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(v)) => Value::Int(i64::from(v)),
            Err(_) => Value::Null,
        },
        Type::INT4 => match row.try_get::<_, Option<i32>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(v)) => Value::Int(i64::from(v)),
            Err(_) => Value::Null,
        },
        Type::INT8 => match row.try_get::<_, Option<i64>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(v)) => Value::Int(v),
            Err(_) => Value::Null,
        },
        Type::FLOAT4 => match row.try_get::<_, Option<f32>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(v)) => Value::Float(f64::from(v)),
            Err(_) => Value::Null,
        },
        Type::FLOAT8 => match row.try_get::<_, Option<f64>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(v)) => Value::Float(v),
            Err(_) => Value::Null,
        },
        Type::NUMERIC => match row.try_get::<_, Option<String>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(v)) => Value::Decimal(arena.push_str(&v)),
            Err(_) => unknown_text(row, col, ty, arena),
        },
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => {
            match row.try_get::<_, Option<&str>>(col) {
                Ok(None) => Value::Null,
                Ok(Some(v)) => Value::Text(arena.push_str(v)),
                Err(_) => Value::Null,
            }
        }
        Type::JSON | Type::JSONB => match row.try_get::<_, Option<&str>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(v)) => Value::Json(arena.push_str(v)),
            Err(_) => Value::Null,
        },
        Type::UUID => match row.try_get::<_, Option<uuid::Uuid>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(v)) => Value::Uuid(v.into_bytes()),
            Err(_) => Value::Null,
        },
        Type::BYTEA => match row.try_get::<_, Option<&[u8]>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(data)) => {
                let total = data.len() as u64;
                let take = data.len().min(BYTES_PREVIEW_MAX as usize);
                let prefix = arena.push_bytes(&data[..take]);
                Value::Bytes(BytesPreview {
                    total_len: total,
                    prefix,
                })
            }
            Err(_) => Value::Null,
        },
        Type::DATE => match row.try_get::<_, Option<chrono::NaiveDate>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(date)) => {
                use chrono::Datelike;
                Value::Date(date.num_days_from_ce())
            }
            Err(_) => Value::Null,
        },
        Type::TIME | Type::TIMETZ => match row.try_get::<_, Option<chrono::NaiveTime>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(time)) => {
                use chrono::Timelike;
                let nanos = time.num_seconds_from_midnight() as i64 * 1_000_000_000
                    + i64::from(time.nanosecond());
                Value::Time(nanos)
            }
            Err(_) => Value::Null,
        },
        Type::TIMESTAMP | Type::TIMESTAMPTZ => {
            match row.try_get::<_, Option<chrono::NaiveDateTime>>(col) {
                Ok(None) => Value::Null,
                Ok(Some(ts)) => Value::Timestamp(ts.and_utc().timestamp_micros()),
                Err(_) => Value::Null,
            }
        }
        Type::BOOL_ARRAY
        | Type::INT2_ARRAY
        | Type::INT4_ARRAY
        | Type::INT8_ARRAY
        | Type::TEXT_ARRAY
        | Type::VARCHAR_ARRAY
        | Type::BPCHAR_ARRAY
        | Type::JSON_ARRAY
        | Type::JSONB_ARRAY
        | Type::UUID_ARRAY => match row.try_get::<_, Option<String>>(col) {
            Ok(None) => Value::Null,
            Ok(Some(raw)) => Value::Text(arena.push_str(&raw)),
            Err(_) => Value::Null,
        },
        _ => unknown_text(row, col, ty, arena),
    }
}

fn unknown_text(row: &Row, col: usize, ty: &Type, arena: &mut PageArena) -> Value {
    let mut buf = String::new();
    let _ = write!(buf, "<{}>", ty.name());
    if let Ok(Some(s)) = row.try_get::<_, Option<String>>(col) {
        let _ = write!(buf, ":{s}");
    }
    Value::Unknown(arena.push_str(&buf))
}
