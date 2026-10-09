//! SQL editor core: rope buffer, editing, statement split (LUM-030).

mod classify;
mod completion;
mod editor;
mod explain_tree;
mod highlight;
mod selection;
mod statement_split;

pub use completion::{
    complete_at_cursor, CompletionContext, CompletionItem, CompletionItemKind, RecentCompletionUse,
    SnippetSuggestion,
};
pub use editor::SqlEditor;
pub use highlight::{
    spans_for_line, HighlightSpan, SqlDialect, SqlSyntaxHighlighter, SyntaxTokenKind,
};
pub use selection::Selection;
pub use classify::{sql_is_explain, sql_is_explain_analyze, sql_returns_rows};
pub use explain_tree::format_explain_json;
pub use statement_split::{
    current_statement_range, split_statements, statement_index_at_cursor, SplitFlavor,
};
