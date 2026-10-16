//! `launcher`: the copy of the runner that Zed tasks fall back to.

mod support;

use std::fs;
use support::Dir;
use zed_http::launcher;

/// A file standing in for the running binary.
fn exe(dir: &Dir, contents: &str) -> std::path::PathBuf {
    dir.write("bin/zed-http", contents)
}

#[test]
fn installs_the_running_binary_under_the_home_directory() {
    let dir = Dir::new();
    let home = dir.join("home");
    let installed = launcher::install_into(&home, &exe(&dir, "#!/bin/sh\necho one\n")).unwrap();

    assert_eq!(installed, home.join(launcher::DIR).join("zed-http"));
    assert_eq!(fs::read_to_string(&installed).unwrap(), "#!/bin/sh\necho one\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&installed).unwrap().permissions().mode() & 0o777, 0o755);
    }
    // Nothing is left behind by the copy-then-rename.
    let entries: Vec<_> = fs::read_dir(home.join(launcher::DIR)).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
}

#[test]
fn keeps_an_up_to_date_copy_and_replaces_an_outdated_one() {
    let dir = Dir::new();
    let home = dir.join("home");
    let first = exe(&dir, "one");
    launcher::install_into(&home, &first).unwrap();
    let installed = home.join(launcher::DIR).join("zed-http");
    let stamp = fs::metadata(&installed).unwrap().modified().unwrap();

    // Same size and not older: left alone.
    launcher::install_into(&home, &first).unwrap();
    assert_eq!(fs::metadata(&installed).unwrap().modified().unwrap(), stamp);

    // A different build: copied over.
    let second = exe(&dir, "another one");
    launcher::install_into(&home, &second).unwrap();
    assert_eq!(fs::read_to_string(&installed).unwrap(), "another one");
}

#[test]
fn does_not_copy_a_binary_that_is_already_the_installed_one() {
    let dir = Dir::new();
    let home = dir.join("home");
    let installed = home.join(launcher::DIR).join("zed-http");
    fs::create_dir_all(installed.parent().unwrap()).unwrap();
    fs::write(&installed, "running from the home directory").unwrap();

    assert_eq!(launcher::install_into(&home, &installed).unwrap(), installed);
    assert_eq!(fs::read_to_string(&installed).unwrap(), "running from the home directory");
}

#[test]
fn reports_a_binary_that_cannot_be_read() {
    let dir = Dir::new();
    assert!(launcher::install_into(&dir.join("home"), &dir.join("gone")).is_err());
}
