//! Redact literal values before persisting query text (LUM-034).

/// Strip string/ numeric literals so parameter values are not stored on disk.
pub fn sql_for_history(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\'' {
            out.push('\'');
            out.push('?');
            out.push('\'');
            i = skip_single_quoted(bytes, i + 1);
            continue;
        }
        if b == b'"' {
            out.push('"');
            out.push('?');
            out.push('"');
            i = skip_double_quoted(bytes, i + 1);
            continue;
        }
        if b == b'$' && bytes.get(i + 1) == Some(&b'$') {
            if let Some(end) = skip_dollar_quote(bytes, i) {
                out.push_str("$?$");
                i = end;
                continue;
            }
        }
        if b.is_ascii_digit()
            && (i == 0 || !is_ident_byte(bytes[i - 1]))
            && !is_ident_byte(bytes.get(i + 1).copied().unwrap_or(0))
        {
            out.push('?');
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if bytes.get(i) == Some(&b'.') && bytes.get(i + 1).map(|c| c.is_ascii_digit()) == Some(true) {
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
            }
            continue;
        }
        out.push(char::from(b));
        i += 1;
    }
    out
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
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

fn skip_dollar_quote(bytes: &[u8], start: usize) -> Option<usize> {
    let rest = &bytes[start + 2..];
    let tag_end = rest.iter().position(|&b| b == b'$')? + start + 2;
    let tag = &bytes[start..=tag_end];
    let mut i = tag_end + 1;
    while i < bytes.len() {
        if bytes[i..].starts_with(tag) {
            return Some(i + tag.len());
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_string_literals() {
        let sql = "SELECT * FROM t WHERE name = 'secret' AND id = 42";
        assert_eq!(
            sql_for_history(sql),
            "SELECT * FROM t WHERE name = '?' AND id = ?"
        );
    }
}
