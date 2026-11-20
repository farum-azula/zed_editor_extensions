//! The tree-sitter-http grammar at the commit `extension.toml` pins, compiled from C so that the
//! tests can run the queries of `languages/http/` against the grammar Zed builds.

use tree_sitter_language::LanguageFn;

extern "C" {
    fn tree_sitter_http() -> *const ();
}

/// The grammar, for `tree_sitter::Language::new`.
// SAFETY: `tree_sitter_http` is the language function of the parser `build.rs` compiles.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_http) };
