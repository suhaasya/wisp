//! Fuzzy filter for schema object names (subsequence match, case-insensitive).

pub fn fuzzy_match_name(name: &str, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    query
        .split_whitespace()
        .all(|token| subsequence_match(&name.to_lowercase(), &token.to_lowercase()))
}

/// Character indices in `name` to emphasize when `query` is non-empty and matches.
pub fn fuzzy_match_highlight_indices(name: &str, query: &str) -> Vec<usize> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let lower: Vec<char> = name.to_lowercase().chars().collect();
    let mut indices = Vec::new();
    for token in query.split_whitespace() {
        let needle: Vec<char> = token.to_lowercase().chars().collect();
        if needle.is_empty() {
            continue;
        }
        let mut start = 0usize;
        for &n in &needle {
            let mut found = None;
            for (i, &c) in lower.iter().enumerate().skip(start) {
                if c == n {
                    found = Some(i);
                    break;
                }
            }
            let Some(i) = found else {
                return Vec::new();
            };
            indices.push(i);
            start = i + 1;
        }
    }
    indices.sort_unstable();
    indices.dedup();
    indices
}

fn subsequence_match(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut h = haystack.chars();
    for n in needle.chars() {
        loop {
            match h.next() {
                Some(c) if c == n => break,
                Some(_) => {}
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_subsequence() {
        assert!(fuzzy_match_name("order_items", "ord"));
        assert!(fuzzy_match_name("daily_revenue", "dr"));
        assert!(!fuzzy_match_name("customers", "xyz"));
    }
}
