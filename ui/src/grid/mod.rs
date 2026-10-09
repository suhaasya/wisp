//! Virtualised read-only data grid (LUM-024).

mod cell;
mod columns;
mod copy;
mod data_grid;
mod commit_review;
mod edit;
mod filter_bar;
mod layout_cache;
mod row_detail;
mod selection;
mod settings_global;
mod status_global;

pub use data_grid::DataGrid;
pub use settings_global::{init as init_grid_settings, row_detail_width, set_row_detail_width};
pub use status_global::{grid_status, init_grid_status, GridStatusHandle};

pub const MODULE_MARKER: &str = "wisp-ui-grid";
