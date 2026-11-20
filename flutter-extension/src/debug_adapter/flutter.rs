//! The `Flutter` debug adapter: Flutter's own (`flutter debug_adapter`) behind
//! `tool/flutter_proxy.dart`, which hot reloads the app when a `.dart` file under `lib/` changes,
//! serves DevTools for it, and picks (or starts) the device of a scenario's `target`.

use zed_extension_api::serde_json::{json, Value};
use zed_extension_api::{DebugAdapterBinary, StartDebuggingRequestArguments, Worktree};

use super::{set_if_some, DebugAdapter};
use crate::launch_config::LaunchConfig;
use crate::registry::Named;
use crate::sdk::{project_pins_sdk_with_fvm, DART, FLUTTER, FVM};
use crate::tool_files::ToolFiles;

pub const NAME: &str = "Flutter";
/// The proxy, one of the installed tool files.
const PROXY: &str = "flutter_proxy.dart";
const DEFAULT_PROGRAM: &str = "lib/main.dart";

pub struct FlutterAdapter {
    tools: ToolFiles,
}

impl FlutterAdapter {
    pub fn new(tools: ToolFiles) -> Self {
        Self { tools }
    }

    /// The `launch` / `attach` arguments Flutter's adapter reads, and those only the proxy reads
    /// (`hotReloadOnSave`, `devTools`, `target`).
    fn configuration(config: &LaunchConfig, cwd: &str) -> Value {
        let mut configuration = json!({
            "program": config.program.path.as_deref().unwrap_or(DEFAULT_PROGRAM),
            "cwd": cwd,
            "args": config.program.args,
            "toolArgs": config.flutter.tool_args_with_device(),
            "flutterMode": config.flutter.mode.as_deref().unwrap_or("debug"),
            "hotReloadOnSave": config.hot_reload_on_save.unwrap_or(true),
        });
        set_if_some(&mut configuration, "vmServiceUri", &config.vm_service_uri);
        set_if_some(&mut configuration, "target", &config.flutter.target);
        set_if_some(&mut configuration, "devTools", &config.dev_tools);
        configuration
    }
}

impl Named for FlutterAdapter {
    fn name(&self) -> &'static str {
        NAME
    }
}

impl DebugAdapter for FlutterAdapter {
    fn binary(
        &self,
        config: &LaunchConfig,
        worktree: &Worktree,
    ) -> Result<DebugAdapterBinary, String> {
        let proxy = self.tools.install()?.path(PROXY)?;
        // The proxy runs the Flutter command it is given, followed by `debug_adapter`.
        let use_fvm = config
            .use_fvm
            .unwrap_or_else(|| project_pins_sdk_with_fvm(worktree));
        let (command, mut arguments) = if use_fvm {
            let fvm = FVM.find(worktree)?;
            (
                fvm.clone(),
                vec![DART.name.into(), proxy, fvm, FLUTTER.name.into()],
            )
        } else {
            (DART.find(worktree)?, vec![proxy, FLUTTER.find(worktree)?])
        };
        arguments.push("debug_adapter".into());

        let cwd = config
            .program
            .cwd
            .clone()
            .unwrap_or_else(|| worktree.root_path());
        Ok(DebugAdapterBinary {
            command: Some(command),
            arguments,
            envs: config.program.env.clone(),
            cwd: Some(cwd.clone()),
            connection: None,
            request_args: StartDebuggingRequestArguments {
                configuration: Self::configuration(config, &cwd).to_string(),
                request: config.request,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configuration(config: Value) -> Value {
        FlutterAdapter::configuration(&LaunchConfig::from_value(&config).unwrap(), "/app")
    }

    #[test]
    fn fills_in_the_defaults() {
        assert_eq!(
            configuration(json!({})),
            json!({
                "program": "lib/main.dart",
                "cwd": "/app",
                "args": [],
                "toolArgs": [],
                "flutterMode": "debug",
                "hotReloadOnSave": true,
            })
        );
    }

    #[test]
    fn passes_on_what_the_scenario_sets() {
        assert_eq!(
            configuration(json!({
                "program": "lib/main_dev.dart",
                "args": ["--verbose"],
                "toolArgs": ["--flavor", "dev"],
                "deviceId": "emulator-5554",
                "flutterMode": "profile",
                "hotReloadOnSave": false,
                "devTools": false,
                "target": "android",
                "vmServiceUri": "ws://127.0.0.1:8181/ws",
            })),
            json!({
                "program": "lib/main_dev.dart",
                "cwd": "/app",
                "args": ["--verbose"],
                "toolArgs": ["--flavor", "dev", "-d", "emulator-5554"],
                "flutterMode": "profile",
                "hotReloadOnSave": false,
                "devTools": false,
                "target": "android",
                "vmServiceUri": "ws://127.0.0.1:8181/ws",
            })
        );
    }
}
