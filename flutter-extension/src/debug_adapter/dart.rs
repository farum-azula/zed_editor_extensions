//! The `Dart` debug adapter, as Zed's Dart extension (https://github.com/zed-extensions/dart,
//! Apache-2.0) defines it: the SDK's own `dart debug_adapter`, or `flutter debug_adapter` for a
//! scenario of `"type": "flutter"`.

use zed_extension_api::serde_json::{json, Value};
use zed_extension_api::{
    DebugAdapterBinary, LaunchRequest, StartDebuggingRequestArguments, Worktree,
};

use super::{launch_request_config, set_if_some, DebugAdapter};
use crate::launch_config::{request_name, LaunchConfig};
use crate::registry::Named;
use crate::sdk::{project_pins_sdk_with_fvm, Executable, DART, FLUTTER, FVM};

pub const NAME: &str = "Dart";

pub struct DartAdapter;

/// A scenario's `type`: what kind of program it debugs.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ProgramType {
    Dart,
    Flutter,
}

impl ProgramType {
    fn of(config: &LaunchConfig) -> Result<Self, String> {
        match config.program_type.as_deref().map(str::trim) {
            Some("dart") => Ok(Self::Dart),
            Some("flutter") => Ok(Self::Flutter),
            None | Some("") => Err(format!(
                "\"type\" is required for the {NAME} debug adapter: \"dart\" or \"flutter\""
            )),
            Some(other) => Err(format!(
                "\"type\" must be \"dart\" or \"flutter\", not {other:?}"
            )),
        }
    }

    /// The SDK program whose `debug_adapter` debugs it.
    fn executable(self) -> Executable {
        match self {
            Self::Dart => DART,
            Self::Flutter => FLUTTER,
        }
    }

    fn default_program(self) -> &'static str {
        match self {
            Self::Dart => "bin/main.dart",
            Self::Flutter => "lib/main.dart",
        }
    }
}

impl DartAdapter {
    /// The `launch` / `attach` arguments of the SDK's adapter: those that apply to [program_type].
    fn configuration(program_type: ProgramType, config: &LaunchConfig, cwd: &str) -> Value {
        let mut configuration = json!({
            "type": program_type.executable().name,
            "request": request_name(config.request),
            "program": config.program.path.as_deref().unwrap_or(program_type.default_program()),
            "cwd": cwd,
            "args": config.program.args,
            "sendLogsToClient": true,
        });
        set_if_some(&mut configuration, "vmServiceUri", &config.vm_service_uri);
        set_if_some(&mut configuration, "stopOnEntry", &config.stop_on_entry);
        let tool_args = match program_type {
            ProgramType::Dart => config.flutter.tool_args.clone(),
            ProgramType::Flutter => {
                configuration["flutterMode"] =
                    json!(config.flutter.mode.as_deref().unwrap_or("debug"));
                set_if_some(&mut configuration, "platform", &config.flutter.platform);
                config.flutter.tool_args_with_device()
            }
        };
        if !tool_args.is_empty() {
            configuration["toolArgs"] = json!(tool_args);
        }
        configuration
    }
}

impl Named for DartAdapter {
    fn name(&self) -> &'static str {
        NAME
    }
}

impl DebugAdapter for DartAdapter {
    fn binary(
        &self,
        config: &LaunchConfig,
        worktree: &Worktree,
    ) -> Result<DebugAdapterBinary, String> {
        let program_type = ProgramType::of(config)?;
        let executable = program_type.executable();
        let use_fvm = config
            .use_fvm
            .unwrap_or_else(|| project_pins_sdk_with_fvm(worktree));
        let (command, arguments) = if use_fvm {
            (
                FVM.find(worktree)?,
                vec![executable.name.into(), "debug_adapter".into()],
            )
        } else {
            (executable.find(worktree)?, vec!["debug_adapter".into()])
        };

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
                configuration: Self::configuration(program_type, config, &cwd).to_string(),
                request: config.request,
            },
        })
    }

    /// A Dart program, from the "new session" window.
    fn launch_scenario(&self, launch: &LaunchRequest, stop_on_entry: Option<bool>) -> Value {
        let mut config = launch_request_config(launch);
        config["type"] = json!(ProgramType::Dart.executable().name);
        set_if_some(&mut config, "stopOnEntry", &stop_on_entry);
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(config: Value) -> LaunchConfig {
        LaunchConfig::from_value(&config).unwrap()
    }

    fn configuration(config: Value) -> Value {
        let config = parse(config);
        DartAdapter::configuration(ProgramType::of(&config).unwrap(), &config, "/app")
    }

    #[test]
    fn the_type_is_required() {
        let error = |config: Value| ProgramType::of(&parse(config)).err().unwrap();
        assert_eq!(
            error(json!({})),
            "\"type\" is required for the Dart debug adapter: \"dart\" or \"flutter\""
        );
        assert_eq!(
            error(json!({"type": " "})),
            "\"type\" is required for the Dart debug adapter: \"dart\" or \"flutter\""
        );
        assert_eq!(
            error(json!({"type": "python"})),
            "\"type\" must be \"dart\" or \"flutter\", not \"python\""
        );
        assert_eq!(
            ProgramType::of(&parse(json!({"type": "flutter"}))),
            Ok(ProgramType::Flutter)
        );
    }

    #[test]
    fn a_dart_program_gets_no_flutter_fields() {
        assert_eq!(
            configuration(json!({
                "type": "dart",
                "program": "bin/server.dart",
                "deviceId": "chrome",
                "flutterMode": "profile",
                "platform": "web",
            })),
            json!({
                "type": "dart",
                "request": "launch",
                "program": "bin/server.dart",
                "cwd": "/app",
                "args": [],
                "sendLogsToClient": true,
            })
        );
    }

    #[test]
    fn a_dart_program_defaults_to_bin_main() {
        assert_eq!(
            configuration(json!({"type": "dart"}))["program"],
            "bin/main.dart"
        );
    }

    #[test]
    fn a_dart_program_keeps_its_tool_arguments() {
        assert_eq!(
            configuration(json!({"type": "dart", "toolArgs": ["--enable-asserts"]}))["toolArgs"],
            json!(["--enable-asserts"])
        );
    }

    #[test]
    fn a_flutter_program_gets_its_mode_device_and_platform() {
        assert_eq!(
            configuration(json!({
                "type": "flutter",
                "request": "attach",
                "deviceId": "chrome",
                "platform": "web",
                "stopOnEntry": true,
                "vmServiceUri": "ws://127.0.0.1:8181/ws",
            })),
            json!({
                "type": "flutter",
                "request": "attach",
                "program": "lib/main.dart",
                "cwd": "/app",
                "args": [],
                "sendLogsToClient": true,
                "flutterMode": "debug",
                "platform": "web",
                "toolArgs": ["-d", "chrome"],
                "stopOnEntry": true,
                "vmServiceUri": "ws://127.0.0.1:8181/ws",
            })
        );
    }

    #[test]
    fn no_platform_unless_the_scenario_names_one() {
        let configuration = configuration(json!({"type": "flutter"}));
        assert!(configuration.get("platform").is_none());
        assert!(configuration.get("toolArgs").is_none());
    }

    #[test]
    fn a_new_session_debugs_a_dart_program() {
        let launch = LaunchRequest {
            program: "bin/main.dart".into(),
            cwd: None,
            args: Vec::new(),
            envs: Vec::new(),
        };
        assert_eq!(
            DartAdapter.launch_scenario(&launch, Some(true)),
            json!({
                "request": "launch",
                "program": "bin/main.dart",
                "args": [],
                "type": "dart",
                "stopOnEntry": true,
            })
        );
    }
}
