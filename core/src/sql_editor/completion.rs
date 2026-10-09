//! SQL autocomplete (LUM-032).

use std::collections::HashMap;

use tree_sitter::{Node, Parser};

use super::statement_split::{current_statement_range, SplitFlavor};
use crate::schema::SchemaMetadataCache;
use crate::schema::fuzzy_match_name;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemKind {
    Keyword,
    Table,
    Column,
    Function,
    Snippet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub detail: String,
    pub source_table: Option<String>,
    pub score: i32,
    /// When set (snippets), inserted text on accept instead of `label`.
    pub insert_text: Option<String>,
}

impl CompletionItem {
    pub fn text_to_insert(&self) -> &str {
        self.insert_text.as_deref().unwrap_or(&self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionContextKind {
    Keyword,
    Table,
    Column { table: String },
    Function,
}

#[derive(Debug, Clone)]
pub struct CompletionContext {
    pub kind: CompletionContextKind,
    pub prefix: String,
    pub replace_start_byte: usize,
    pub replace_end_byte: usize,
}

#[derive(Debug, Default, Clone)]
pub struct RecentCompletionUse {
    hits: HashMap<String, u32>,
}

impl RecentCompletionUse {
    pub fn bump(&mut self, label: &str) {
        *self.hits.entry(label.to_ascii_lowercase()).or_insert(0) += 1;
    }

    fn boost(&self, label: &str) -> i32 {
        self.hits.get(&label.to_ascii_lowercase()).copied().unwrap_or(0) as i32 * 8
    }
}

#[derive(Debug, Clone)]
pub struct SnippetSuggestion {
    pub name: String,
    pub sql: String,
    pub tags: Vec<String>,
}

pub fn complete_at_cursor(
    sql: &str,
    cursor_byte: usize,
    flavor: SplitFlavor,
    default_schema: &str,
    cache: &mut SchemaMetadataCache,
    recent: &RecentCompletionUse,
    snippets: &[SnippetSuggestion],
) -> (Option<CompletionContext>, Vec<CompletionItem>) {
    let Some(ctx) = detect_context(sql, cursor_byte, flavor) else {
        return (None, Vec::new());
    };
    let mut items = suggest(&ctx, default_schema, cache, recent);
    items.extend(suggest_snippets(&ctx, snippets, recent));
    items.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.label.cmp(&b.label)));
    items.truncate(50);
    (Some(ctx), items)
}

fn suggest_snippets(
    ctx: &CompletionContext,
    snippets: &[SnippetSuggestion],
    recent: &RecentCompletionUse,
) -> Vec<CompletionItem> {
    let prefix = ctx.prefix.to_ascii_lowercase();
    snippets
        .iter()
        .filter(|s| {
            prefix.is_empty()
                || s.name.to_ascii_lowercase().contains(&prefix)
                || s.tags
                    .iter()
                    .any(|t| t.to_ascii_lowercase().contains(&prefix))
        })
        .map(|s| {
            let detail = if s.tags.is_empty() {
                "snippet".into()
            } else {
                format!("snippet · {}", s.tags.join(", "))
            };
            CompletionItem {
                label: s.name.clone(),
                kind: CompletionItemKind::Snippet,
                detail,
                source_table: None,
                score: 140 + recent.boost(&s.name),
                insert_text: Some(s.sql.clone()),
            }
        })
        .collect()
}

fn detect_context(sql: &str, cursor_byte: usize, flavor: SplitFlavor) -> Option<CompletionContext> {
    let cursor_byte = cursor_byte.min(sql.len());
    let stmt_range = current_statement_range(sql, cursor_byte, flavor);
    let stmt = &sql[stmt_range.clone()];
    let stmt_cursor = cursor_byte - stmt_range.start;

    let (_, prefix_probe) = identifier_prefix(stmt, stmt_cursor);
    if prefix_probe.contains('.') {
        return lexical_context(sql, cursor_byte, stmt, stmt_cursor, stmt_range.start);
    }

    if let Some(ctx) = tree_sitter_context(stmt, stmt_cursor) {
        return Some(offset_context(ctx, stmt_range.start));
    }

    lexical_context(sql, cursor_byte, stmt, stmt_cursor, stmt_range.start)
}

fn offset_context(mut ctx: CompletionContext, stmt_start: usize) -> CompletionContext {
    ctx.replace_start_byte += stmt_start;
    ctx.replace_end_byte += stmt_start;
    ctx
}

fn lexical_context(
    _full: &str,
    _cursor_byte: usize,
    stmt: &str,
    stmt_cursor: usize,
    stmt_start: usize,
) -> Option<CompletionContext> {
    let (replace_start, prefix) = identifier_prefix(stmt, stmt_cursor);
    let replace_end = stmt_cursor;

    if let Some(dot) = prefix.rfind('.') {
        let alias = prefix[..dot].to_string();
        let col_prefix = prefix[dot + 1..].to_string();
        let refs = table_refs_in_statement(stmt);
        let table = refs
            .iter()
            .find(|r| {
                r.alias.as_deref() == Some(alias.as_str()) || (r.alias.is_none() && r.name == alias)
            })
            .map(|r| r.name.clone())?;
        return Some(CompletionContext {
            kind: CompletionContextKind::Column { table },
            prefix: col_prefix,
            replace_start_byte: stmt_start + replace_start,
            replace_end_byte: stmt_start + replace_end,
        });
    }

    let before = stmt[..replace_start].trim_end();
    let table_ctx = before
        .to_ascii_uppercase()
        .ends_with("FROM")
        || before.to_ascii_uppercase().ends_with("JOIN")
        || before.to_ascii_uppercase().ends_with("UPDATE")
        || before.to_ascii_uppercase().ends_with("INTO")
        || before.to_ascii_uppercase().ends_with("TABLE");

    if table_ctx {
        return Some(CompletionContext {
            kind: CompletionContextKind::Table,
            prefix: prefix.clone(),
            replace_start_byte: stmt_start + replace_start,
            replace_end_byte: stmt_start + replace_end,
        });
    }

    if is_function_context(before) {
        return Some(CompletionContext {
            kind: CompletionContextKind::Function,
            prefix: prefix.clone(),
            replace_start_byte: stmt_start + replace_start,
            replace_end_byte: stmt_start + replace_end,
        });
    }

    Some(CompletionContext {
        kind: CompletionContextKind::Keyword,
        prefix,
        replace_start_byte: stmt_start + replace_start,
        replace_end_byte: stmt_start + replace_end,
    })
}

fn is_function_context(before: &str) -> bool {
    before.ends_with('(') || before.to_ascii_uppercase().ends_with("SELECT ")
}

fn identifier_prefix(text: &str, cursor: usize) -> (usize, String) {
    let bytes = text.as_bytes();
    let mut start = cursor.min(bytes.len());
    while start > 0 {
        let b = bytes[start - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
            start -= 1;
        } else {
            break;
        }
    }
    (start, text[start..cursor.min(bytes.len())].to_string())
}

#[derive(Debug, Clone)]
struct TableRef {
    name: String,
    alias: Option<String>,
}

fn table_refs_in_statement(stmt: &str) -> Vec<TableRef> {
    let mut out = Vec::new();
    let upper = stmt.to_ascii_uppercase();
    let bytes = stmt.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if upper[i..].starts_with("FROM") && boundary_before(&upper, i) {
            i = parse_table_list(stmt, i + 4, &mut out);
            continue;
        }
        if (upper[i..].starts_with("JOIN") || upper[i..].starts_with("INNER JOIN")
            || upper[i..].starts_with("LEFT JOIN")
            || upper[i..].starts_with("RIGHT JOIN"))
            && boundary_before(&upper, i)
        {
            let advance = if upper[i..].starts_with("INNER JOIN") {
                10
            } else if upper[i..].starts_with("LEFT JOIN") {
                9
            } else if upper[i..].starts_with("RIGHT JOIN") {
                10
            } else {
                4
            };
            i = parse_single_table(stmt, i + advance, &mut out);
            continue;
        }
        i += 1;
    }
    out
}

fn boundary_before(upper: &str, i: usize) -> bool {
    i == 0 || !upper.as_bytes()[i - 1].is_ascii_alphanumeric()
}

fn skip_ws(s: &str, mut i: usize) -> usize {
    while i < s.len() && s.as_bytes()[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn parse_table_list(stmt: &str, mut i: usize, out: &mut Vec<TableRef>) -> usize {
    i = skip_ws(stmt, i);
    loop {
        i = parse_single_table(stmt, i, out);
        i = skip_ws(stmt, i);
        if stmt.as_bytes().get(i) == Some(&b',') {
            i += 1;
            i = skip_ws(stmt, i);
            continue;
        }
        break;
    }
    i
}

fn parse_single_table(stmt: &str, mut i: usize, out: &mut Vec<TableRef>) -> usize {
    i = skip_ws(stmt, i);
    let start = i;
    while i < stmt.len() {
        let b = stmt.as_bytes()[i];
        if b.is_ascii_alphanumeric() || b == b'_' {
            i += 1;
        } else {
            break;
        }
    }
    if i == start {
        return i;
    }
    let name = stmt[start..i].to_string();
    i = skip_ws(stmt, i);
    let mut alias = None;
    if stmt[i..].starts_with("AS ") || stmt[i..].starts_with("as ") {
        i = skip_ws(stmt, i + 2);
        let a_start = i;
        while i < stmt.len() && (stmt.as_bytes()[i].is_ascii_alphanumeric() || stmt.as_bytes()[i] == b'_') {
            i += 1;
        }
        if i > a_start {
            alias = Some(stmt[a_start..i].to_string());
        }
    } else if i < stmt.len() && stmt.as_bytes()[i].is_ascii_alphabetic() {
        let a_start = i;
        while i < stmt.len() && (stmt.as_bytes()[i].is_ascii_alphanumeric() || stmt.as_bytes()[i] == b'_') {
            i += 1;
        }
        if i > a_start && !matches!(stmt[a_start..i].to_ascii_uppercase().as_str(), "ON" | "WHERE" | "JOIN" | "LEFT" | "RIGHT" | "INNER" | "ORDER" | "GROUP" | "LIMIT") {
            alias = Some(stmt[a_start..i].to_string());
        } else {
            i = a_start;
        }
    }
    out.push(TableRef { name, alias });
    i
}

fn tree_sitter_context(stmt: &str, stmt_cursor: usize) -> Option<CompletionContext> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_sequel::LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(stmt, None)?;
    let node = tree
        .root_node()
        .descendant_for_byte_range(stmt_cursor, stmt_cursor)?;
    classify_node(&tree, node, stmt, stmt_cursor)
}

fn classify_node(tree: &tree_sitter::Tree, node: Node, stmt: &str, stmt_cursor: usize) -> Option<CompletionContext> {
    let (replace_start, prefix) = identifier_prefix(stmt, stmt_cursor);
    let replace_end = stmt_cursor;
    let mut cursor = tree.walk();
    cursor.reset(node);
    loop {
        let n = cursor.node();
        match n.kind() {
            "relation" | "object_reference" => {
                return Some(CompletionContext {
                    kind: CompletionContextKind::Table,
                    prefix,
                    replace_start_byte: replace_start,
                    replace_end_byte: replace_end,
                });
            }
            "field" => {
                let refs = table_refs_in_statement(stmt);
                let table = refs.first()?.name.clone();
                return Some(CompletionContext {
                    kind: CompletionContextKind::Column { table },
                    prefix,
                    replace_start_byte: replace_start,
                    replace_end_byte: replace_end,
                });
            }
            "invocation" => {
                return Some(CompletionContext {
                    kind: CompletionContextKind::Function,
                    prefix,
                    replace_start_byte: replace_start,
                    replace_end_byte: replace_end,
                });
            }
            _ => {}
        }
        if !cursor.goto_parent() {
            break;
        }
    }
    None
}

fn suggest(
    ctx: &CompletionContext,
    default_schema: &str,
    cache: &mut SchemaMetadataCache,
    recent: &RecentCompletionUse,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    match ctx.kind {
        CompletionContextKind::Keyword => {
            for kw in SQL_KEYWORDS {
                if fuzzy_match_name(kw, &ctx.prefix) {
                    items.push(CompletionItem {
                        label: (*kw).into(),
                        kind: CompletionItemKind::Keyword,
                        detail: "keyword".into(),
                        source_table: None,
                        score: 100 + recent.boost(kw),
                        insert_text: None,
                    });
                }
            }
            for name in cache.function_names(default_schema) {
                if fuzzy_match_name(&name, &ctx.prefix) {
                    items.push(CompletionItem {
                        label: name.clone(),
                        kind: CompletionItemKind::Function,
                        detail: "function".into(),
                        source_table: None,
                        score: 115 + recent.boost(&name),
                        insert_text: None,
                    });
                }
            }
        }
        CompletionContextKind::Table => {
            let names: Vec<String> = cache
                .table_names(default_schema)
                .into_iter()
                .filter(|name| fuzzy_match_name(name, &ctx.prefix))
                .collect();
            for name in names {
                cache.ensure_columns(default_schema, &name);
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: CompletionItemKind::Table,
                    detail: "table".into(),
                    source_table: None,
                    score: 120 + recent.boost(&name),
                    insert_text: None,
                });
            }
        }
        CompletionContextKind::Column { ref table } => {
            let cols = cache.ensure_columns(default_schema, table);
            for col in cols {
                if fuzzy_match_name(&col.name, &ctx.prefix) {
                    items.push(CompletionItem {
                        label: col.name.clone(),
                        kind: CompletionItemKind::Column,
                        detail: col.type_name.clone(),
                        source_table: Some(table.to_string()),
                        score: 130 + recent.boost(&col.name),
                        insert_text: None,
                    });
                }
            }
        }
        CompletionContextKind::Function => {
            for name in cache.function_names(default_schema) {
                if fuzzy_match_name(&name, &ctx.prefix) {
                    items.push(CompletionItem {
                        label: name.clone(),
                        kind: CompletionItemKind::Function,
                        detail: "function".into(),
                        source_table: None,
                        score: 110 + recent.boost(&name),
                        insert_text: None,
                    });
                }
            }
        }
    }
    items.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.label.cmp(&b.label)));
    items.truncate(50);
    items
}

const SQL_KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "JOIN", "LEFT", "RIGHT", "INNER", "ON", "AND", "OR", "NOT", "IN",
    "IS", "NULL", "ORDER", "BY", "GROUP", "HAVING", "LIMIT", "OFFSET", "INSERT", "INTO", "VALUES",
    "UPDATE", "SET", "DELETE", "CREATE", "TABLE", "VIEW", "INDEX", "AS", "DISTINCT", "UNION",
    "ALL", "CASE", "WHEN", "THEN", "ELSE", "END", "WITH", "RETURNING",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{demo_catalog_shop, SchemaMetadataCache};

    #[derive(Clone, Copy)]
    struct FixtureCase {
        sql: &'static str,
        cursor: usize,
        expect_kind: ExpectKind,
        expect_label: &'static str,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ExpectKind {
        Keyword,
        Table,
        Column,
        Function,
    }

    fn run_case(case: FixtureCase, cache: &mut SchemaMetadataCache) -> bool {
        let (ctx, items) = complete_at_cursor(
            case.sql,
            case.cursor,
            SplitFlavor::Standard,
            "public",
            cache,
            &RecentCompletionUse::default(),
            &[],
        );
        let Some(ctx) = ctx else { return false };
        if !kind_matches(&ctx.kind, case.expect_kind) {
            return false;
        }
        items.iter().any(|i| i.label.eq_ignore_ascii_case(case.expect_label))
    }

    fn kind_matches(got: &CompletionContextKind, expect: ExpectKind) -> bool {
        matches!(
            (got, expect),
            (CompletionContextKind::Keyword, ExpectKind::Keyword)
                | (CompletionContextKind::Table, ExpectKind::Table)
                | (CompletionContextKind::Column { .. }, ExpectKind::Column)
                | (CompletionContextKind::Function, ExpectKind::Function)
        )
    }

    #[test]
    fn alias_column_resolution() {
        let sql = "SELECT o. FROM orders o";
        let cursor = sql.find('.').map(|i| i + 1).unwrap();
        let mut cache = SchemaMetadataCache::new(demo_catalog_shop());
        let (ctx, items) = complete_at_cursor(
            sql,
            cursor,
            SplitFlavor::Standard,
            "public",
            &mut cache,
            &RecentCompletionUse::default(),
            &[],
        );
        let ctx = ctx.expect("context");
        assert!(matches!(ctx.kind, CompletionContextKind::Column { .. }));
        assert!(items.iter().any(|i| i.label == "id"));
        assert!(items.iter().any(|i| i.source_table.as_deref() == Some("orders")));
    }

    #[test]
    fn fixture_thirty_positions() {
        let mut cache = SchemaMetadataCache::new(demo_catalog_shop());
        let cases = fixture_cases();
        assert_eq!(cases.len(), 30);
        let start = std::time::Instant::now();
        for case in &cases {
            assert!(
                run_case(*case, &mut cache), // FixtureCase: Copy
                "failed at cursor {} in {:?}",
                case.cursor,
                case.sql
            );
        }
        assert!(
            start.elapsed() < std::time::Duration::from_millis(30),
            "30 completions took {:?}",
            start.elapsed()
        );
    }

    fn fixture_cases() -> Vec<FixtureCase> {
        vec![
            FixtureCase { sql: "SEL", cursor: 3, expect_kind: ExpectKind::Keyword, expect_label: "SELECT" },
            FixtureCase { sql: "SELECT ", cursor: 7, expect_kind: ExpectKind::Keyword, expect_label: "FROM" },
            FixtureCase { sql: "SELECT * FROM ord", cursor: 17, expect_kind: ExpectKind::Table, expect_label: "orders" },
            FixtureCase { sql: "SELECT * FROM orders o WHERE ", cursor: 29, expect_kind: ExpectKind::Keyword, expect_label: "WHERE" },
            FixtureCase { sql: "SELECT o.id FROM orders o", cursor: 10, expect_kind: ExpectKind::Column, expect_label: "id" },
            FixtureCase { sql: "SELECT o.created_at FROM orders o", cursor: 19, expect_kind: ExpectKind::Column, expect_label: "created_at" },
            FixtureCase { sql: "SELECT c. FROM customers c", cursor: 9, expect_kind: ExpectKind::Column, expect_label: "email" },
            FixtureCase { sql: "SELECT * FROM products p JOIN order_items oi ON", cursor: 45, expect_kind: ExpectKind::Keyword, expect_label: "ON" },
            FixtureCase { sql: "SELECT * FROM products", cursor: 22, expect_kind: ExpectKind::Table, expect_label: "products" },
            FixtureCase { sql: "SELECT * FROM customers", cursor: 23, expect_kind: ExpectKind::Table, expect_label: "customers" },
            FixtureCase { sql: "INSERT INTO ord", cursor: 15, expect_kind: ExpectKind::Table, expect_label: "orders" },
            FixtureCase { sql: "UPDATE ord", cursor: 11, expect_kind: ExpectKind::Table, expect_label: "orders" },
            FixtureCase { sql: "DELETE FROM ord", cursor: 15, expect_kind: ExpectKind::Table, expect_label: "orders" },
            FixtureCase { sql: "SELECT set_updat", cursor: 15, expect_kind: ExpectKind::Keyword, expect_label: "set_updated_at" },
            FixtureCase { sql: "SELECT * FROM daily_rev", cursor: 23, expect_kind: ExpectKind::Table, expect_label: "daily_revenue" },
            FixtureCase { sql: "SELECT * FROM order_items oi WHERE oi.", cursor: 38, expect_kind: ExpectKind::Column, expect_label: "product_id" },
            FixtureCase { sql: "SELECT * FROM orders o WHERE o.", cursor: 32, expect_kind: ExpectKind::Column, expect_label: "customer_id" },
            FixtureCase { sql: "SELECT * FROM orders o WHERE o.st", cursor: 32, expect_kind: ExpectKind::Column, expect_label: "status" },
            FixtureCase { sql: "SELECT * FROM products p WHERE p.sk", cursor: 33, expect_kind: ExpectKind::Column, expect_label: "sku" },
            FixtureCase { sql: "SELECT DISTINCT ", cursor: 16, expect_kind: ExpectKind::Keyword, expect_label: "FROM" },
            FixtureCase { sql: "SELECT * FROM orders o JOIN customers c ON c.", cursor: 45, expect_kind: ExpectKind::Column, expect_label: "id" },
            FixtureCase { sql: "SELECT * FROM orders o JOIN customers c ON o.", cursor: 45, expect_kind: ExpectKind::Column, expect_label: "id" },
            FixtureCase { sql: "SELECT COUNT(", cursor: 13, expect_kind: ExpectKind::Function, expect_label: "set_updated_at" },
            FixtureCase { sql: "SELECT * FROM active_cust", cursor: 25, expect_kind: ExpectKind::Table, expect_label: "active_customers" },
            FixtureCase { sql: "SELECT * FROM order_items oi WHERE oi.or", cursor: 40, expect_kind: ExpectKind::Column, expect_label: "order_id" },
            FixtureCase { sql: "SELECT * FROM orders AS o WHERE o.", cursor: 36, expect_kind: ExpectKind::Column, expect_label: "created_at" },
            FixtureCase { sql: "WITH x AS (SELECT 1) SEL", cursor: 24, expect_kind: ExpectKind::Keyword, expect_label: "SELECT" },
            FixtureCase { sql: "SELECT * FROM orders o, customers c WHERE c.", cursor: 46, expect_kind: ExpectKind::Column, expect_label: "name" },
            FixtureCase { sql: "SELECT * FROM order_items oi WHERE oi.quantity", cursor: 42, expect_kind: ExpectKind::Column, expect_label: "quantity" },
            FixtureCase { sql: "SELECT 1 UNION SEL", cursor: 17, expect_kind: ExpectKind::Keyword, expect_label: "SELECT" },
        ]
    }
}
