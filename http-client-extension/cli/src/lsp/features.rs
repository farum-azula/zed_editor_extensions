//! Code lenses and code actions. What a document gets depends on its kind, each a
//! [`DocumentFeatures`] in [`FEATURES`]: the environment switcher in `.http` files, request
//! generation in source files.

use super::environments::HttpFileFeatures;
use super::generation::SourceFileFeatures;
use super::protocol::{decode, Client, Position, TextDocumentIdentifier};
use super::{Handler, Outcome, Session, Workspace};
use serde::Deserialize;
use serde_json::{json, Value};

pub(super) struct CodeLens;
pub(super) struct CodeAction;

/// Lenses and code actions for one kind of document.
pub(super) trait DocumentFeatures {
    /// Whether the document at `uri` is of this kind.
    fn applies_to(&self, uri: &str) -> bool;
    /// The lenses of the open document `uri`, whose text is `text`.
    fn lenses(&self, workspace: &Workspace, uri: &str, text: &str) -> Vec<Value>;
    /// The code actions on `line` of the document `uri`.
    fn actions(&self, workspace: &Workspace, uri: &str, line: usize) -> Vec<Value>;
}

/// Asked in order; the first kind a document is of decides.
const FEATURES: [&dyn DocumentFeatures; 2] = [&HttpFileFeatures, &SourceFileFeatures];

fn features_for(uri: &str) -> Option<&'static dyn DocumentFeatures> {
    FEATURES.into_iter().find(|features| features.applies_to(uri))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodeLensParams {
    #[serde(default)]
    text_document: TextDocumentIdentifier,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodeActionParams {
    #[serde(default)]
    text_document: TextDocumentIdentifier,
    range: Range,
}

#[derive(Deserialize)]
struct Range {
    start: Position,
}

impl Handler for CodeLens {
    fn method(&self) -> &'static str {
        "textDocument/codeLens"
    }

    fn handle(&self, params: Value, session: &mut Session, _client: &mut Client) -> Outcome {
        let workspace = session.workspace();
        let uri = decode::<CodeLensParams>(params).map(|params| params.text_document.uri).unwrap_or_default();
        let lenses = match (workspace.documents.get(&uri), features_for(&uri)) {
            (Some(text), Some(features)) => features.lenses(&workspace, &uri, text),
            _ => vec![],
        };
        Outcome::Result(json!(lenses))
    }
}

impl Handler for CodeAction {
    fn method(&self) -> &'static str {
        "textDocument/codeAction"
    }

    fn handle(&self, params: Value, session: &mut Session, _client: &mut Client) -> Outcome {
        let workspace = session.workspace();
        let actions = match decode::<CodeActionParams>(params) {
            Some(params) => {
                let uri = params.text_document.uri;
                features_for(&uri).map(|features| features.actions(&workspace, &uri, params.range.start.line))
            }
            None => None,
        };
        Outcome::Result(json!(actions.unwrap_or_default()))
    }
}
