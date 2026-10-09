//! Render EXPLAIN JSON as an indented text tree (LUM-033).

use serde_json::Value;

/// Pretty-print JSON plan objects (PostgreSQL `EXPLAIN (FORMAT JSON)` / MySQL `FORMAT=JSON`).
pub fn format_explain_json(raw: &str) -> String {
    let trimmed = raw.trim();
    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return raw.to_string();
    };
    let mut out = String::new();
    format_value(&value, 0, &mut out);
    if out.is_empty() {
        raw.to_string()
    } else {
        out
    }
}

fn format_value(value: &Value, depth: usize, out: &mut String) {
    match value {
        Value::Array(items) => {
            for (ix, item) in items.iter().enumerate() {
                if depth == 0 && items.len() > 1 {
                    out.push_str(&format!("--- plan {} ---\n", ix + 1));
                }
                format_value(item, depth, out);
            }
        }
        Value::Object(map) => {
            if let Some(plan) = map.get("Plan") {
                format_plan_node(plan, depth, out);
                return;
            }
            if map.contains_key("Node Type") || map.contains_key("select_type") {
                format_plan_node(value, depth, out);
                return;
            }
            for (k, v) in map {
                indent(depth, out);
                out.push_str(k);
                out.push_str(": ");
                match v {
                    Value::Object(_) | Value::Array(_) => {
                        out.push('\n');
                        format_value(v, depth + 1, out);
                    }
                    _ => {
                        out.push_str(&scalar_string(v));
                        out.push('\n');
                    }
                }
            }
        }
        other => {
            indent(depth, out);
            out.push_str(&scalar_string(other));
            out.push('\n');
        }
    }
}

fn format_plan_node(node: &Value, depth: usize, out: &mut String) {
    let Some(map) = node.as_object() else {
        format_value(node, depth, out);
        return;
    };
    let label = map
        .get("Node Type")
        .or_else(|| map.get("select_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("Plan");
    let cost = map
        .get("Total Cost")
        .or_else(|| map.get("Startup Cost"))
        .map(scalar_string);
    let time = map
        .get("Actual Total Time")
        .or_else(|| map.get("Actual Startup Time"))
        .map(scalar_string);
    indent(depth, out);
    out.push_str(label);
    if let Some(c) = cost {
        out.push_str(&format!("  cost={c}"));
    }
    if let Some(t) = time {
        out.push_str(&format!("  time={t}ms"));
    }
    if let Some(rel) = map.get("Relation Name").and_then(|v| v.as_str()) {
        out.push_str(&format!("  on {rel}"));
    }
    out.push('\n');

    for key in ["Plans", "Plan"] {
        if let Some(Value::Array(children)) = map.get(key) {
            for child in children {
                format_plan_node(child, depth + 1, out);
            }
        } else if let Some(child) = map.get(key) {
            format_plan_node(child, depth + 1, out);
        }
    }
}

fn indent(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn scalar_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
        other => other.to_string(),
    }
}
