//! Split SQL scripts into executable statements (LUM-030).

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitFlavor {
    /// PostgreSQL-style (dollar quotes, no DELIMITER).
    Standard,
    /// MySQL client scripts with optional DELIMITER directives.
    Mysql,
}

/// Byte ranges `[start, end)` of each statement (trimmed; delimiter excluded).
pub fn split_statements(sql: &str, flavor: SplitFlavor) -> Vec<Range<usize>> {
    let bytes = sql.as_bytes();
    let mut delimiter: Vec<u8> = b";".to_vec();
    let mut out = Vec::new();
    let mut stmt_start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if is_line_start(bytes, i) && flavor == SplitFlavor::Mysql {
            if let Some(new_delim) = parse_delimiter_directive(&sql[i..]) {
                let line_end = sql[i..]
                    .find('\n')
                    .map(|n| i + n + 1)
                    .unwrap_or(bytes.len());
                delimiter = new_delim;
                i = line_end;
                stmt_start = i;
                continue;
            }
        }

        let ch = bytes[i];
        match ch {
            b'\'' => {
                i = skip_single_quoted(bytes, i + 1);
                continue;
            }
            b'"' => {
                i = skip_double_quoted(bytes, i + 1);
                continue;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                i = skip_line_comment(bytes, i + 2);
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = skip_block_comment(bytes, i + 2);
                continue;
            }
            b'$' if flavor == SplitFlavor::Standard => {
                if let Some(end) = skip_dollar_quote(bytes, i) {
                    i = end;
                    continue;
                }
            }
            _ => {}
        }

        if matches_delimiter(bytes, i, &delimiter) {
            let end = i;
            if let Some(range) = trim_range(sql, stmt_start, end) {
                out.push(range);
            }
            i += delimiter.len();
            stmt_start = i;
            continue;
        }

        i += 1;
    }

    if let Some(range) = trim_range(sql, stmt_start, bytes.len()) {
        out.push(range);
    }
    out
}

pub fn statement_index_at_cursor(ranges: &[Range<usize>], offset: usize) -> Option<usize> {
    ranges.iter().position(|r| offset >= r.start && offset <= r.end)
}

pub fn current_statement_range(sql: &str, cursor: usize, flavor: SplitFlavor) -> Range<usize> {
    let ranges = split_statements(sql, flavor);
    statement_index_at_cursor(&ranges, cursor)
        .and_then(|ix| ranges.get(ix).cloned())
        .unwrap_or(0..sql.len())
}

fn trim_range(sql: &str, start: usize, end: usize) -> Option<Range<usize>> {
    let slice = &sql[start..end.min(sql.len())];
    if slice.trim().is_empty() {
        return None;
    }
    let leading = slice.len() - slice.trim_start().len();
    let trailing = slice.len() - slice.trim_end().len();
    Some(start + leading..start + slice.len() - trailing)
}

fn is_line_start(bytes: &[u8], i: usize) -> bool {
    let mut j = i;
    while j > 0 {
        j -= 1;
        match bytes[j] {
            b'\n' | b'\r' => return true,
            b' ' | b'\t' => continue,
            _ => return false,
        }
    }
    true
}

fn parse_delimiter_directive(rest: &str) -> Option<Vec<u8>> {
    let upper = rest.to_ascii_uppercase();
    if !upper.starts_with("DELIMITER ") {
        return None;
    }
    let tail = rest[10..].trim_start();
    let token: String = tail
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect();
    if token.is_empty() {
        return None;
    }
    Some(token.into_bytes())
}

fn matches_delimiter(bytes: &[u8], i: usize, delimiter: &[u8]) -> bool {
    bytes[i..].starts_with(delimiter)
}

fn skip_single_quoted(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if bytes.get(i + 1) == Some(&b'\'') {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn skip_double_quoted(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if bytes.get(i + 1) == Some(&b'"') {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn skip_line_comment(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    i
}

fn skip_block_comment(bytes: &[u8], mut i: usize) -> usize {
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            return i + 2;
        }
        i += 1;
    }
    bytes.len()
}

fn skip_dollar_quote(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    while i < bytes.len() && bytes[i] != b'$' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let tag = &bytes[start..=i];
    let mut j = i + 1;
    while j + tag.len() <= bytes.len() {
        if bytes[j..].starts_with(tag) {
            return Some(j + tag.len());
        }
        j += 1;
    }
    Some(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semicolon_in_string_does_not_split() {
        let sql = "SELECT ';' AS x;\nSELECT 1;";
        let parts: Vec<_> = split_statements(sql, SplitFlavor::Standard)
            .into_iter()
            .map(|r| sql[r].to_string())
            .collect();
        assert_eq!(parts, vec!["SELECT ';' AS x", "SELECT 1"]);
    }

    #[test]
    fn line_comment_before_semicolon() {
        let sql = "SELECT 1 -- ; drop\n;\nSELECT 2;";
        let parts: Vec<_> = split_statements(sql, SplitFlavor::Standard)
            .into_iter()
            .map(|r| sql[r].to_string())
            .collect();
        assert_eq!(parts, vec!["SELECT 1 -- ; drop", "SELECT 2"]);
    }

    #[test]
    fn block_comment_with_semicolon() {
        let sql = "SELECT 1 /* ; */ ; SELECT 2;";
        let parts: Vec<_> = split_statements(sql, SplitFlavor::Standard)
            .into_iter()
            .map(|r| sql[r].to_string())
            .collect();
        assert_eq!(parts, vec!["SELECT 1 /* ; */", "SELECT 2"]);
    }

    #[test]
    fn dollar_quoted_body() {
        let sql = "SELECT $$ foo; bar $$;\nSELECT 1;";
        let parts: Vec<_> = split_statements(sql, SplitFlavor::Standard)
            .into_iter()
            .map(|r| sql[r].to_string())
            .collect();
        assert_eq!(parts, vec!["SELECT $$ foo; bar $$", "SELECT 1"]);
    }

    #[test]
    fn tagged_dollar_quote() {
        let sql = "SELECT $tag$ x; y $tag$; SELECT 2;";
        let parts: Vec<_> = split_statements(sql, SplitFlavor::Standard)
            .into_iter()
            .map(|r| sql[r].to_string())
            .collect();
        assert_eq!(parts, vec!["SELECT $tag$ x; y $tag$", "SELECT 2"]);
    }

    #[test]
    fn mysql_delimiter_change() {
        let sql = "DELIMITER $$\nCREATE PROCEDURE p() BEGIN SELECT 1; END$$\nDELIMITER ;\nSELECT 2;";
        let parts: Vec<_> = split_statements(sql, SplitFlavor::Mysql)
            .into_iter()
            .map(|r| sql[r].to_string())
            .collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].contains("CREATE PROCEDURE"));
        assert_eq!(parts[1], "SELECT 2");
    }

    #[test]
    fn cursor_statement_lookup() {
        let sql = "SELECT 1;\nSELECT 2;";
        let ranges = split_statements(sql, SplitFlavor::Standard);
        let ix = statement_index_at_cursor(&ranges, 12).expect("second stmt");
        let r = ranges[ix].clone();
        assert_eq!(&sql[r], "SELECT 2");
    }
}
