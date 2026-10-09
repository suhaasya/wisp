//! Named SQL snippets (LUM-034).

mod schema;
mod store;

pub use schema::{Snippet, SnippetsFile, SNIPPETS_VERSION};
pub use store::{SnippetStore, SnippetStoreError};
