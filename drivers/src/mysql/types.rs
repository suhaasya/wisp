//! Map MySQL wire types into [`Value`].

use mysql_async::{consts::ColumnType, Row, Value as MysqlValue};

use crate::{
    column::ColumnMeta,
    page::PageArena,
    value::{BytesPreview, Value, BYTES_PREVIEW_MAX},
};

pub fn column_meta(name: &str, column_type: ColumnType) -> ColumnMeta {
    ColumnMeta::new(name, format!("{column_type:?}"))
}

pub fn row_to_values(
    row: &Row,
    arena: &mut PageArena,
    tinyint1_is_bool: bool,
) -> Vec<Value> {
    (0..row.len())
        .map(|i| {
            let col = &row.columns_ref()[i];
            let ty = col.column_type();
            match row.as_ref(i) {
                None => Value::Null,
                Some(v) => cell_to_value(v, ty, arena, tinyint1_is_bool),
            }
        })
        .collect()
}

fn cell_to_value(
    cell: &MysqlValue,
    column_type: ColumnType,
    arena: &mut PageArena,
    tinyint1_is_bool: bool,
) -> Value {
    match cell {
        MysqlValue::NULL => Value::Null,
        MysqlValue::Int(n) => {
            if tinyint1_is_bool && matches!(column_type, ColumnType::MYSQL_TYPE_TINY) {
                Value::Bool(*n != 0)
            } else {
                Value::Int(*n)
            }
        }
        MysqlValue::UInt(n) => Value::Int(*n as i64),
        MysqlValue::Float(f) => Value::Float(f64::from(*f)),
        MysqlValue::Double(f) => Value::Float(*f),
        MysqlValue::Date(year, month, day, hour, minute, second, micros) => {
            use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
            if *hour == 0 && *minute == 0 && *second == 0 && *micros == 0 {
                let date = NaiveDate::from_ymd_opt(*year as i32, *month as u32, *day as u32)
                    .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).expect("epoch"));
                Value::Date(date.num_days_from_ce())
            } else {
                let date = NaiveDate::from_ymd_opt(*year as i32, *month as u32, *day as u32)
                    .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).expect("epoch"));
                let time = NaiveTime::from_hms_micro_opt(
                    u32::from(*hour),
                    u32::from(*minute),
                    u32::from(*second),
                    *micros,
                )
                    .unwrap_or(NaiveTime::MIN);
                let dt = NaiveDateTime::new(date, time);
                Value::Timestamp(dt.and_utc().timestamp_micros())
            }
        }
        MysqlValue::Time(neg, days, hours, minutes, seconds, micros) => {
            let total_micros = i64::from(*days) * 86_400_000_000
                + i64::from(*hours) * 3_600_000_000
                + i64::from(*minutes) * 60_000_000
                + i64::from(*seconds) * 1_000_000
                + i64::from(*micros);
            Value::Time(if *neg { -total_micros } else { total_micros })
        }
        MysqlValue::Bytes(bytes) => bytes_value(bytes, column_type, arena),
    }
}

fn bytes_value(bytes: &[u8], column_type: ColumnType, arena: &mut PageArena) -> Value {
    match column_type {
        ColumnType::MYSQL_TYPE_JSON => {
            Value::Json(arena.push_str(&String::from_utf8_lossy(bytes)))
        }
        ColumnType::MYSQL_TYPE_DECIMAL | ColumnType::MYSQL_TYPE_NEWDECIMAL => {
            Value::Decimal(arena.push_str(&String::from_utf8_lossy(bytes)))
        }
        ColumnType::MYSQL_TYPE_ENUM | ColumnType::MYSQL_TYPE_SET => {
            Value::Text(arena.push_str(&String::from_utf8_lossy(bytes)))
        }
        ColumnType::MYSQL_TYPE_BLOB
        | ColumnType::MYSQL_TYPE_TINY_BLOB
        | ColumnType::MYSQL_TYPE_MEDIUM_BLOB
        | ColumnType::MYSQL_TYPE_LONG_BLOB => {
            let total = bytes.len() as u64;
            let take = bytes.len().min(BYTES_PREVIEW_MAX as usize);
            let prefix = arena.push_bytes(&bytes[..take]);
            Value::Bytes(BytesPreview { total_len: total, prefix })
        }
        _ => {
            if let Ok(text) = std::str::from_utf8(bytes) {
                Value::Text(arena.push_str(text))
            } else {
                Value::Unknown(arena.push_str(&format!("<bytes {}>", bytes.len())))
            }
        }
    }
}
