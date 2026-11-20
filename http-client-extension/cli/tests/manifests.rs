//! The files of the extension that must agree with each other: manifests, the catalog, the
//! tasks and the runner.

use std::collections::BTreeSet;
use zed_http::catalog::Catalog;
use zed_http::launcher;

const EXTENSION_TOML: &str = include_str!("../../extension.toml");
const EXTENSION_CARGO_TOML: &str = include_str!("../../Cargo.toml");
const RUNNER_CARGO_TOML: &str = include_str!("../Cargo.toml");
const TASKS: &str = include_str!("../../languages/http/tasks.json");
const RUNNABLES: &str = include_str!("../../languages/http/runnables.scm");

/// The first `version = "…"` of a manifest: the package's own.
fn version(manifest: &str) -> &str {
    let line = manifest.lines().find(|line| line.starts_with("version = ")).expect("a version");
    line.trim_start_matches("version = ").trim_matches('"')
}

/// The `languages = [...]` the language server is attached to, in extension.toml.
fn server_languages() -> BTreeSet<String> {
    let section = EXTENSION_TOML.split("[language_servers.zed-http]").nth(1).expect("the language server");
    let list = section.split("languages = [").nth(1).and_then(|rest| rest.split(']').next()).expect("its languages");
    list.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .flat_map(|line| line.split(','))
        .map(|name| name.trim().trim_matches('"').to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

fn tasks() -> Vec<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(TASKS).unwrap().as_array().unwrap().clone()
}

/// The extension downloads the runner of its own version, so the three must not drift apart.
#[test]
fn the_manifests_agree_on_the_version() {
    assert_eq!(version(EXTENSION_CARGO_TOML), version(EXTENSION_TOML), "Cargo.toml and extension.toml");
    assert_eq!(version(RUNNER_CARGO_TOML), version(EXTENSION_TOML), "cli/Cargo.toml and extension.toml");
}

/// The language server is attached to languages by their Zed names: every language of the
/// catalog must be listed in extension.toml, and every language listed there (but HTTP) must
/// be one the catalog has frameworks for.
#[test]
fn the_server_languages_are_those_of_the_catalog() {
    let catalog: BTreeSet<String> = Catalog::builtin().languages.iter().flat_map(|l| l.zed.iter().cloned()).collect();
    let mut listed = server_languages();
    assert!(listed.remove("HTTP"), "the server serves .http files too");
    let missing: Vec<&String> = catalog.difference(&listed).collect();
    let unknown: Vec<&String> = listed.difference(&catalog).collect();
    assert!(missing.is_empty(), "missing in extension.toml: {missing:?}");
    assert!(unknown.is_empty(), "listed in extension.toml, but not in frameworks.json: {unknown:?}");
}

/// Every tag set in `runnables.scm` is bound to a task in `tasks.json`, or its ▶ would do nothing.
#[test]
fn every_runnable_has_a_task() {
    let tasks = tasks();
    let tagged: BTreeSet<&str> = tasks
        .iter()
        .flat_map(|task| task["tags"].as_array().into_iter().flatten())
        .filter_map(|tag| tag.as_str())
        .collect();
    let tags: BTreeSet<&str> =
        RUNNABLES.split("(#set! tag ").skip(1).filter_map(|rest| rest.split(')').next()).collect();
    assert_eq!(tags, BTreeSet::from(["http-request", "http-request-named"]));
    assert_eq!(tags, tagged);
}

/// Tasks are shell commands: without `zed-http` in `PATH` they fall back to the copy the server
/// keeps of itself (see `launcher.rs`).
#[test]
fn every_task_falls_back_to_the_launcher_copy() {
    let fallback = format!("$HOME/{}/zed-http", launcher::DIR);
    for task in tasks() {
        let command = task["command"].as_str().unwrap_or_default();
        assert!(command.contains("command -v zed-http"), "{} does not look in PATH: {command}", task["label"]);
        assert!(command.contains(&fallback), "{} does not fall back to {fallback}: {command}", task["label"]);
    }
}
