//! The Zed side of the extension.
//!
//! - The Dart language, as Zed's Dart extension provides it ([`dart`]): its language server and its
//!   `Dart` debug adapter. The language's tasks (`languages/dart/tasks.json`) add Flutter's
//!   `Flutter: run on <platform>`.
//! - The `flutter` debug locator, which offers each of those tasks in the debugger as
//!   `Flutter: debug on <platform>`.
//! - The `Flutter` debug adapter: Flutter's own (`flutter debug_adapter`) behind
//!   `tool/flutter_proxy.dart`, which hot reloads the app when a `.dart` file under `lib/` changes
//!   and picks (or starts) the device of a scenario's `target`.
//! - The `flutter-tools` language server, `tool/flutter_lsp.dart`: it offers the snippets
//!   (`tool/snippets.json`) as completions that add the imports they need, and keeps
//!   `~/.zed-flutter/flutter_run.dart`, the runner of the run tasks, up to date.
//!
//! The programs of `tool/` are plain Dart that the Dart SDK coming with Flutter runs, so there is
//! nothing to download or build. They are written into the extension's work directory whenever one
//! of them starts, so they always match this version of the extension.

mod dart;

use std::fs;
use zed_extension_api::serde_json::{self, json, Value};
use zed_extension_api::{
    self as zed, CodeLabel, DebugAdapterBinary, DebugConfig, DebugRequest, DebugScenario,
    DebugTaskDefinition, LanguageServerId, Result, StartDebuggingRequestArguments,
    StartDebuggingRequestArgumentsRequest, TaskTemplate, Worktree,
};

const ADAPTER: &str = "Flutter";
const DART_ADAPTER: &str = "Dart";
/// The runner the `Flutter: run on <platform>` tasks start, followed by the platform.
const RUNNER: &str = "flutter_run.dart";
/// The platforms of those tasks; `tool/devices.dart` knows how to find a device for each.
const TARGETS: &[&str] = &[
    "ios-simulator",
    "android",
    "web",
    "macos",
    "windows",
    "linux",
];

/// The files of `tool/`, written into the work directory.
const TOOLS: &[(&str, &str)] = &[
    (
        "flutter_proxy.dart",
        include_str!("../tool/flutter_proxy.dart"),
    ),
    ("flutter_lsp.dart", include_str!("../tool/flutter_lsp.dart")),
    ("flutter_run.dart", include_str!("../tool/flutter_run.dart")),
    ("devices.dart", include_str!("../tool/devices.dart")),
    ("protocol.dart", include_str!("../tool/protocol.dart")),
    ("snippets.json", include_str!("../tool/snippets.json")),
];

struct FlutterExtension;

impl zed::Extension for FlutterExtension {
    fn new() -> Self {
        Self
    }

    fn get_dap_binary(
        &mut self,
        adapter_name: String,
        task: DebugTaskDefinition,
        _user_provided_debug_adapter_path: Option<String>,
        worktree: &Worktree,
    ) -> Result<DebugAdapterBinary, String> {
        if adapter_name == DART_ADAPTER {
            return dart::get_dap_binary(task, worktree);
        }
        let config: Value = serde_json::from_str(&task.config)
            .map_err(|e| format!("invalid debug configuration: {e}"))?;
        // A project that pins its Flutter with fvm runs it through fvm unless told otherwise.
        let fvm = worktree.read_text_file(".fvmrc").is_ok();
        let launch = Launch::from_config(&config, &worktree.root_path(), fvm);

        let proxy = tool("flutter_proxy.dart")?;

        let (command, mut arguments) = if launch.use_fvm {
            let fvm = worktree
                .which("fvm")
                .ok_or("fvm is not in PATH; install it or set useFvm to false")?;
            (
                fvm.clone(),
                vec!["dart".into(), proxy, fvm, "flutter".into()],
            )
        } else {
            let dart = dart(worktree)?;
            let flutter = which(worktree, &["flutter", "flutter.bat"])
                .ok_or("flutter is not in PATH; install it from flutter.dev")?;
            (dart, vec![proxy, flutter])
        };
        arguments.push("debug_adapter".into());

        Ok(DebugAdapterBinary {
            command: Some(command),
            arguments,
            envs: launch.env.clone(),
            cwd: Some(launch.cwd.clone()),
            connection: None,
            request_args: StartDebuggingRequestArguments {
                configuration: launch.configuration().to_string(),
                request: launch.request,
            },
        })
    }

    fn language_server_command(
        &mut self,
        id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<zed::Command> {
        if id.as_ref() == dart::SERVER {
            return dart::language_server_command(worktree);
        }
        Ok(zed::Command {
            command: dart(worktree)?,
            args: vec![tool("flutter_lsp.dart")?],
            env: worktree.shell_env(),
        })
    }

    fn language_server_workspace_configuration(
        &mut self,
        id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<Value>> {
        if id.as_ref() == dart::SERVER {
            return dart::workspace_configuration(worktree);
        }
        Ok(None)
    }

    fn label_for_completion(
        &self,
        id: &LanguageServerId,
        completion: zed::lsp::Completion,
    ) -> Option<CodeLabel> {
        (id.as_ref() == dart::SERVER)
            .then(|| dart::label_for_completion(completion))
            .flatten()
    }

    fn dap_request_kind(
        &mut self,
        adapter_name: String,
        config: Value,
    ) -> Result<StartDebuggingRequestArgumentsRequest, String> {
        if adapter_name == DART_ADAPTER {
            return dart::dap_request_kind(config);
        }
        request_kind(&config)
    }

    /// Offers a `Flutter: run on <platform>` task in the debugger as `Flutter: debug on <platform>`:
    /// the same app on the same kind of device, under the `Flutter` adapter.
    fn dap_locator_create_scenario(
        &mut self,
        _locator_name: String,
        task: TaskTemplate,
        resolved_label: String,
        _debug_adapter_name: String,
    ) -> Option<DebugScenario> {
        let target = run_target(&task)?;
        Some(DebugScenario {
            label: resolved_label.replacen("Flutter: run on", "Flutter: debug on", 1),
            adapter: ADAPTER.into(),
            build: None,
            config: json!({
                "request": "launch",
                "program": "lib/main.dart",
                "target": target,
            })
            .to_string(),
            tcp_connection: None,
        })
    }

    /// A session started from the debugger's "new session" window: run the given entry point.
    fn dap_config_to_scenario(&mut self, config: DebugConfig) -> Result<DebugScenario, String> {
        let DebugRequest::Launch(launch) = config.request else {
            return Err("attaching from this window is not supported; use a debug.json scenario with \"request\": \"attach\"".into());
        };
        let mut scenario = json!({
            "request": "launch",
            "program": launch.program,
            "args": launch.args,
        });
        if let Some(cwd) = launch.cwd {
            scenario["cwd"] = json!(cwd);
        }
        if !launch.envs.is_empty() {
            let env: serde_json::Map<_, _> = launch
                .envs
                .into_iter()
                .map(|(k, v)| (k, json!(v)))
                .collect();
            scenario["env"] = json!(env);
        }
        if config.adapter == DART_ADAPTER {
            scenario["type"] = json!("dart");
        }
        Ok(DebugScenario {
            label: config.label,
            adapter: config.adapter,
            build: None,
            config: scenario.to_string(),
            tcp_connection: None,
        })
    }
}

/// A `Flutter` debug configuration, with its defaults filled in.
struct Launch {
    request: StartDebuggingRequestArgumentsRequest,
    program: String,
    cwd: String,
    args: Vec<String>,
    tool_args: Vec<String>,
    env: Vec<(String, String)>,
    flutter_mode: String,
    vm_service_uri: Option<String>,
    target: Option<String>,
    use_fvm: bool,
    hot_reload_on_save: bool,
}

impl Launch {
    fn from_config(config: &Value, root: &str, fvm: bool) -> Self {
        let text = |key: &str| config.get(key).and_then(Value::as_str).map(str::to_string);
        let list = |key: &str| -> Vec<String> {
            config
                .get(key)
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut tool_args = list("toolArgs");
        // Flutter's adapter reads the device from the `flutter run` arguments only.
        if let Some(device) = text("deviceId") {
            if !tool_args
                .iter()
                .any(|a| a == "-d" || a.starts_with("--device-id"))
            {
                tool_args.extend(["-d".to_string(), device]);
            }
        }
        Self {
            request: request_kind(config).unwrap_or(StartDebuggingRequestArgumentsRequest::Launch),
            program: text("program").unwrap_or_else(|| "lib/main.dart".into()),
            cwd: text("cwd").unwrap_or_else(|| root.into()),
            args: list("args"),
            tool_args,
            env: config
                .get("env")
                .and_then(Value::as_object)
                .map(|vars| {
                    vars.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            flutter_mode: text("flutterMode").unwrap_or_else(|| "debug".into()),
            vm_service_uri: text("vmServiceUri"),
            target: text("target"),
            use_fvm: config.get("useFvm").and_then(Value::as_bool).unwrap_or(fvm),
            hot_reload_on_save: config
                .get("hotReloadOnSave")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }

    /// The `launch` / `attach` arguments Flutter's adapter reads (and `hotReloadOnSave` and
    /// `target`, which the proxy reads).
    fn configuration(&self) -> Value {
        let mut config = json!({
            "program": self.program,
            "cwd": self.cwd,
            "args": self.args,
            "toolArgs": self.tool_args,
            "flutterMode": self.flutter_mode,
            "hotReloadOnSave": self.hot_reload_on_save,
        });
        if let Some(uri) = &self.vm_service_uri {
            config["vmServiceUri"] = json!(uri);
        }
        if let Some(target) = &self.target {
            config["target"] = json!(target);
        }
        config
    }
}

/// The platform of a `Flutter: run on <platform>` task: the word after the runner in its command.
fn run_target(task: &TaskTemplate) -> Option<String> {
    let command = [task.command.as_str()]
        .into_iter()
        .chain(task.args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    let (_, after) = command.split_once(RUNNER)?;
    let target = after
        .trim_start_matches(['"', '\''])
        .split_whitespace()
        .next()?;
    TARGETS.contains(&target).then(|| target.to_string())
}

fn request_kind(config: &Value) -> Result<StartDebuggingRequestArgumentsRequest, String> {
    match config.get("request").and_then(Value::as_str) {
        None | Some("launch") => Ok(StartDebuggingRequestArgumentsRequest::Launch),
        Some("attach") => Ok(StartDebuggingRequestArgumentsRequest::Attach),
        Some(other) => Err(format!("unknown request {other:?} for the {ADAPTER} debug adapter: use \"launch\" or \"attach\"")),
    }
}

/// Writes the files of `tool/` into the work directory; returns the host path of [name] there.
fn tool(name: &str) -> Result<String, String> {
    for (file, text) in TOOLS {
        if fs::read_to_string(file).ok().as_deref() != Some(*text) {
            fs::write(file, text).map_err(|e| format!("could not write {file}: {e}"))?;
        }
    }
    Ok(work_dir()?.join(name).to_string_lossy().into_owned())
}

fn dart(worktree: &Worktree) -> Result<String, String> {
    which(worktree, &["dart", "dart.exe", "dart.bat"])
        .ok_or_else(|| "dart is not in PATH; install Flutter from flutter.dev".into())
}

fn which(worktree: &Worktree, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| worktree.which(name))
}

/// The extension's work directory, as a path for the host: the WASI sandbox reports Windows paths
/// as `/C:/...`.
fn work_dir() -> Result<std::path::PathBuf, String> {
    let dir = std::env::current_dir().map_err(|e| format!("no work directory: {e}"))?;
    let text = dir.to_string_lossy();
    match text.strip_prefix('/') {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => Ok(rest.into()),
        _ => Ok(dir),
    }
}

zed::register_extension!(FlutterExtension);
