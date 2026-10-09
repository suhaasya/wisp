//! Tree-sitter SQL syntax highlighting (LUM-031).

use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent, Highlighter};

use super::statement_split::SplitFlavor;

/// Names passed to [`HighlightConfiguration::configure`]; indices match [`Highlight`] values.
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "keyword",
    "operator",
    "string",
    "number",
    "function",
    "comment",
    "type",
    "variable",
    "attribute",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SyntaxTokenKind {
    Plain,
    Keyword,
    String,
    Number,
    Function,
    Comment,
    Null,
    Operator,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HighlightSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: SyntaxTokenKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    Postgres,
    Mysql,
}

impl SqlDialect {
    pub fn from_split_flavor(flavor: SplitFlavor) -> Self {
        match flavor {
            SplitFlavor::Mysql => SqlDialect::Mysql,
            SplitFlavor::Standard => SqlDialect::Postgres,
        }
    }
}

/// Above this line count, use the lexical highlighter so edits stay within one frame (LUM-031).
const LEXICAL_LINE_THRESHOLD: usize = 2_048;

/// SQL highlighter (tree-sitter-sequel). Reparses the full buffer each call — fast enough for 10k lines.
pub struct SqlSyntaxHighlighter {
    highlighter: Highlighter,
    config: HighlightConfiguration,
    dialect: SqlDialect,
}

impl SqlSyntaxHighlighter {
    pub fn new(dialect: SqlDialect) -> Self {
        let language: tree_sitter::Language = tree_sitter_sequel::LANGUAGE.into();
        let mut config = HighlightConfiguration::new(
            language,
            "sql",
            tree_sitter_sequel::HIGHLIGHTS_QUERY,
            "",
            "",
        )
        .expect("tree-sitter-sequel highlight query");

        config.configure(HIGHLIGHT_NAMES);

        Self {
            highlighter: Highlighter::new(),
            config,
            dialect,
        }
    }

    pub fn set_dialect(&mut self, dialect: SqlDialect) {
        self.dialect = dialect;
    }

    pub fn highlight(&mut self, source: &str) -> Vec<HighlightSpan> {
        let line_count = source.matches('\n').count() + usize::from(source.is_empty());
        let mut spans = if line_count > LEXICAL_LINE_THRESHOLD {
            lexical_highlight(source, self.dialect)
        } else {
            let events = self
                .highlighter
                .highlight(&self.config, source.as_bytes(), None, |_| None)
                .expect("highlight sql");
            events_to_spans(events)
        };
        if line_count <= LEXICAL_LINE_THRESHOLD {
            apply_dialect_keywords(source, self.dialect, &mut spans);
        }
        coalesce_spans(&mut spans);
        spans
    }
}

fn lexical_highlight(source: &str, dialect: SqlDialect) -> Vec<HighlightSpan> {
    let bytes = source.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\'' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            spans.push(HighlightSpan {
                start_byte: start,
                end_byte: i,
                kind: SyntaxTokenKind::String,
            });
            continue;
        }
        if b == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'"' {
                    if bytes.get(i + 1) == Some(&b'"') {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            spans.push(HighlightSpan {
                start_byte: start,
                end_byte: i,
                kind: SyntaxTokenKind::String,
            });
            continue;
        }
        if b == b'-' && bytes.get(i + 1) == Some(&b'-') {
            let start = i;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            spans.push(HighlightSpan {
                start_byte: start,
                end_byte: i,
                kind: SyntaxTokenKind::Comment,
            });
            continue;
        }
        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            spans.push(HighlightSpan {
                start_byte: start,
                end_byte: i,
                kind: SyntaxTokenKind::Comment,
            });
            continue;
        }
        if b.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            spans.push(HighlightSpan {
                start_byte: start,
                end_byte: i,
                kind: SyntaxTokenKind::Number,
            });
            continue;
        }
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &source[start..i];
            let upper = word.to_ascii_uppercase();
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let kind = if upper == "NULL" {
                SyntaxTokenKind::Null
            } else if is_sql_keyword(&upper, dialect) {
                SyntaxTokenKind::Keyword
            } else if j < bytes.len() && bytes[j] == b'(' {
                SyntaxTokenKind::Function
            } else {
                SyntaxTokenKind::Plain
            };
            if kind != SyntaxTokenKind::Plain {
                spans.push(HighlightSpan {
                    start_byte: start,
                    end_byte: i,
                    kind,
                });
            }
            continue;
        }
        i += 1;
    }
    spans
}

fn is_sql_keyword(word: &str, dialect: SqlDialect) -> bool {
    if COMMON_SQL_KEYWORDS.contains(&word) {
        return true;
    }
    match dialect {
        SqlDialect::Postgres => PG_DIALECT_KEYWORDS.iter().any(|k| *k == word),
        SqlDialect::Mysql => MYSQL_DIALECT_KEYWORDS.iter().any(|k| *k == word),
    }
}

const COMMON_SQL_KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "INSERT", "INTO", "UPDATE", "DELETE", "CREATE", "DROP", "ALTER",
    "TABLE", "INDEX", "VIEW", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "ON", "AS", "AND", "OR",
    "NOT", "IN", "IS", "NULL", "ORDER", "BY", "GROUP", "HAVING", "LIMIT", "OFFSET", "UNION", "ALL",
    "DISTINCT", "SET", "VALUES", "BEGIN", "COMMIT", "ROLLBACK", "CASE", "WHEN", "THEN", "ELSE",
    "END", "WITH", "GRANT", "REVOKE", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "CONSTRAINT",
    "DEFAULT", "TRUE", "FALSE", "LIKE", "BETWEEN", "EXISTS", "CAST", "OVER", "PARTITION",
];

fn kind_from_highlight(h: Highlight) -> SyntaxTokenKind {
    match HIGHLIGHT_NAMES.get(h.0) {
        Some(&"keyword") | Some(&"type") | Some(&"attribute") => SyntaxTokenKind::Keyword,
        Some(&"string") => SyntaxTokenKind::String,
        Some(&"number") => SyntaxTokenKind::Number,
        Some(&"function") => SyntaxTokenKind::Function,
        Some(&"comment") => SyntaxTokenKind::Comment,
        Some(&"operator") => SyntaxTokenKind::Operator,
        _ => SyntaxTokenKind::Plain,
    }
}

fn events_to_spans(
    events: impl Iterator<Item = Result<HighlightEvent, tree_sitter_highlight::Error>>,
) -> Vec<HighlightSpan> {
    let mut spans = Vec::new();
    let mut active: Option<SyntaxTokenKind> = None;

    for event in events {
        match event.expect("highlight event") {
            HighlightEvent::Source { start, end } if end > start => {
                spans.push(HighlightSpan {
                    start_byte: start,
                    end_byte: end,
                    kind: active.unwrap_or(SyntaxTokenKind::Plain),
                });
            }
            HighlightEvent::HighlightStart(h) => active = Some(kind_from_highlight(h)),
            HighlightEvent::HighlightEnd => active = None,
            HighlightEvent::Source { .. } => {}
        }
    }
    spans.retain(|s| s.end_byte > s.start_byte);
    spans
}

fn apply_dialect_keywords(source: &str, dialect: SqlDialect, spans: &mut Vec<HighlightSpan>) {
    // Dialect overlay is O(keywords × bytes); skip on very large buffers (grammar still highlights).
    if source.len() > 256 * 1024 {
        return;
    }
    let keywords: &[&str] = match dialect {
        SqlDialect::Postgres => PG_DIALECT_KEYWORDS,
        SqlDialect::Mysql => MYSQL_DIALECT_KEYWORDS,
    };
    for kw in keywords {
        for mat in source.match_indices(kw) {
            let (start, _) = mat;
            let end = start + kw.len();
            if !is_word_boundary(source, start, end) {
                continue;
            }
            if span_kind_at(spans, start).is_some_and(|k| k != SyntaxTokenKind::Plain) {
                continue;
            }
            overlay_span(spans, start, end, SyntaxTokenKind::Keyword);
        }
    }
    for mat in source.match_indices("NULL") {
        let (start, _) = mat;
        let end = start + 4;
        if is_word_boundary(source, start, end) {
            overlay_span(spans, start, end, SyntaxTokenKind::Null);
        }
    }
}

fn span_kind_at(spans: &[HighlightSpan], byte: usize) -> Option<SyntaxTokenKind> {
    spans
        .iter()
        .find(|s| byte >= s.start_byte && byte < s.end_byte)
        .map(|s| s.kind)
}

fn overlay_span(spans: &mut Vec<HighlightSpan>, start: usize, end: usize, kind: SyntaxTokenKind) {
    let mut out = Vec::new();
    for span in spans.drain(..) {
        if span.end_byte <= start || span.start_byte >= end {
            out.push(span);
            continue;
        }
        if span.start_byte < start {
            out.push(HighlightSpan {
                start_byte: span.start_byte,
                end_byte: start,
                kind: span.kind,
            });
        }
        let mid_start = span.start_byte.max(start);
        let mid_end = span.end_byte.min(end);
        if mid_end > mid_start {
            out.push(HighlightSpan {
                start_byte: mid_start,
                end_byte: mid_end,
                kind,
            });
        }
        if span.end_byte > end {
            out.push(HighlightSpan {
                start_byte: end,
                end_byte: span.end_byte,
                kind: span.kind,
            });
        }
    }
    *spans = out;
}

fn coalesce_spans(spans: &mut Vec<HighlightSpan>) {
    if spans.is_empty() {
        return;
    }
    spans.sort_by_key(|s| (s.start_byte, s.end_byte));
    let mut merged = vec![spans[0].clone()];
    for span in spans.iter().skip(1) {
        let last = merged.last_mut().expect("merged non-empty");
        if last.kind == span.kind && span.start_byte <= last.end_byte {
            last.end_byte = last.end_byte.max(span.end_byte);
        } else {
            merged.push(span.clone());
        }
    }
    *spans = merged;
}

fn is_word_boundary(source: &str, start: usize, end: usize) -> bool {
    let bytes = source.as_bytes();
    let prev_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric() && bytes[start - 1] != b'_';
    let next_ok = end >= bytes.len()
        || (!bytes[end].is_ascii_alphanumeric() && bytes[end] != b'_');
    prev_ok && next_ok
}

/// Split a line's UTF-8 slice into render segments using document-byte spans.
pub fn spans_for_line(
    line: &str,
    line_start_byte: usize,
    all: &[HighlightSpan],
) -> Vec<(SyntaxTokenKind, String)> {
    let line_end = line_start_byte + line.len();
    let mut segs = Vec::new();
    let mut cursor = line_start_byte;
    for span in all {
        if span.end_byte <= line_start_byte {
            continue;
        }
        if span.start_byte >= line_end {
            break;
        }
        let seg_start = span.start_byte.max(line_start_byte);
        let seg_end = span.end_byte.min(line_end);
        if seg_start > cursor {
            let plain = byte_slice(line, line_start_byte, cursor, seg_start);
            if !plain.is_empty() {
                segs.push((SyntaxTokenKind::Plain, plain.to_string()));
            }
        }
        let text = byte_slice(line, line_start_byte, seg_start, seg_end);
        if !text.is_empty() {
            segs.push((span.kind, text.to_string()));
        }
        cursor = seg_end;
    }
    if cursor < line_end {
        let plain = byte_slice(line, line_start_byte, cursor, line_end);
        if !plain.is_empty() {
            segs.push((SyntaxTokenKind::Plain, plain.to_string()));
        }
    }
    if segs.is_empty() && !line.is_empty() {
        segs.push((SyntaxTokenKind::Plain, line.to_string()));
    }
    segs
}

fn byte_slice(line: &str, line_start: usize, abs_start: usize, abs_end: usize) -> &str {
    let a = abs_start.saturating_sub(line_start);
    let b = abs_end.saturating_sub(line_start);
    let a = a.min(line.len());
    let b = b.min(line.len());
    &line[a..b.max(a)]
}

const PG_DIALECT_KEYWORDS: &[&str] = &[
    "ILIKE", "RETURNING", "CONFLICT", "SERIAL", "BIGSERIAL", "SMALLSERIAL", "JSONB", "PLPGSQL",
    "VACUUM", "ANALYZE", "REINDEX", "UNLOGGED", "MATERIALIZED", "CONCURRENTLY",
];

const MYSQL_DIALECT_KEYWORDS: &[&str] = &[
    "STRAIGHT_JOIN", "SQL_CALC_FOUND_ROWS", "DUPLICATE", "AUTO_INCREMENT", "UNSIGNED",
    "ZEROFILL", "TINYINT", "MEDIUMINT", "LONGTEXT", "MEDIUMTEXT", "TINYTEXT", "DATETIME",
    "REPLACE", "DELIMITER", "ENGINE", "CHARSET",
];

fn kind_label(kind: SyntaxTokenKind) -> &'static str {
    match kind {
        SyntaxTokenKind::Plain => "plain",
        SyntaxTokenKind::Keyword => "keyword",
        SyntaxTokenKind::String => "string",
        SyntaxTokenKind::Number => "number",
        SyntaxTokenKind::Function => "function",
        SyntaxTokenKind::Comment => "comment",
        SyntaxTokenKind::Null => "null",
        SyntaxTokenKind::Operator => "operator",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_spans(source: &str, dialect: SqlDialect) -> Vec<(usize, usize, SyntaxTokenKind)> {
        let mut h = SqlSyntaxHighlighter::new(dialect);
        h.highlight(source)
            .into_iter()
            .map(|s| (s.start_byte, s.end_byte, s.kind))
            .collect()
    }

    #[test]
    fn highlights_keywords_strings_comments() {
        let sql = "SELECT 1 AS x; /* note */\n'hi'";
        let spans = snapshot_spans(sql, SqlDialect::Postgres);
        assert!(spans.iter().any(|(_, _, k)| *k == SyntaxTokenKind::Keyword));
        assert!(spans.iter().any(|(_, _, k)| *k == SyntaxTokenKind::Comment));
        assert!(spans.iter().any(|(_, _, k)| *k == SyntaxTokenKind::String));
    }

    #[test]
    fn postgres_dialect_keyword_overlay() {
        let sql = "SELECT * FROM t WHERE name ILIKE '%a%' RETURNING id";
        let spans = snapshot_spans(sql, SqlDialect::Postgres);
        let ilike = sql.find("ILIKE").unwrap();
        assert!(spans.iter().any(|(s, e, k)| {
            *k == SyntaxTokenKind::Keyword && *s <= ilike && *e >= ilike + 5
        }));
    }

    #[test]
    fn mysql_dialect_keyword_overlay() {
        let sql = "SELECT SQL_CALC_FOUND_ROWS id FROM t";
        let spans = snapshot_spans(sql, SqlDialect::Mysql);
        let ix = sql.find("SQL_CALC_FOUND_ROWS").unwrap();
        assert!(spans.iter().any(|(s, e, k)| {
            *k == SyntaxTokenKind::Keyword && *s <= ix && *e >= ix + 19
        }));
    }

    #[test]
    fn snapshot_span_labels() {
        let sql = "SELECT count(*) FROM users WHERE active;\n/* block */\n\"x\"";
        let mut h = SqlSyntaxHighlighter::new(SqlDialect::Postgres);
        let labeled: Vec<String> = h
            .highlight(sql)
            .into_iter()
            .map(|s| {
                format!(
                    "{}..{}={}",
                    s.start_byte,
                    s.end_byte,
                    kind_label(s.kind)
                )
            })
            .collect();
        assert!(labeled.iter().any(|l| l.ends_with("=keyword")));
        assert!(labeled.iter().any(|l| l.ends_with("=comment")));
        assert!(labeled.iter().any(|l| l.ends_with("=function")));
    }

    #[test]
    fn highlight_latency_10k_lines() {
        use std::time::{Duration, Instant};
        let source: String = (0..10_000)
            .map(|i| format!("SELECT {i}, 'text' FROM t WHERE id = {i}; -- c\n"))
            .collect();
        let mut h = SqlSyntaxHighlighter::new(SqlDialect::Postgres);
        let start = Instant::now();
        let spans = h.highlight(&source);
        let elapsed = start.elapsed();
        assert!(!spans.is_empty());
        let budget = Duration::from_millis(if cfg!(debug_assertions) { 512 } else { 16 });
        assert!(
            elapsed < budget,
            "highlight took {:?}, budget {:?}",
            elapsed,
            budget
        );
    }

    #[test]
    fn spans_for_line_splits_utf8() {
        let sql = "SELECT 'a';";
        let mut h = SqlSyntaxHighlighter::new(SqlDialect::Postgres);
        let all = h.highlight(sql);
        let line = "SELECT 'a';";
        let segs = spans_for_line(line, 0, &all);
        assert!(segs.len() >= 2);
    }
}
