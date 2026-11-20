//! `workspace/executeCommand` and the commands lenses and code actions run, each a [`Command`]
//! in [`COMMANDS`] (also what `initialize` advertises).

use super::environments::ChooseEnvironment;
use super::generation::{GenerateRequest, OpenRequest};
use super::protocol::{decode, Client};
use super::{Handler, Outcome, Session, Workspace};
use serde::Deserialize;
use serde_json::Value;

pub(super) struct ExecuteCommand;

/// A command of the server.
pub(super) trait Command {
    /// The name lenses and code actions refer to it by, e.g. `zed-http.generate`.
    fn name(&self) -> &'static str;
    fn execute(&self, arguments: &Arguments, workspace: &Workspace, client: &mut Client);
}

pub(super) const COMMANDS: [&dyn Command; 3] = [&GenerateRequest, &OpenRequest, &ChooseEnvironment];

/// The positional arguments of a command, as the lens or code action gave them.
pub(super) struct Arguments(Vec<Value>);

impl Arguments {
    /// The string at `index`, if there is one there.
    pub fn text(&self, index: usize) -> Option<&str> {
        self.0.get(index).and_then(Value::as_str)
    }

    /// The line number at `index`, if there is one there.
    pub fn line(&self, index: usize) -> Option<usize> {
        self.0.get(index).and_then(Value::as_u64).map(|line| line as usize)
    }
}

#[derive(Deserialize, Default)]
struct ExecuteCommandParams {
    #[serde(default)]
    command: String,
    #[serde(default)]
    arguments: Vec<Value>,
}

impl Handler for ExecuteCommand {
    fn method(&self) -> &'static str {
        "workspace/executeCommand"
    }

    /// Runs the command; an unknown one does nothing. Lenses are asked for again either way: a
    /// command may have changed a `.http` file.
    fn handle(&self, params: Value, session: &mut Session, client: &mut Client) -> Outcome {
        let params: ExecuteCommandParams = decode(params).unwrap_or_default();
        let workspace = session.workspace();
        if let Some(command) = COMMANDS.iter().find(|command| command.name() == params.command) {
            command.execute(&Arguments(params.arguments), &workspace, client);
        }
        client.refresh_lenses();
        Outcome::Result(Value::Null)
    }
}
