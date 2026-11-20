//! Starting and stopping: `initialize`, `initialized`, `shutdown`, `exit`.

use super::commands::COMMANDS;
use super::protocol::{decode, uri_to_path, Client};
use super::{Handler, Outcome, Session};
use serde::Deserialize;
use serde_json::{json, Value};

pub(super) struct Initialize;
pub(super) struct Initialized;
pub(super) struct Shutdown;
pub(super) struct Exit;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    #[serde(default)]
    workspace_folders: Option<Vec<WorkspaceFolder>>,
    #[serde(default)]
    root_uri: Option<String>,
}

#[derive(Deserialize)]
struct WorkspaceFolder {
    #[serde(default)]
    uri: Option<String>,
}

impl Handler for Initialize {
    fn method(&self) -> &'static str {
        "initialize"
    }

    fn handle(&self, params: Value, session: &mut Session, _client: &mut Client) -> Outcome {
        let params: InitializeParams = decode(params).unwrap_or_default();
        let first_folder = params.workspace_folders.iter().flatten().next().and_then(|folder| folder.uri.as_deref());
        session.root = first_folder.or(params.root_uri.as_deref()).and_then(uri_to_path);
        let commands: Vec<&str> = COMMANDS.iter().map(|command| command.name()).collect();
        Outcome::Result(json!({
            "capabilities": {
                "textDocumentSync": 1,
                "definitionProvider": true,
                "hoverProvider": true,
                "codeActionProvider": true,
                "codeLensProvider": { "resolveProvider": false },
                "executeCommandProvider": { "commands": commands }
            },
            "serverInfo": { "name": "zed-http" }
        }))
    }
}

impl Handler for Initialized {
    fn method(&self) -> &'static str {
        "initialized"
    }

    /// Requests generated, edited or deleted outside the open buffers change the "Open" /
    /// "Generate" lenses of source files too: Zed is asked to report changes to `.http` files.
    fn handle(&self, _params: Value, _session: &mut Session, client: &mut Client) -> Outcome {
        let registration = json!({
            "id": "zed-http-files",
            "method": "workspace/didChangeWatchedFiles",
            "registerOptions": { "watchers": [{ "globPattern": "**/*.http" }] }
        });
        client.request("client/registerCapability", Some(json!({ "registrations": [registration] })));
        Outcome::Nothing
    }
}

impl Handler for Shutdown {
    fn method(&self) -> &'static str {
        "shutdown"
    }

    fn handle(&self, _params: Value, _session: &mut Session, _client: &mut Client) -> Outcome {
        Outcome::Result(Value::Null)
    }
}

impl Handler for Exit {
    fn method(&self) -> &'static str {
        "exit"
    }

    fn handle(&self, _params: Value, _session: &mut Session, _client: &mut Client) -> Outcome {
        Outcome::Exit
    }
}
