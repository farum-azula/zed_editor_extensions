use zed_extension_api::{self as zed, LanguageServerId, Result};

struct HttpClientExtension;

impl zed::Extension for HttpClientExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let path = worktree
            .which("zed-http")
            .ok_or_else(|| "zed-http not found in PATH, install it with `cargo install --path cli`".to_string())?;
        Ok(zed::Command {
            command: path,
            args: vec!["lsp".into()],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(HttpClientExtension);
