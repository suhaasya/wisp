//! Cell display formatting (NULL, numbers, previews).

use wisp_core::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Null,
    Number,
    Text,
    JsonPreview,
    BlobPreview,
    Bool,
}

#[derive(Debug, Clone)]
pub struct CellDisplay {
    pub kind: CellKind,
    pub text: String,
}

const MAX_CELL_CHARS: usize = 64;

pub fn format_cell(value: Option<&Value>, type_name: &str) -> CellDisplay {
    let Some(value) = value else {
        return CellDisplay {
            kind: CellKind::Null,
            text: "NULL".into(),
        };
    };
    match value {
        Value::Null => CellDisplay {
            kind: CellKind::Null,
            text: "NULL".into(),
        },
        Value::Bool(v) => CellDisplay {
            kind: CellKind::Bool,
            text: if *v { "true" } else { "false" }.into(),
        },
        Value::Int(v) => CellDisplay {
            kind: CellKind::Number,
            text: v.to_string(),
        },
        Value::Float(v) => CellDisplay {
            kind: CellKind::Number,
            text: format!("{v}"),
        },
        Value::Json(_) | Value::Unknown(_) if type_name.contains("json") => CellDisplay {
            kind: CellKind::JsonPreview,
            text: "{…}".into(),
        },
        Value::Bytes(b) => CellDisplay {
            kind: CellKind::BlobPreview,
            text: format!("⬡ {} B", b.total_len),
        },
        Value::Text(_) | Value::Decimal(_) | Value::Json(_) | Value::Unknown(_) => CellDisplay {
            kind: CellKind::Text,
            text: truncate("<text>", MAX_CELL_CHARS),
        },
        _ => CellDisplay {
            kind: CellKind::Text,
            text: truncate("<value>", MAX_CELL_CHARS),
        },
    }
}

pub fn format_cell_text(text: Option<&str>, type_name: &str, value: Option<&Value>) -> CellDisplay {
    if let Some(t) = text.filter(|s| !s.is_empty()) {
        if value.is_some_and(|v| !matches!(v, Value::Null)) {
            let kind = if type_name.contains("int")
                || type_name.contains("bigint")
                || type_name.contains("numeric")
                || type_name.contains("float")
                || type_name.contains("double")
            {
                CellKind::Number
            } else {
                CellKind::Text
            };
            return CellDisplay {
                kind,
                text: truncate(t, MAX_CELL_CHARS),
            };
        }
    }
    format_cell(value, type_name)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
