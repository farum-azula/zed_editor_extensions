//! Keeping the runner reachable from Zed tasks.
//!
//! The extension installs `zed-http` itself, into a directory of its own that is not in `PATH`,
//! so nothing has to be set up by hand. Tasks — the ▶ run button in a `.http` file — are shell
//! commands, and the shell only knows `PATH`, so the server keeps a copy of itself where
//! `languages/http/tasks.json` can always find it.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

/// Directory under the home directory that tasks fall back to. Keep in sync with `tasks.json`.
pub const DIR: &str = ".zed-http/bin";

/// Copies the running binary to `~/.zed-http/bin`, unless the copy there is already this one.
/// Failures are not worth reporting: the server runs either way, only tasks need the copy.
pub fn install() -> Option<PathBuf> {
    let home = env::var_os("HOME").or(env::var_os("USERPROFILE"))?;
    install_into(Path::new(&home), &env::current_exe().ok()?).ok()
}

/// The part of [`install`] that does not read the environment.
pub fn install_into(home: &Path, exe: &Path) -> io::Result<PathBuf> {
    let name = exe.file_name().unwrap_or(OsStr::new("zed-http"));
    let dir = home.join(DIR);
    let target = dir.join(name);
    if is_copy_of(&target, exe) {
        return Ok(target);
    }
    fs::create_dir_all(&dir)?;
    // A running binary cannot be overwritten in place, so write beside it and rename.
    let temp = dir.join(format!(".{}.new", name.to_string_lossy()));
    fs::copy(exe, &temp)?;
    make_executable(&temp)?;
    fs::rename(&temp, &target)?;
    Ok(target)
}

/// Is `target` the same file as `exe`, or a copy of it (same size, not older)?
fn is_copy_of(target: &Path, exe: &Path) -> bool {
    if target == exe {
        return true;
    }
    let (Ok(t), Ok(e)) = (fs::metadata(target), fs::metadata(exe)) else { return false };
    t.len() == e.len() && t.modified().ok() >= e.modified().ok()
}

#[cfg(unix)]
fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}
