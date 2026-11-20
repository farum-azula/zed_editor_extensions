//! The environment switcher of `.http` files: lenses that show and change the environment a file
//! (or one of its requests) runs in, written into the file as `# @env NAME`.

use super::commands::{Arguments, Command};
use super::documents::is_http_file;
use super::features::DocumentFeatures;
use super::protocol::{line_range, uri_to_path, utf16_len, Client, Position};
use super::Workspace;
use crate::parser;
use crate::vars::{requested_env, Environments};
use serde_json::{json, Value};
use std::path::Path;

pub(super) const ENV: &str = "zed-http.env";

/// Lenses and code actions of `.http` files: the environment switcher.
pub(super) struct HttpFileFeatures;
/// `zed-http.env [uri, name | null, line?]`: sets the environment of the file, or of the
/// request whose block holds `line`; `null` drops the request's own choice.
pub(super) struct ChooseEnvironment;

impl DocumentFeatures for HttpFileFeatures {
    fn applies_to(&self, uri: &str) -> bool {
        is_http_file(uri)
    }

    /// Above the file's first line: `env: dev`, the environment the file runs in, then the
    /// others. Above a request's own `# @env`: the same, plus `from file`. A click writes the
    /// choice into the file (see [`env_edit`]).
    fn lenses(&self, _workspace: &Workspace, uri: &str, text: &str) -> Vec<Value> {
        let environments = environments_of(uri);
        let mut lenses = Vec::new();
        let requested = requested_env(None, text);
        if let Some(current) = environments.select(requested.as_deref()).unwrap_or(requested) {
            lenses.extend(switcher(uri, &environments, 0, &current, None));
        }
        for directive in parser::parse(text).requests.iter().filter_map(|request| request.env.as_ref()) {
            lenses.extend(switcher(uri, &environments, directive.line, &directive.name, Some(directive.line)));
        }
        lenses
    }

    /// `cmd+.` on a request without an environment of its own: give it one (then switch with
    /// its lens).
    fn actions(&self, workspace: &Workspace, uri: &str, line: usize) -> Vec<Value> {
        let Some(text) = workspace.documents.get(uri) else { return vec![] };
        let file = parser::parse(text);
        if !parser::request_at(&file, line + 1).is_some_and(|request| request.own_block && request.env.is_none()) {
            return vec![];
        }
        let Some(current) = environments_of(uri).select(requested_env(None, text).as_deref()).ok().flatten() else {
            return vec![];
        };
        vec![json!({
            "title": "HTTP: choose an environment for this request",
            "kind": "refactor",
            "command": { "title": "Environment", "command": ENV, "arguments": [uri, current, line] }
        })]
    }
}

/// One row of lenses on `line`: the `current` environment, then the others to switch to. A
/// request's row (`request` is a line of its block) can also go back to the file's choice.
fn switcher(uri: &str, environments: &Environments, line: usize, current: &str, request: Option<usize>) -> Vec<Value> {
    let label = if environments.names.iter().any(|name| name == current) {
        format!("env: {current}")
    } else {
        format!("env: {current} (not defined)")
    };
    let lens = |title: String, name: Value| {
        let mut arguments = vec![json!(uri), name];
        arguments.extend(request.map(|line| json!(line)));
        json!({ "range": line_range(line), "command": { "title": title, "command": ENV, "arguments": arguments } })
    };
    let mut row = vec![lens(label, json!(current))];
    row.extend(environments.names.iter().filter(|name| *name != current).map(|name| lens(name.clone(), json!(name))));
    if request.is_some() {
        row.push(lens("from file".into(), Value::Null));
    }
    row
}

/// The environment files next to the `.http` file at `uri`; none if they are missing or invalid.
fn environments_of(uri: &str) -> Environments {
    let directory = uri_to_path(uri).and_then(|path| path.parent().map(Path::to_path_buf)).unwrap_or_default();
    Environments::load(&directory).unwrap_or_default()
}

impl Command for ChooseEnvironment {
    fn name(&self) -> &'static str {
        ENV
    }

    fn execute(&self, arguments: &Arguments, workspace: &Workspace, client: &mut Client) {
        let Some(uri) = arguments.text(0) else { return };
        let Some(text) = workspace.documents.get(uri) else { return };
        if let Some(edit) = env_edit(uri, text, arguments.text(1), arguments.line(2)) {
            client.apply_edit("Environment", edit);
        }
    }
}

/// The edit that sets the environment of the file, or of the request whose block holds `request`
/// (a 0-based line), to `name`; `name: None` drops the request's own choice. `None` if there is
/// nothing to change.
pub fn env_edit(uri: &str, text: &str, name: Option<&str>, request: Option<usize>) -> Option<Value> {
    let file = parser::parse(text);
    let lines: Vec<&str> = text.lines().collect();
    let replace = |line: usize, name: &str| {
        let end = Position { line, character: utf16_len(lines[line]) };
        json!({ "range": { "start": Position { line, character: 0 }, "end": end }, "newText": format!("# @env {name}") })
    };
    let insert = |line: usize, name: &str| json!({ "range": line_range(line), "newText": format!("# @env {name}\n") });
    // A request in the file's header has no choice of its own: the file's is its.
    let request = request.and_then(|line| parser::request_at(&file, line + 1)).filter(|request| request.own_block);
    let edit = match (request, name) {
        (Some(request), Some(name)) => match &request.env {
            Some(directive) => replace(directive.line, name),
            // Right below its `###` line.
            None => insert(request.start_line, name),
        },
        (Some(request), None) => {
            let line = request.env.as_ref()?.line;
            let range =
                json!({ "start": Position { line, character: 0 }, "end": Position { line: line + 1, character: 0 } });
            json!({ "range": range, "newText": "" })
        }
        (None, Some(name)) => match parser::file_env(text) {
            Some(directive) => replace(directive.line, name),
            None => insert(0, name),
        },
        (None, None) => return None,
    };
    Some(json!({ "changes": { uri: [edit] } }))
}

/// The environment variables resolve in at `line` (0-based): the request's own, then the file's.
pub(super) fn env_at(text: &str, line: usize) -> Option<String> {
    let file = parser::parse(text);
    let own = parser::request_at(&file, line + 1).and_then(|request| request.env.as_ref().map(|env| env.name.clone()));
    requested_env(own, text)
}
