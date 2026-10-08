//! Single-line text inputs backed by [`SingleLineEditor`].

mod editor;
mod view;

pub use editor::SingleLineEditor;
pub use view::{bind_text_input_keys, TextInput, TextInputKind};
