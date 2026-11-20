//! The Zed side of the extension. It has one job: hand Zed the command that starts the
//! `zed-http` language server — and install that runner when the machine does not have it,
//! so that installing the extension is all a user has to do.
//!
//! The runner comes from the first of [`SOURCES`] that has it: a `zed-http` already in `PATH`,
//! a copy installed earlier, a release build downloaded from GitHub, or a build from the `cli/`
//! crate when the extension is a clone of the repository (a development extension, where cargo
//! is present by definition).

use std::fs;
use zed_extension_api::{
    self as zed, LanguageServerId, LanguageServerInstallationStatus as Status, Result,
};

const BIN: &str = "zed-http";
/// Prebuilt runners are published here, in a release tagged `http-client-extension-v<version>`.
const REPO: &str = "farum-azula/zed_editor_extensions";
/// The extension version: a test checks that `Cargo.toml` and `extension.toml` agree on it.
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The directory this version of the runner is installed in, inside the extension's work directory.
const INSTALL_DIR: &str = concat!("zed-http-", env!("CARGO_PKG_VERSION"));

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

/// What the sources of the runner work with.
struct Setup<'a> {
    id: &'a LanguageServerId,
    worktree: &'a zed::Worktree,
    /// Where this version of the runner is installed: relative paths are resolved by Zed against
    /// the extension's work directory, which is also what this extension's own file access sees.
    installed: String,
}

/// A way of getting the runner.
trait RunnerSource {
    /// The runner's path; `Ok(None)` when this source has none to offer.
    fn runner(&self, setup: &Setup) -> Result<Option<String>>;
    /// Whether the runner was installed just now, and still has to be made executable.
    fn installs(&self) -> bool {
        false
    }
    /// How this source's failure reads in the error shown when no source has the runner.
    fn failure(&self, error: &str) -> String {
        error.to_string()
    }
}

/// `zed-http` in the `PATH` of the worktree's shell.
struct InPath;
/// The copy of this version installed earlier.
struct Installed;
/// The prebuilt runner of this version from the repository's releases.
struct Release;
/// A build of the `cli/` crate, which only a clone of the repository has.
struct SourceBuild;

/// Asked in order; the first source that has the runner wins.
const SOURCES: [&dyn RunnerSource; 4] = [&InPath, &Installed, &Release, &SourceBuild];

fn runner(id: &LanguageServerId, worktree: &zed::Worktree) -> Result<String> {
    let setup = Setup {
        id,
        worktree,
        installed: binary_path(INSTALL_DIR, zed::current_platform().0),
    };
    let mut failures = Vec::new();
    let mut last_error = String::new();
    for source in SOURCES {
        match source.runner(&setup) {
            Ok(Some(path)) => {
                if source.installs() {
                    zed::make_file_executable(&path)?;
                    zed::set_language_server_installation_status(id, &Status::None);
                    remove_stale_installs();
                }
                return Ok(path);
            }
            Ok(None) => {}
            Err(error) => {
                failures.push(source.failure(&error));
                last_error = error;
            }
        }
    }
    zed::set_language_server_installation_status(id, &Status::Failed(last_error));
    Err(installation_error(&failures))
}

impl RunnerSource for InPath {
    fn runner(&self, setup: &Setup) -> Result<Option<String>> {
        Ok(setup.worktree.which(BIN))
    }
}

impl RunnerSource for Installed {
    fn runner(&self, setup: &Setup) -> Result<Option<String>> {
        let installed = fs::metadata(&setup.installed).is_ok_and(|metadata| metadata.is_file());
        Ok(installed.then(|| setup.installed.clone()))
    }
}

impl RunnerSource for Release {
    /// Unpacks the release build for this platform into `<install dir>/bin`.
    fn runner(&self, setup: &Setup) -> Result<Option<String>> {
        zed::set_language_server_installation_status(setup.id, &Status::CheckingForUpdate);
        let (os, arch) = zed::current_platform();
        let asset = asset_name(os, arch);
        let tag = release_tag();
        let release = zed::github_release_by_tag_name(REPO, &tag)?;
        let url = release
            .assets
            .into_iter()
            .find(|candidate| candidate.name == asset)
            .ok_or_else(|| format!("release {tag} has no asset {asset}"))?
            .download_url;
        zed::set_language_server_installation_status(setup.id, &Status::Downloading);
        zed::download_file(
            &url,
            &format!("{INSTALL_DIR}/bin"),
            zed::DownloadedFileType::GzipTar,
        )?;
        Ok(Some(setup.installed.clone()))
    }

    fn installs(&self) -> bool {
        true
    }

    fn failure(&self, error: &str) -> String {
        format!("no prebuilt runner ({error})")
    }
}

impl RunnerSource for SourceBuild {
    /// `cargo install --root` lays the binary out exactly like the release archive.
    fn runner(&self, setup: &Setup) -> Result<Option<String>> {
        let work = std::env::current_dir().map_err(|error| error.to_string())?;
        let work = work.to_string_lossy();
        let source =
            source_dir(&work).ok_or("the extension is not installed next to its work directory")?;
        let output = zed::process::Command::new("cargo")
            .args([
                "install",
                "--quiet",
                "--path",
                &format!("{source}/cli"),
                "--root",
                &format!("{work}/{INSTALL_DIR}"),
            ])
            .output()?;
        match output.status {
            Some(0) => Ok(Some(setup.installed.clone())),
            _ => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        }
    }

    fn installs(&self) -> bool {
        true
    }

    fn failure(&self, error: &str) -> String {
        format!("building one from source failed ({error})")
    }
}

/// The error shown when no source has the runner, with why each that tried failed.
fn installation_error(failures: &[String]) -> String {
    format!(
        "could not install {BIN} {VERSION}: {}. Install it by hand with `cargo install --path cli` in a clone of {REPO}.",
        failures.join(", and ")
    )
}

/// Older versions are of no use once this one is in place. Failing to remove one is reported
/// (Zed's log), not fatal.
fn remove_stale_installs() {
    let entries = match fs::read_dir(".") {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("{BIN}: cannot list the work directory to remove older runners: {error}");
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("{BIN}: cannot read the work directory to remove older runners: {error}");
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_stale_install(&name) {
            if let Err(error) = fs::remove_dir_all(entry.path()) {
                eprintln!("{BIN}: cannot remove the older runner {name}: {error}");
            }
        }
    }
}

/// Does the work directory entry `name` hold another version of the runner?
fn is_stale_install(name: &str) -> bool {
    name.starts_with(&format!("{BIN}-")) && name != INSTALL_DIR
}

/// The release the prebuilt runners of this version are attached to.
fn release_tag() -> String {
    format!("http-client-extension-v{VERSION}")
}

/// The release asset of the runner for a platform: `zed-http-<version>-<target>.tar.gz`.
fn asset_name(os: zed::Os, arch: zed::Architecture) -> String {
    format!("{BIN}-{VERSION}-{}-{}.tar.gz", arch_name(arch), os_name(os))
}

/// The runner in the install directory `dir`, as the release archive and `cargo install` lay it out.
fn binary_path(dir: &str, os: zed::Os) -> String {
    format!("{dir}/bin/{BIN}{}", exe_suffix(os))
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

fn exe_suffix(os: zed::Os) -> &'static str {
    match os {
        zed::Os::Windows => ".exe",
        _ => "",
    }
}

zed::register_extension!(HttpClientExtension);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_match_the_release_workflow() {
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(
            asset_name(zed::Os::Mac, zed::Architecture::Aarch64),
            format!("zed-http-{version}-aarch64-apple-darwin.tar.gz")
        );
        assert_eq!(
            asset_name(zed::Os::Linux, zed::Architecture::X8664),
            format!("zed-http-{version}-x86_64-unknown-linux-gnu.tar.gz")
        );
        assert_eq!(
            asset_name(zed::Os::Windows, zed::Architecture::X86),
            format!("zed-http-{version}-i686-pc-windows-msvc.tar.gz")
        );
        assert_eq!(release_tag(), format!("http-client-extension-v{version}"));
    }

    /// The targets the release workflow builds, which the downloads look for.
    #[test]
    fn every_released_target_is_one_the_extension_asks_for() {
        let workflow = include_str!("../../.github/workflows/http-client-extension-release.yml");
        let platforms = [
            (zed::Os::Mac, zed::Architecture::Aarch64),
            (zed::Os::Mac, zed::Architecture::X8664),
            (zed::Os::Linux, zed::Architecture::X8664),
            (zed::Os::Linux, zed::Architecture::Aarch64),
        ];
        for (os, arch) in platforms {
            let target = format!("{}-{}", arch_name(arch), os_name(os));
            assert!(
                workflow.contains(&format!("target: {target}")),
                "the release workflow does not build {target}"
            );
        }
    }

    #[test]
    fn the_runner_is_laid_out_like_the_release_archive() {
        assert_eq!(
            INSTALL_DIR,
            format!("zed-http-{}", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(
            binary_path("zed-http-1.0.0", zed::Os::Mac),
            "zed-http-1.0.0/bin/zed-http"
        );
        assert_eq!(
            binary_path("zed-http-1.0.0", zed::Os::Linux),
            "zed-http-1.0.0/bin/zed-http"
        );
        assert_eq!(
            binary_path("zed-http-1.0.0", zed::Os::Windows),
            "zed-http-1.0.0/bin/zed-http.exe"
        );
    }

    #[test]
    fn only_other_versions_are_stale() {
        assert!(is_stale_install("zed-http-0.0.1"));
        assert!(!is_stale_install(INSTALL_DIR), "this version");
        assert!(!is_stale_install("zed-http"), "not a versioned directory");
        assert!(!is_stale_install("something-else"));
    }

    #[test]
    fn the_sources_live_next_to_the_work_directory() {
        assert_eq!(
            source_dir("/ext/work/http-client").as_deref(),
            Some("/ext/installed/http-client")
        );
        assert_eq!(
            source_dir("/ext/work/http-client/").as_deref(),
            Some("/ext/installed/http-client")
        );
        assert_eq!(source_dir("/ext/elsewhere/http-client"), None);
        assert_eq!(source_dir("http-client"), None);
    }

    #[test]
    fn the_installation_error_says_why_each_source_failed() {
        let failures = [Release.failure("404"), SourceBuild.failure("no cargo")];
        let error = installation_error(&failures);
        assert!(
            error.contains(
                "no prebuilt runner (404), and building one from source failed (no cargo)"
            ),
            "{error}"
        );
        assert!(
            error.ends_with(&format!("in a clone of {REPO}.")),
            "{error}"
        );
    }
}
