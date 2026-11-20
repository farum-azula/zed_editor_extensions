//! The Zed side of the extension. Each part Zed asks for by name is one type, registered in
//! [`FlutterExtension::new`]:
//!
//! - debug adapters ([`debug_adapter`]): `Flutter`, Flutter's own adapter behind
//!   `tool/flutter_proxy.dart` (hot reload on save, DevTools, the device of a `target`), and
//!   `Dart`, the Dart extension's;
//! - language servers ([`language_server`]): `dart`, the Dart extension's, and `flutter-tools`,
//!   `tool/flutter_lsp.dart` (the snippets, the runners of the tasks, `build_runner watch`);
//! - debug locators ([`debug_locator`]): `flutter` offers each `Flutter: run on <platform>` task in
//!   the debugger as `Flutter: debug on <platform>`, `dart` each `dart run: <file>` task as
//!   `Dart: debug <file>`.
//!
//! The programs of `tool/` are plain Dart that the Dart SDK coming with Flutter runs, so there is
//! nothing to download or build. They are installed into the extension's work directory
//! ([`tool_files`]) before one of them starts, so they always match this version of the extension.

mod debug_adapter;
mod debug_locator;
mod language_server;
mod launch_config;
mod registry;
mod sdk;
mod task_command;
mod tool_files;

use zed_extension_api::serde_json::Value;
use zed_extension_api::{
    self as zed, CodeLabel, DebugAdapterBinary, DebugConfig, DebugRequest, DebugScenario,
    DebugTaskDefinition, LanguageServerId, Result, StartDebuggingRequestArgumentsRequest,
    TaskTemplate, Worktree,
};

use debug_adapter::{DartAdapter, DebugAdapter, FlutterAdapter};
use debug_locator::{DartRunLocator, DebugLocator, FlutterRunLocator};
use language_server::{DartLanguageServer, FlutterToolsServer, LanguageServer};
use launch_config::{request_kind, LaunchConfig};
use registry::Registry;
use tool_files::ToolFiles;

// RUNNER, TARGETS and TOOLS stay in this file: tests/check_tasks.py checks them against
// languages/dart/tasks.json, tool/devices.dart and the files of tool/.

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
/// The runner the `dart run: <file>` tasks start, followed by the file and its arguments.
const DART_RUNNER: &str = "dart_run.dart";

/// The files of `tool/` that [`ToolFiles`] installs into the work directory: the one list of them.
const TOOLS: &[(&str, &str)] = &[
    (
        "flutter_proxy.dart",
        include_str!("../tool/flutter_proxy.dart"),
    ),
    ("flutter_lsp.dart", include_str!("../tool/flutter_lsp.dart")),
    ("flutter_run.dart", include_str!("../tool/flutter_run.dart")),
    ("dart_run.dart", include_str!("../tool/dart_run.dart")),
    ("devices.dart", include_str!("../tool/devices.dart")),
    ("devtools.dart", include_str!("../tool/devtools.dart")),
    (
        "flutter_devtools.dart",
        include_str!("../tool/flutter_devtools.dart"),
    ),
    ("protocol.dart", include_str!("../tool/protocol.dart")),
    ("lsp_server.dart", include_str!("../tool/lsp_server.dart")),
    (
        "snippet_completions.dart",
        include_str!("../tool/snippet_completions.dart"),
    ),
    (
        "runner_installation.dart",
        include_str!("../tool/runner_installation.dart"),
    ),
    ("paths.dart", include_str!("../tool/paths.dart")),
    ("pubspec.dart", include_str!("../tool/pubspec.dart")),
    (
        "build_runner.dart",
        include_str!("../tool/build_runner.dart"),
    ),
    (
        "build_runner_watches.dart",
        include_str!("../tool/build_runner_watches.dart"),
    ),
    (
        "build_runner_run.dart",
        include_str!("../tool/build_runner_run.dart"),
    ),
    ("snippets.json", include_str!("../tool/snippets.json")),
];

struct FlutterExtension {
    debug_adapters: Registry<dyn DebugAdapter>,
    language_servers: Registry<dyn LanguageServer>,
    debug_locators: Registry<dyn DebugLocator>,
}

impl zed::Extension for FlutterExtension {
    fn new() -> Self {
        let tools = ToolFiles::new(TOOLS);
        Self {
            debug_adapters: Registry::<dyn DebugAdapter>::new("debug adapter")
                .with(Box::new(FlutterAdapter::new(tools)))
                .with(Box::new(DartAdapter)),
            language_servers: Registry::<dyn LanguageServer>::new("language server")
                .with(Box::new(DartLanguageServer))
                .with(Box::new(FlutterToolsServer::new(tools))),
            debug_locators: Registry::<dyn DebugLocator>::new("debug locator")
                .with(Box::new(FlutterRunLocator::new(RUNNER, TARGETS)))
                .with(Box::new(DartRunLocator::new(DART_RUNNER))),
        }
    }

    fn get_dap_binary(
        &mut self,
        adapter_name: String,
        task: DebugTaskDefinition,
        _user_provided_debug_adapter_path: Option<String>,
        worktree: &Worktree,
    ) -> Result<DebugAdapterBinary, String> {
        let adapter = self.debug_adapters.get(&adapter_name)?;
        adapter.binary(&LaunchConfig::parse(&task.config)?, worktree)
    }

    fn dap_request_kind(
        &mut self,
        adapter_name: String,
        config: Value,
    ) -> Result<StartDebuggingRequestArgumentsRequest, String> {
        self.debug_adapters.get(&adapter_name)?;
        request_kind(&config)
    }

    /// A session started from the debugger's "new session" window: run the given entry point.
    fn dap_config_to_scenario(&mut self, config: DebugConfig) -> Result<DebugScenario, String> {
        let adapter = self.debug_adapters.get(&config.adapter)?;
        let DebugRequest::Launch(launch) = &config.request else {
            return Err("attaching from this window is not supported; use a debug.json scenario with \"request\": \"attach\"".into());
        };
        let scenario = adapter.launch_scenario(launch, config.stop_on_entry);
        Ok(DebugScenario {
            label: config.label,
            adapter: config.adapter,
            build: None,
            config: scenario.to_string(),
            tcp_connection: None,
        })
    }

    fn dap_locator_create_scenario(
        &mut self,
        locator_name: String,
        task: TaskTemplate,
        resolved_label: String,
        _debug_adapter_name: String,
    ) -> Option<DebugScenario> {
        self.debug_locators
            .find(&locator_name)?
            .scenario(&task, &resolved_label)
    }

    fn language_server_command(
        &mut self,
        id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<zed::Command> {
        self.language_servers.get(id.as_ref())?.command(worktree)
    }

    fn language_server_workspace_configuration(
        &mut self,
        id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<Value>> {
        self.language_servers
            .get(id.as_ref())?
            .workspace_configuration(worktree)
    }

    fn label_for_completion(
        &self,
        id: &LanguageServerId,
        completion: zed::lsp::Completion,
    ) -> Option<CodeLabel> {
        self.language_servers
            .find(id.as_ref())?
            .label_for_completion(completion)
    }
}

zed::register_extension!(FlutterExtension);

#[cfg(test)]
mod tests {
    use super::*;
    use zed::Extension;

    /// The names of the `[<section>.<name>]` tables of extension.toml.
    fn declared(section: &str) -> Vec<String> {
        let prefix = format!("[{section}.");
        let mut names: Vec<String> = include_str!("../extension.toml")
            .lines()
            .filter_map(|line| line.strip_prefix(&prefix)?.strip_suffix(']'))
            .map(str::to_string)
            .collect();
        names.sort();
        names
    }

    fn sorted(names: Vec<&str>) -> Vec<String> {
        let mut names: Vec<String> = names.into_iter().map(str::to_string).collect();
        names.sort();
        names
    }

    #[test]
    fn every_part_extension_toml_declares_is_registered() {
        let extension = FlutterExtension::new();
        assert_eq!(
            sorted(extension.debug_adapters.names()),
            declared("debug_adapters")
        );
        assert_eq!(
            sorted(extension.language_servers.names()),
            declared("language_servers")
        );
        assert_eq!(
            sorted(extension.debug_locators.names()),
            declared("debug_locators")
        );
    }

    #[test]
    fn the_request_of_an_unknown_adapter_is_an_error() {
        let mut extension = FlutterExtension::new();
        assert!(extension
            .dap_request_kind("Python".into(), Value::Null)
            .err()
            .unwrap()
            .starts_with("Flutter Tools has no debug adapter named \"Python\""));
    }

    #[test]
    fn a_locator_answers_only_to_its_name() {
        let mut extension = FlutterExtension::new();
        let task = TaskTemplate {
            label: "Flutter: run on Web".into(),
            command: format!("dart \"$HOME/.zed-flutter/{RUNNER}\" web"),
            args: Vec::new(),
            env: Vec::new(),
            cwd: None,
        };
        let scenario = |extension: &mut FlutterExtension, locator: &str| {
            extension.dap_locator_create_scenario(
                locator.into(),
                task.clone(),
                task.label.clone(),
                "Flutter".into(),
            )
        };
        assert!(scenario(&mut extension, "flutter").is_some());
        assert!(scenario(&mut extension, "dart").is_none());
        assert!(scenario(&mut extension, "cargo").is_none());
    }

    #[test]
    fn every_tool_file_is_installed_once() {
        let mut names: Vec<&str> = TOOLS.iter().map(|(name, _)| *name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), TOOLS.len());
        for name in [
            RUNNER,
            DART_RUNNER,
            "flutter_proxy.dart",
            "flutter_lsp.dart",
        ] {
            assert!(names.contains(&name), "{name}");
        }
    }
}
