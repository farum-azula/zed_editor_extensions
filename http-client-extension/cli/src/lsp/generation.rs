//! Requests generated from the routes and HTTP calls of source files. All requests from one
//! source file go to `.zed/http/endpoints/<source file>.http`; an already generated request is
//! opened instead of being generated again.

use super::commands::{Arguments, Command};
use super::documents::is_http_file;
use super::features::DocumentFeatures;
use super::protocol::{file_name, line_range, path_to_uri, uri_to_path, utf16_len, Client, Position};
use super::Workspace;
use crate::endpoint;
use crate::vars::ZED_HTTP_DIR;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(super) const GENERATE: &str = "zed-http.generate";
pub(super) const OPEN: &str = "zed-http.open";

/// Lenses and code actions of source files: generate a route's request, or open it.
pub(super) struct SourceFileFeatures;
/// `zed-http.generate [uri, line]`: generates the request of the route on `line`, then shows it.
pub(super) struct GenerateRequest;
/// `zed-http.open [uri, line]`: shows a generated request.
pub(super) struct OpenRequest;

/// What generating the request for a line would do.
struct Generation {
    /// `METHOD URL` of the request.
    title: String,
    name: String,
    target: PathBuf,
    /// Line of the existing request in `target`, if it was generated before.
    existing: Option<usize>,
    /// `target` before the request is added; `None` if it does not exist yet.
    current: Option<String>,
    /// The text that adds the request at the end of `current`.
    added: String,
    /// `.zed/http/` of the project: its environment file is created (or pointed at the
    /// application's server) when the request is added.
    base: PathBuf,
    /// The source file the request comes from (its application's configuration gives the port).
    source: PathBuf,
}

fn generation(workspace: &Workspace, uri: &str, line: usize) -> Option<Generation> {
    if is_http_file(uri) {
        return None;
    }
    let text = workspace.documents.get(uri)?;
    let file = uri_to_path(uri)?;
    let request = endpoint::generate(workspace.catalog, text, &file, line)?;
    let root = workspace.root.map(Path::to_path_buf).or_else(|| file.parent().map(Path::to_path_buf))?;
    let base = root.join(ZED_HTTP_DIR);
    let target = endpoint::file_for_source(&base.join("endpoints"), &file);
    // A file deleted from disk is gone, even if its tab is still open in Zed.
    let current = target
        .exists()
        .then(|| workspace.documents.get(&path_to_uri(&target)).cloned().or_else(|| fs::read_to_string(&target).ok()))
        .flatten();
    let existing = current.as_deref().and_then(|text| endpoint::find_request(text, &request.name));
    let added = endpoint::append_text(current.as_deref().unwrap_or_default(), &request.content);
    Some(Generation {
        title: format!("{} {}", request.endpoint.method, request.endpoint.url),
        name: request.name,
        target,
        existing,
        current,
        added,
        base,
        source: file,
    })
}

impl DocumentFeatures for SourceFileFeatures {
    fn applies_to(&self, uri: &str) -> bool {
        !is_http_file(uri)
    }

    /// A clickable lens above every route / client call: generates the request, or opens it if
    /// it exists.
    fn lenses(&self, workspace: &Workspace, uri: &str, text: &str) -> Vec<Value> {
        let Some(file) = uri_to_path(uri) else { return vec![] };
        let mut lenses = Vec::new();
        for route in workspace.catalog.routes(text, &file).into_iter().filter(|route| !route.endpoint.weak) {
            let line = route.line;
            let Some(generation) = generation(workspace, uri, line) else { continue };
            let command = match generation.existing {
                Some(at) => json!({
                    "title": format!("🔗 Open {}", file_name(&generation.target)),
                    "command": OPEN,
                    "arguments": [path_to_uri(&generation.target), at]
                }),
                None => json!({ "title": "🌐 Generate .http request", "command": GENERATE, "arguments": [uri, line] }),
            };
            lenses.push(json!({ "range": line_range(line), "command": command }));
        }
        lenses
    }

    /// "Generate .http request" / "Open request" for an endpoint on the cursor line.
    fn actions(&self, workspace: &Workspace, uri: &str, line: usize) -> Vec<Value> {
        let Some(generation) = generation(workspace, uri, line) else { return vec![] };
        let action = match generation.existing {
            Some(at) => json!({
                "title": format!("HTTP: open {} in {}", generation.name, file_name(&generation.target)),
                "kind": "refactor",
                "command": { "title": "Open", "command": OPEN, "arguments": [path_to_uri(&generation.target), at] }
            }),
            None => json!({
                "title": format!("HTTP: generate request {} → {}", generation.title, file_name(&generation.target)),
                "kind": "refactor",
                "command": { "title": "Generate", "command": GENERATE, "arguments": [uri, line] }
            }),
        };
        vec![action]
    }
}

impl Command for GenerateRequest {
    fn name(&self) -> &'static str {
        GENERATE
    }

    fn execute(&self, arguments: &Arguments, workspace: &Workspace, client: &mut Client) {
        let (Some(uri), Some(line)) = (arguments.text(0), arguments.line(1)) else { return };
        let Some(generation) = generation(workspace, uri, line) else { return };
        let at = match generation.existing {
            Some(at) => at,
            None => {
                let at = appended_line(&generation.target, workspace.documents);
                if let Err(error) = add_request(&generation, workspace.documents, client) {
                    // Shown in Zed's language server log.
                    eprintln!("zed-http: {}: {error}", generation.target.display());
                    return;
                }
                at
            }
        };
        client.show_document(&generation.target, at);
    }
}

impl Command for OpenRequest {
    fn name(&self) -> &'static str {
        OPEN
    }

    fn execute(&self, arguments: &Arguments, _workspace: &Workspace, client: &mut Client) {
        if let Some(path) = arguments.text(0).and_then(uri_to_path) {
            client.show_document(&path, arguments.line(1).unwrap_or(0));
        }
    }
}

/// Adds the generated request. A `.http` file open in Zed is edited in its buffer, which Zed then
/// saves like any other edit; otherwise the file is written to disk, where `zed-http run` reads it.
/// (A workspace edit to a file that is not open would leave it unsaved, and empty on disk.) A file
/// missing on disk counts as gone, open or not: it is written afresh.
fn add_request(generation: &Generation, documents: &HashMap<String, String>, client: &mut Client) -> io::Result<()> {
    endpoint::ensure_env(&generation.base, &generation.source)?;
    let uri = path_to_uri(&generation.target);
    match generation.current.as_deref().filter(|_| documents.contains_key(&uri)) {
        Some(open) => {
            let at = end_position(open);
            let change = json!({ "range": { "start": at, "end": at }, "newText": generation.added });
            client.apply_edit(&format!("Generate {}", generation.title), json!({ "changes": { uri: [change] } }));
        }
        None => {
            let whole = format!("{}{}", generation.current.as_deref().unwrap_or_default(), generation.added);
            write_target(&generation.target, &whole)?;
        }
    }
    Ok(())
}

fn write_target(target: &Path, text: &str) -> io::Result<()> {
    // A target always lies in `.zed/http/endpoints`; creating "" does nothing.
    fs::create_dir_all(target.parent().unwrap_or(Path::new("")))?;
    fs::write(target, text)
}

/// LSP position just past the end of `text`.
fn end_position(text: &str) -> Position {
    let line = text.matches('\n').count();
    let last = text.rsplit('\n').next().unwrap_or_default();
    Position { line, character: utf16_len(last) }
}

/// Line of the newly appended request (its `###` header) after applying the edit.
fn appended_line(target: &Path, documents: &HashMap<String, String>) -> usize {
    let current = documents.get(&path_to_uri(target)).cloned().or_else(|| fs::read_to_string(target).ok());
    let current = current.unwrap_or_default();
    (current.clone() + &endpoint::append_text(&current, "")).matches('\n').count()
}
