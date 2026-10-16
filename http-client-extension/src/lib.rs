//! The Zed side of the extension. It has one job: hand Zed the command that starts the
//! `zed-http` language server — and install that runner when the machine does not have it,
//! so that installing the extension is all a user has to do.
//!
//! In order: a `zed-http` already in `PATH`, a copy installed earlier, a release build
//! downloaded from GitHub, or a build from the `cli/` crate when the extension is a clone of
//! the repository (a development extension, where cargo is present by definition).

use std::fs;
use zed_extension_api::{self as zed, LanguageServerId, Result};

const BIN: &str = "zed-http";
/// Prebuilt runners are published here, in a release tagged `http-client-extension-v<version>`.
const REPO: &str = "farum-azula/zed_editor_extensions";
/// The extension version: CI checks that `Cargo.toml` and `extension.toml` agree on it.
const VERSION: &str = env!("CARGO_PKG_VERSION");

struct HttpClientExtension;

impl zed::Extension for HttpClientExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        Ok(zed::Command {
            command: runner(id, worktree)?,
            args: vec!["lsp".into()],
            env: worktree.shell_env(),
        })
    }
}

/// Path to the runner, installing it if needed. Relative paths are resolved by Zed against the
/// extension's work directory, which is also what this extension's own file access sees.
fn runner(id: &LanguageServerId, worktree: &zed::Worktree) -> Result<String> {
    if let Some(path) = worktree.which(BIN) {
        return Ok(path);
    }
    let dir = format!("{BIN}-{VERSION}");
    let path = format!("{dir}/bin/{BIN}{}", exe_suffix());
    if fs::metadata(&path).is_ok_and(|m| m.is_file()) {
        return Ok(path);
    }
    zed::set_language_server_installation_status(
        id,
        &zed::LanguageServerInstallationStatus::CheckingForUpdate,
    );
    if let Err(release) = download(id, &dir) {
        build(&dir).map_err(|source| {
            let status = zed::LanguageServerInstallationStatus::Failed(source.clone());
            zed::set_language_server_installation_status(id, &status);
            format!(
                "could not install {BIN} {VERSION}: no prebuilt runner ({release}), \
                 and building one from source failed ({source}). Install it by hand with \
                 `cargo install --path cli` in a clone of {REPO}."
            )
        })?;
    }
    zed::make_file_executable(&path)?;
    zed::set_language_server_installation_status(id, &zed::LanguageServerInstallationStatus::None);
    // Older versions are of no use once this one is in place.
    for stale in fs::read_dir(".").into_iter().flatten().flatten() {
        let name = stale.file_name().to_string_lossy().into_owned();
        if name.starts_with(&format!("{BIN}-")) && name != dir {
            fs::remove_dir_all(stale.path()).ok();
        }
    }
    Ok(path)
}

/// Unpacks the release build for this platform into `<dir>/bin`.
fn download(id: &LanguageServerId, dir: &str) -> Result<()> {
    let (os, arch) = zed::current_platform();
    let asset = format!("{BIN}-{VERSION}-{}-{}.tar.gz", arch_name(arch), os_name(os));
    let tag = format!("http-client-extension-v{VERSION}");
    let release = zed::github_release_by_tag_name(REPO, &tag)?;
    let url = release
        .assets
        .into_iter()
        .find(|a| a.name == asset)
        .ok_or_else(|| format!("release {tag} has no asset {asset}"))?
        .download_url;
    zed::set_language_server_installation_status(
        id,
        &zed::LanguageServerInstallationStatus::Downloading,
    );
    zed::download_file(&url, &format!("{dir}/bin"), zed::DownloadedFileType::GzipTar)
}

/// Builds the runner from the `cli/` crate of the extension itself, which only a clone of the
/// repository has. `cargo install --root` lays the binary out exactly like the release archive.
fn build(dir: &str) -> Result<()> {
    let work = std::env::current_dir().map_err(|e| e.to_string())?;
    let work = work.to_string_lossy();
    let source = source_dir(&work).ok_or("the extension is not installed next to its work directory")?;
    let output = zed::process::Command::new("cargo")
        .args(["install", "--quiet", "--path", &format!("{source}/cli"), "--root", &format!("{work}/{dir}")])
        .output()?;
    match output.status {
        Some(0) => Ok(()),
        _ => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
}

/// `<extensions>/work/<id>` → `<extensions>/installed/<id>`, where Zed keeps (or, for a
/// development extension, symlinks) the extension's own files.
fn source_dir(work_dir: &str) -> Option<String> {
    let (work, id) = work_dir.trim_end_matches('/').rsplit_once('/')?;
    Some(format!("{}/installed/{id}", work.strip_suffix("/work")?))
}

fn os_name(os: zed::Os) -> &'static str {
    match os {
        zed::Os::Mac => "apple-darwin",
        zed::Os::Linux => "unknown-linux-gnu",
        zed::Os::Windows => "pc-windows-msvc",
    }
}

fn arch_name(arch: zed::Architecture) -> &'static str {
    match arch {
        zed::Architecture::Aarch64 => "aarch64",
        zed::Architecture::X8664 => "x86_64",
        zed::Architecture::X86 => "i686",
    }
}

fn exe_suffix() -> &'static str {
    match zed::current_platform().0 {
        zed::Os::Windows => ".exe",
        _ => "",
    }
}

zed::register_extension!(HttpClientExtension);
