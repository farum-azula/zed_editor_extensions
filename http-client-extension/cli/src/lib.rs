//! The `zed-http` runner and language server behind the Zed HTTP Client extension.
//!
//! The binary is a thin wrapper around [`cli::run_main`]; everything else lives here so it can
//! be driven directly from tests.

pub mod catalog;
pub mod cli;
pub mod endpoint;
pub mod infer;
pub mod lsp;
pub mod parser;
pub mod pattern;
pub mod vars;
