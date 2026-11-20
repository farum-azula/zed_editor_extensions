//! Compiles the grammar's parser. Its sources are the checkout Zed makes in `grammars/http` when it
//! builds the extension, if that is at the pinned commit; otherwise (CI, a fresh clone) the pinned
//! commit is fetched with git into the build directory.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The grammar's repository and commit, from `[grammars.http]` in `extension.toml`.
struct Pin {
    repository: String,
    commit: String,
}

fn main() {
    let extension = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let manifest = extension.join("extension.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = fs::read_to_string(&manifest).unwrap_or_else(|error| panic!("{}: {error}", manifest.display()));
    let pin = pin(&text)
        .unwrap_or_else(|| panic!("{}: no [grammars.http] with a repository and a commit", manifest.display()));
    let checkout = extension.join("grammars/http");
    let sources = if head(&checkout).as_deref() == Some(pin.commit.as_str()) { checkout } else { fetch(&pin) };
    let src = sources.join("src");
    println!("cargo:rerun-if-changed={}", src.join("parser.c").display());
    let mut build = cc::Build::new();
    build.include(&src).file(src.join("parser.c")).warnings(false).flag_if_supported("-w");
    if src.join("scanner.c").exists() {
        build.file(src.join("scanner.c"));
    }
    build.compile("tree-sitter-http");
}

/// `repository` and `commit` of the `[grammars.http]` table.
fn pin(manifest: &str) -> Option<Pin> {
    let table = manifest.split("[grammars.http]").nth(1)?.split("\n[").next()?;
    let value = |key: &str| {
        table.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == key).then(|| value.trim().trim_matches('"').to_string())
        })
    };
    Some(Pin { repository: value("repository")?, commit: value("commit")? })
}

/// The commit a git checkout is at.
fn head(checkout: &Path) -> Option<String> {
    let output = Command::new("git").arg("-C").arg(checkout).args(["rev-parse", "HEAD"]).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// The sources at the pinned commit, fetched into the build directory once.
fn fetch(pin: &Pin) -> PathBuf {
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let checkout = out.join(format!("tree-sitter-http-{}", pin.commit));
    if checkout.join("src/parser.c").exists() {
        return checkout;
    }
    let git = |args: &[&str]| {
        let status = Command::new("git").arg("-C").arg(&checkout).args(args).status();
        if !status.is_ok_and(|status| status.success()) {
            panic!(
                "git {} failed: the grammar at {} {} is needed to test the queries",
                args.join(" "),
                pin.repository,
                pin.commit
            );
        }
    };
    fs::create_dir_all(&checkout).unwrap_or_else(|error| panic!("{}: {error}", checkout.display()));
    git(&["init", "--quiet"]);
    git(&["fetch", "--quiet", "--depth", "1", &pin.repository, &pin.commit]);
    git(&["checkout", "--quiet", "FETCH_HEAD"]);
    checkout
}
