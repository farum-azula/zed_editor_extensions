//! The programs of `tool/`, plain Dart that the SDK runs: compiled into the extension and written
//! into its work directory before one of them starts, so they always match this version of the
//! extension. Which files there are is the `TOOLS` list of `lib.rs`, and nothing else.

use std::fs;
use std::path::PathBuf;

/// The files to install: (file name, contents).
#[derive(Clone, Copy)]
pub struct ToolFiles {
    files: &'static [(&'static str, &'static str)],
}

/// The work directory once the files are in it.
pub struct InstalledTools {
    directory: PathBuf,
    files: &'static [(&'static str, &'static str)],
}

impl ToolFiles {
    pub const fn new(files: &'static [(&'static str, &'static str)]) -> Self {
        Self { files }
    }

    /// Writes every file that is missing from the work directory or differs from this version's.
    pub fn install(&self) -> Result<InstalledTools, String> {
        for (file, text) in self.files {
            if fs::read_to_string(file).ok().as_deref() != Some(*text) {
                fs::write(file, text).map_err(|e| format!("could not write {file}: {e}"))?;
            }
        }
        Ok(InstalledTools {
            directory: work_directory()?,
            files: self.files,
        })
    }
}

impl InstalledTools {
    /// The host path of the installed file [name].
    pub fn path(&self, name: &str) -> Result<String, String> {
        resolve(&self.directory, self.files, name)
    }
}

fn resolve(
    directory: &std::path::Path,
    files: &[(&str, &str)],
    name: &str,
) -> Result<String, String> {
    if !files.iter().any(|(file, _)| *file == name) {
        return Err(format!(
            "{name} is not one of the files Flutter Tools installs"
        ));
    }
    Ok(directory.join(name).to_string_lossy().into_owned())
}

/// The extension's work directory, as a path for the host.
fn work_directory() -> Result<PathBuf, String> {
    let directory = std::env::current_dir().map_err(|e| format!("no work directory: {e}"))?;
    Ok(host_path(&directory.to_string_lossy()).into())
}

/// The WASI sandbox reports a Windows path as `/C:/...`; the host wants `C:/...`.
fn host_path(sandbox_path: &str) -> &str {
    match sandbox_path.strip_prefix('/') {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => rest,
        _ => sandbox_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const FILES: &[(&str, &str)] = &[("a.dart", "void main() {}")];

    #[test]
    fn a_windows_path_loses_the_slash_of_the_sandbox() {
        assert_eq!(host_path("/C:/Users/me/work"), "C:/Users/me/work");
    }

    #[test]
    fn a_unix_path_stays() {
        assert_eq!(host_path("/home/me/work"), "/home/me/work");
        assert_eq!(host_path("/"), "/");
    }

    #[test]
    fn an_installed_file_is_in_the_work_directory() {
        assert_eq!(
            resolve(Path::new("/work"), FILES, "a.dart"),
            Ok("/work/a.dart".to_string())
        );
    }

    #[test]
    fn a_file_that_is_not_installed_is_an_error() {
        assert_eq!(
            resolve(Path::new("/work"), FILES, "b.dart"),
            Err("b.dart is not one of the files Flutter Tools installs".to_string())
        );
    }
}
