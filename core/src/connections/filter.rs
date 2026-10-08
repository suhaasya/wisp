//! Fuzzy search over connection name, host, and tags.

use wisp_store::{ConnectionProfile, EnvironmentTag};

use super::view::profile_tags;

/// Case-insensitive fuzzy match: every whitespace-separated query token must appear as a
/// subsequence in at least one searchable field (name, host, user, database, env, tags).
pub fn fuzzy_match(query: &str, profile: &ConnectionProfile) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let haystacks = searchable_fields(profile);
    query
        .split_whitespace()
        .all(|token| haystacks.iter().any(|h| subsequence_match(h, token)))
}

fn searchable_fields(profile: &ConnectionProfile) -> Vec<String> {
    let mut out = vec![
        profile.name.to_lowercase(),
        profile.env_tag.label().to_lowercase(),
    ];
    if let Some(host) = &profile.host {
        out.push(host.to_lowercase());
    }
    if let Some(user) = &profile.user {
        out.push(user.to_lowercase());
    }
    if let Some(db) = &profile.database {
        out.push(db.to_lowercase());
    }
    for tag in profile_tags(profile) {
        out.push(tag.to_lowercase());
    }
    out
}

fn subsequence_match(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut h = haystack.chars();
    for n in needle.to_lowercase().chars() {
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

trait EnvLabel {
    fn label(&self) -> &str;
}

impl EnvLabel for EnvironmentTag {
    fn label(&self) -> &str {
        match self {
            EnvironmentTag::Development => "development",
            EnvironmentTag::Staging => "staging",
            EnvironmentTag::Production => "production",
            EnvironmentTag::Custom(s) => s.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_store::ConnectionEngine;

    #[test]
    fn empty_query_matches_all() {
        let p = ConnectionProfile::new("shop", ConnectionEngine::PostgreSql);
        assert!(fuzzy_match("", &p));
        assert!(fuzzy_match("   ", &p));
    }

    #[test]
    fn token_must_match_subsequence() {
        let mut p = ConnectionProfile::new("shop_prod", ConnectionEngine::PostgreSql);
        p.host = Some("db-prod-1.internal".into());
        assert!(fuzzy_match("shop prod", &p));
        assert!(fuzzy_match("dbprod", &p));
        assert!(!fuzzy_match("billing", &p));
    }
}
