//! The language servers (`[language_servers.<name>]` of `extension.toml`), one type each.

mod dart;
mod flutter_tools;

pub use dart::DartLanguageServer;
pub use flutter_tools::FlutterToolsServer;

use zed_extension_api::serde_json::Value;
use zed_extension_api::settings::LspSettings;
use zed_extension_api::{self as zed, lsp::Completion, CodeLabel, Worktree};

use crate::registry::Named;

pub trait LanguageServer: Named {
    /// How Zed starts the server.
    fn command(&self, worktree: &Worktree) -> zed::Result<zed::Command>;

    /// The `workspace/configuration` the server gets.
    fn workspace_configuration(&self, _worktree: &Worktree) -> zed::Result<Option<Value>> {
        Ok(None)
    }

    /// How a completion of the server is shown.
    fn label_for_completion(&self, _completion: Completion) -> Option<CodeLabel> {
        None
    }
}

/// The user's settings of [server] (`lsp.<server>`).
fn lsp_settings(server: &str, worktree: &Worktree) -> zed::Result<LspSettings> {
    LspSettings::for_worktree(server, worktree)
        .map_err(|e| format!("could not read the settings lsp.{server}: {e}"))
}
