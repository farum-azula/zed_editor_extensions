//! The language server: lenses, code actions, commands, go-to-definition and hover.
//!
//! Every LSP method is a [`Handler`] in [`HANDLERS`]; supporting another method is one more
//! handler and one more entry there.
//!
//! | Module         | What it does                                                         |
//! |----------------|----------------------------------------------------------------------|
//! | `protocol`     | framed messages, the client written to, shared LSP types and URIs    |
//! | `lifecycle`    | `initialize`, `initialized`, `shutdown`, `exit`                      |
//! | `documents`    | the open documents and the changes to them                           |
//! | `features`     | code lenses and code actions, per kind of document                   |
//! | `generation`   | generating a source file's requests into `.zed/http/endpoints/`      |
//! | `environments` | the environment switcher of `.http` files                            |
//! | `commands`     | `workspace/executeCommand` and the commands it runs                  |
//! | `variables`    | definition and hover of `{{variables}}`                              |
//! | `catalogs`     | the built-in catalog, or the project's, reloaded when it changes     |

mod catalogs;
mod commands;
mod documents;
mod environments;
mod features;
mod generation;
mod lifecycle;
mod protocol;
mod variables;

pub use environments::env_edit;
pub use protocol::{path_to_uri, uri_to_path, Position};
pub use variables::{find_env_keys, find_file_variable, find_request_name, variable_at, EnvKey};

use crate::catalog::Catalog;
use catalogs::Catalogs;
use protocol::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// Serves the protocol on stdio (how Zed starts the server).
pub fn serve() {
    // Zed runs this binary from wherever the extension put it; tasks look elsewhere.
    crate::launcher::install();
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve_io(&mut stdin.lock(), &mut stdout.lock());
}

/// Reads requests from `input` and writes responses to `out` until `exit`, the end of the
/// input, or a write that fails (reported on stderr, which Zed shows in the server's log).
pub fn serve_io(input: &mut dyn BufRead, out: &mut dyn Write) {
    let mut session = Session::default();
    let mut client = Client::new(out);
    while let Some(message) = protocol::read_message(input) {
        // Responses to our own requests (workspace/applyEdit, window/showDocument) have no method.
        let Some(method) = message.get("method").and_then(Value::as_str) else { continue };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let outcome = HANDLERS
            .iter()
            .find(|handler| handler.method() == method)
            .map_or(Outcome::Nothing, |handler| handler.handle(params, &mut session, &mut client));
        match (outcome, message.get("id").cloned()) {
            (Outcome::Exit, _) => return,
            (Outcome::Result(result), Some(id)) => client.respond(id, result),
            (Outcome::Nothing, Some(id)) => client.respond_method_not_found(id, method),
            (_, None) => {}
        }
        if let Some(error) = client.take_failure() {
            eprintln!("zed-http: cannot write to the editor, stopping: {error}");
            return;
        }
    }
}

/// What handling a message comes to.
pub(crate) enum Outcome {
    /// The result of a request.
    Result(Value),
    /// Nothing to answer, as for a notification; a request answered so gets "method not found".
    Nothing,
    /// Stop serving.
    Exit,
}

/// The handler of one LSP method.
pub(crate) trait Handler {
    /// The method it handles, e.g. `textDocument/hover`.
    fn method(&self) -> &'static str;
    /// Handles a message whose parameters are `params` (null when it has none).
    fn handle(&self, params: Value, session: &mut Session, client: &mut Client) -> Outcome;
}

/// Every method the server handles.
const HANDLERS: [&dyn Handler; 13] = [
    &lifecycle::Initialize,
    &lifecycle::Initialized,
    &lifecycle::Shutdown,
    &lifecycle::Exit,
    &documents::DidOpen,
    &documents::DidChange,
    &documents::DidClose,
    &documents::DidChangeWatchedFiles,
    &features::CodeLens,
    &features::CodeAction,
    &commands::ExecuteCommand,
    &variables::Definition,
    &variables::Hover,
];

/// What the server keeps between messages.
#[derive(Default)]
pub(crate) struct Session {
    /// Open documents by URI, with their text in the editor.
    documents: HashMap<String, String>,
    /// The workspace folder.
    root: Option<PathBuf>,
    catalogs: Catalogs,
}

impl Session {
    /// The open documents, the workspace folder and the catalog that applies to it.
    fn workspace(&mut self) -> Workspace<'_> {
        Workspace {
            catalog: self.catalogs.get(self.root.as_deref()),
            documents: &self.documents,
            root: self.root.as_deref(),
        }
    }
}

/// A view of the session for lenses, code actions and commands.
pub(crate) struct Workspace<'a> {
    catalog: &'a Catalog,
    documents: &'a HashMap<String, String>,
    root: Option<&'a Path>,
}
