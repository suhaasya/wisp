//! Compact cell values (column-major pages hold these; large payloads use arena refs).

use std::fmt;

use crate::arena::{BlobRef, StrRef};

/// Maximum inline prefix for [`Value::Bytes`] (full value fetched on demand in LUM-012+).
pub const BYTES_PREVIEW_MAX: u32 = 4096;

/// One cell in a result page.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Decimal(StrRef),
    Text(StrRef),
    Bytes(BytesPreview),
    Date(i32),
    Time(i64),
    Timestamp(i64),
    Json(StrRef),
    Uuid([u8; 16]),
    Unknown(StrRef),
}

/// Total byte length plus arena prefix (≤ [`BYTES_PREVIEW_MAX`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytesPreview {
    pub total_len: u64,
    pub prefix: BlobRef,
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "NULL"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Int(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Decimal(_) | Self::Text(_) | Self::Json(_) | Self::Unknown(_) => {
                write!(f, "<text>")
            }
            Self::Bytes(b) => write!(f, "<bytes {}>", b.total_len),
            Self::Date(_) => write!(f, "<date>"),
            Self::Time(_) => write!(f, "<time>"),
            Self::Timestamp(_) => write!(f, "<timestamp>"),
            Self::Uuid(_) => write!(f, "<uuid>"),
        }
    }
}
