//! The debug adapters (`[debug_adapters.<name>]` of `extension.toml`), one type each.

mod dart;
mod flutter;

pub use dart::{DartAdapter, NAME as DART_ADAPTER};
pub use flutter::{FlutterAdapter, NAME as FLUTTER_ADAPTER};

use zed_extension_api::serde_json::{json, Map, Value};
use zed_extension_api::{DebugAdapterBinary, LaunchRequest, Worktree};

use crate::launch_config::LaunchConfig;
use crate::registry::Named;

pub trait DebugAdapter: Named {
    /// How Zed starts the adapter for a scenario, and what it asks the adapter to do.
    fn binary(
        &self,
        config: &LaunchConfig,
        worktree: &Worktree,
    ) -> Result<DebugAdapterBinary, String>;

    /// The `config` of a scenario started from the debugger's "new session" window.
    fn launch_scenario(&self, launch: &LaunchRequest, _stop_on_entry: Option<bool>) -> Value {
        launch_request_config(launch)
    }
}

/// The program, arguments, directory and environment of a "new session" launch, as a scenario's
/// `config`.
fn launch_request_config(launch: &LaunchRequest) -> Value {
    let mut config = json!({
        "request": "launch",
        "program": launch.program,
        "args": launch.args,
    });
    set_if_some(&mut config, "cwd", &launch.cwd);
    if !launch.envs.is_empty() {
        let env: Map<String, Value> = launch
            .envs
            .iter()
            .map(|(name, value)| (name.clone(), json!(value)))
            .collect();
        config["env"] = Value::Object(env);
    }
    config
}

/// Sets [key] of [config] to [value] when there is one; a field left out keeps the adapter's
/// default.
fn set_if_some<T: Into<Value> + Clone>(config: &mut Value, key: &str, value: &Option<T>) {
    if let Some(value) = value {
        config[key] = value.clone().into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(cwd: Option<&str>, envs: &[(&str, &str)]) -> LaunchRequest {
        LaunchRequest {
            program: "bin/server.dart".into(),
            cwd: cwd.map(str::to_string),
            args: vec!["--port".into(), "8080".into()],
            envs: envs
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
        }
    }

    #[test]
    fn a_new_session_launches_the_program() {
        assert_eq!(
            launch_request_config(&launch(None, &[])),
            json!({"request": "launch", "program": "bin/server.dart", "args": ["--port", "8080"]})
        );
    }

    #[test]
    fn with_its_directory_and_environment_when_given() {
        assert_eq!(
            launch_request_config(&launch(Some("/app"), &[("MODE", "dev")])),
            json!({
                "request": "launch",
                "program": "bin/server.dart",
                "args": ["--port", "8080"],
                "cwd": "/app",
                "env": {"MODE": "dev"},
            })
        );
    }
}
