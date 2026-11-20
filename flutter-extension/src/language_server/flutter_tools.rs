//! The `flutter-tools` language server, `tool/flutter_lsp.dart`: it offers the snippets
//! (`tool/snippets.json`) as completions that add the imports they need, keeps the runners of the
//! tasks in `~/.zed-flutter/` up to date, and keeps `build_runner watch` running.

use zed_extension_api::{self as zed, Worktree};

use super::LanguageServer;
use crate::registry::Named;
use crate::sdk::DART;
use crate::tool_files::ToolFiles;

/// The server, one of the installed tool files.
const SERVER: &str = "flutter_lsp.dart";

pub struct FlutterToolsServer {
    tools: ToolFiles,
}

impl FlutterToolsServer {
    pub fn new(tools: ToolFiles) -> Self {
        Self { tools }
    }
}

impl Named for FlutterToolsServer {
    fn name(&self) -> &'static str {
        "flutter-tools"
    }
}

impl LanguageServer for FlutterToolsServer {
    fn command(&self, worktree: &Worktree) -> zed::Result<zed::Command> {
        let server = self.tools.install()?.path(SERVER)?;
        Ok(zed::Command {
            command: DART.find(worktree)?,
            args: vec![server],
            env: worktree.shell_env(),
        })
    }
}
