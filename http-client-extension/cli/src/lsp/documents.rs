//! The open documents (full text sync) and the changes that call for the lenses again.

use super::protocol::{decode, Client, TextDocumentIdentifier};
use super::{Handler, Outcome, Session};
use serde::Deserialize;
use serde_json::Value;

pub(super) struct DidOpen;
pub(super) struct DidChange;
pub(super) struct DidClose;
pub(super) struct DidChangeWatchedFiles;

/// Is the document at `uri` a `.http` file (or `.rest`)?
pub(super) fn is_http_file(uri: &str) -> bool {
    uri.ends_with(".http") || uri.ends_with(".rest")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidOpenParams {
    text_document: TextDocumentItem,
}

#[derive(Deserialize)]
struct TextDocumentItem {
    uri: String,
    text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidChangeParams {
    #[serde(default)]
    text_document: TextDocumentIdentifier,
    #[serde(default)]
    content_changes: Vec<ContentChange>,
}

/// A change to a document; only full-text changes are understood.
#[derive(Deserialize)]
struct ContentChange {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidCloseParams {
    text_document: TextDocumentIdentifier,
}

/// Keeps the text of `uri`; a `.http` file changes the lenses of the sources it has requests of.
fn store(session: &mut Session, client: &mut Client, uri: String, text: String) {
    let http_file = is_http_file(&uri);
    session.documents.insert(uri, text);
    if http_file {
        client.refresh_lenses();
    }
}

impl Handler for DidOpen {
    fn method(&self) -> &'static str {
        "textDocument/didOpen"
    }

    fn handle(&self, params: Value, session: &mut Session, client: &mut Client) -> Outcome {
        if let Some(params) = decode::<DidOpenParams>(params) {
            store(session, client, params.text_document.uri, params.text_document.text);
        }
        Outcome::Nothing
    }
}

impl Handler for DidChange {
    fn method(&self) -> &'static str {
        "textDocument/didChange"
    }

    /// A change without full text (incremental sync, which is not offered) keeps the old text.
    fn handle(&self, params: Value, session: &mut Session, client: &mut Client) -> Outcome {
        let Some(params) = decode::<DidChangeParams>(params) else { return Outcome::Nothing };
        if let Some(text) = params.content_changes.into_iter().last().and_then(|change| change.text) {
            store(session, client, params.text_document.uri, text);
        }
        Outcome::Nothing
    }
}

impl Handler for DidClose {
    fn method(&self) -> &'static str {
        "textDocument/didClose"
    }

    fn handle(&self, params: Value, session: &mut Session, client: &mut Client) -> Outcome {
        let Some(params) = decode::<DidCloseParams>(params) else { return Outcome::Nothing };
        let uri = params.text_document.uri;
        session.documents.remove(&uri);
        if is_http_file(&uri) {
            client.refresh_lenses();
        }
        Outcome::Nothing
    }
}

impl Handler for DidChangeWatchedFiles {
    fn method(&self) -> &'static str {
        "workspace/didChangeWatchedFiles"
    }

    /// A `.http` file created, changed or deleted outside the editor.
    fn handle(&self, _params: Value, _session: &mut Session, client: &mut Client) -> Outcome {
        client.refresh_lenses();
        Outcome::Nothing
    }
}
