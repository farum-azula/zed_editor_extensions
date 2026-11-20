//! A debug scenario's `config` (the fields of a `.zed/debug.json` entry, or of a scenario a locator
//! made), read once for both debug adapters. A field of the wrong type is an error, not a default.

use zed_extension_api::serde_json::{self, Value};
use zed_extension_api::StartDebuggingRequestArgumentsRequest as Request;

/// The fields the debug adapters read.
pub struct LaunchConfig {
    pub request: Request,
    /// `type`: what the `Dart` adapter debugs, `dart` or `flutter`.
    pub program_type: Option<String>,
    pub program: Program,
    pub flutter: FlutterRun,
    /// `useFvm`: run the SDK through fvm.
    pub use_fvm: Option<bool>,
    /// `vmServiceUri`: the app to attach to.
    pub vm_service_uri: Option<String>,
    pub stop_on_entry: Option<bool>,
    /// `hotReloadOnSave` (the Flutter proxy).
    pub hot_reload_on_save: Option<bool>,
    /// `devTools`: serve DevTools for the app (the Flutter proxy).
    pub dev_tools: Option<bool>,
}

/// What runs.
pub struct Program {
    /// `program`: the entry point.
    pub path: Option<String>,
    pub cwd: Option<String>,
    /// `args`: the program's arguments.
    pub args: Vec<String>,
    /// `env`: environment variables of the debug adapter, and so of the program.
    pub env: Vec<(String, String)>,
}

/// How `flutter run` runs it.
pub struct FlutterRun {
    /// `flutterMode`: `debug`, `profile` or `release`.
    pub mode: Option<String>,
    /// `toolArgs`: arguments of `flutter run` (or `dart run`).
    pub tool_args: Vec<String>,
    /// `deviceId`: one device, by id.
    pub device_id: Option<String>,
    /// `target`: a kind of device (the Flutter proxy finds or starts one).
    pub target: Option<String>,
    /// `platform` (the `Dart` adapter's Flutter runs).
    pub platform: Option<String>,
}

impl LaunchConfig {
    /// Reads a scenario's JSON `config`.
    pub fn parse(json: &str) -> Result<Self, String> {
        let config: Value =
            serde_json::from_str(json).map_err(|e| format!("invalid debug configuration: {e}"))?;
        Self::from_value(&config)
    }

    pub fn from_value(config: &Value) -> Result<Self, String> {
        if !config.is_object() {
            return Err(format!(
                "invalid debug configuration: an object was expected, not {config}"
            ));
        }
        let fields = Fields(config);
        Ok(Self {
            request: request_kind(config)?,
            program_type: fields.text("type")?,
            program: Program {
                path: fields.text("program")?,
                cwd: fields.text("cwd")?,
                args: fields.list("args")?,
                env: fields.variables("env")?,
            },
            flutter: FlutterRun {
                mode: fields.text("flutterMode")?,
                tool_args: fields.list("toolArgs")?,
                device_id: fields.text("deviceId")?,
                target: fields.text("target")?,
                platform: fields.text("platform")?,
            },
            use_fvm: fields.flag("useFvm")?,
            vm_service_uri: fields.text("vmServiceUri")?,
            stop_on_entry: fields.flag("stopOnEntry")?,
            hot_reload_on_save: fields.flag("hotReloadOnSave")?,
            dev_tools: fields.flag("devTools")?,
        })
    }
}

impl FlutterRun {
    /// `toolArgs`, with `-d <deviceId>` added when `deviceId` is set and they name no device:
    /// Flutter's debug adapter reads the device from the `flutter run` arguments only.
    pub fn tool_args_with_device(&self) -> Vec<String> {
        let mut tool_args = self.tool_args.clone();
        if let Some(device) = &self.device_id {
            let names_device = tool_args
                .iter()
                .any(|arg| arg == "-d" || arg.starts_with("--device-id"));
            if !names_device {
                tool_args.extend(["-d".to_string(), device.clone()]);
            }
        }
        tool_args
    }
}

/// Whether a scenario launches its program or attaches to a running one: `launch` when it does
/// not say.
pub fn request_kind(config: &Value) -> Result<Request, String> {
    match config.get("request") {
        None | Some(Value::Null) => Ok(Request::Launch),
        Some(Value::String(request)) if request == "launch" => Ok(Request::Launch),
        Some(Value::String(request)) if request == "attach" => Ok(Request::Attach),
        Some(other) => Err(format!(
            "\"request\" must be \"launch\" or \"attach\", not {other}"
        )),
    }
}

/// The name of [request] in a launch configuration.
pub fn request_name(request: Request) -> &'static str {
    match request {
        Request::Launch => "launch",
        Request::Attach => "attach",
    }
}

/// Typed reads of a configuration's fields; `null` counts as missing.
struct Fields<'a>(&'a Value);

impl Fields<'_> {
    fn value(&self, key: &str) -> Option<&Value> {
        self.0.get(key).filter(|value| !value.is_null())
    }

    fn text(&self, key: &str) -> Result<Option<String>, String> {
        self.value(key)
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| wrong_type(key, "a string", value))
            })
            .transpose()
    }

    fn flag(&self, key: &str) -> Result<Option<bool>, String> {
        self.value(key)
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| wrong_type(key, "true or false", value))
            })
            .transpose()
    }

    fn list(&self, key: &str) -> Result<Vec<String>, String> {
        let Some(value) = self.value(key) else {
            return Ok(Vec::new());
        };
        let not_a_list = || wrong_type(key, "a list of strings", value);
        value
            .as_array()
            .ok_or_else(not_a_list)?
            .iter()
            .map(|item| item.as_str().map(str::to_string).ok_or_else(not_a_list))
            .collect()
    }

    fn variables(&self, key: &str) -> Result<Vec<(String, String)>, String> {
        let Some(value) = self.value(key) else {
            return Ok(Vec::new());
        };
        value
            .as_object()
            .ok_or_else(|| wrong_type(key, "an object of strings", value))?
            .iter()
            .map(|(name, text)| {
                text.as_str()
                    .map(|text| (name.clone(), text.to_string()))
                    .ok_or_else(|| wrong_type(&format!("{key}.{name}"), "a string", text))
            })
            .collect()
    }
}

fn wrong_type(key: &str, expected: &str, value: &Value) -> String {
    format!("\"{key}\" must be {expected}, not {value}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use zed_extension_api::serde_json::json;

    fn parse(config: Value) -> Result<LaunchConfig, String> {
        LaunchConfig::from_value(&config)
    }

    #[test]
    fn an_empty_config_launches_with_nothing_set() {
        let config = parse(json!({})).unwrap();
        assert!(config.request == Request::Launch);
        assert_eq!(config.program.path, None);
        assert_eq!(config.program.cwd, None);
        assert!(config.program.args.is_empty());
        assert!(config.program.env.is_empty());
        assert_eq!(config.flutter.mode, None);
        assert_eq!(config.flutter.platform, None);
        assert_eq!(config.use_fvm, None);
        assert_eq!(config.hot_reload_on_save, None);
        assert_eq!(config.dev_tools, None);
        assert_eq!(config.stop_on_entry, None);
    }

    #[test]
    fn reads_every_field() {
        let config = parse(json!({
            "request": "attach",
            "type": "flutter",
            "program": "lib/main_dev.dart",
            "cwd": "/app",
            "args": ["--verbose"],
            "env": {"API": "dev"},
            "flutterMode": "profile",
            "toolArgs": ["--flavor", "dev"],
            "deviceId": "emulator-5554",
            "target": "android",
            "platform": "web",
            "useFvm": true,
            "vmServiceUri": "ws://127.0.0.1:8181/ws",
            "stopOnEntry": true,
            "hotReloadOnSave": false,
            "devTools": false,
        }))
        .unwrap();
        assert!(config.request == Request::Attach);
        assert_eq!(config.program_type.as_deref(), Some("flutter"));
        assert_eq!(config.program.path.as_deref(), Some("lib/main_dev.dart"));
        assert_eq!(config.program.cwd.as_deref(), Some("/app"));
        assert_eq!(config.program.args, ["--verbose"]);
        assert_eq!(config.program.env, [("API".to_string(), "dev".to_string())]);
        assert_eq!(config.flutter.mode.as_deref(), Some("profile"));
        assert_eq!(config.flutter.tool_args, ["--flavor", "dev"]);
        assert_eq!(config.flutter.device_id.as_deref(), Some("emulator-5554"));
        assert_eq!(config.flutter.target.as_deref(), Some("android"));
        assert_eq!(config.flutter.platform.as_deref(), Some("web"));
        assert_eq!(config.use_fvm, Some(true));
        assert_eq!(
            config.vm_service_uri.as_deref(),
            Some("ws://127.0.0.1:8181/ws")
        );
        assert_eq!(config.stop_on_entry, Some(true));
        assert_eq!(config.hot_reload_on_save, Some(false));
        assert_eq!(config.dev_tools, Some(false));
    }

    #[test]
    fn null_counts_as_missing() {
        let config = parse(json!({"program": null, "args": null, "useFvm": null})).unwrap();
        assert_eq!(config.program.path, None);
        assert!(config.program.args.is_empty());
        assert_eq!(config.use_fvm, None);
    }

    #[test]
    fn a_field_of_the_wrong_type_is_an_error() {
        let error = |config: Value| parse(config).err().unwrap();
        assert_eq!(
            error(json!({"program": 1})),
            "\"program\" must be a string, not 1"
        );
        assert_eq!(
            error(json!({"useFvm": "yes"})),
            "\"useFvm\" must be true or false, not \"yes\""
        );
        assert_eq!(
            error(json!({"toolArgs": "--flavor dev"})),
            "\"toolArgs\" must be a list of strings, not \"--flavor dev\""
        );
        assert_eq!(
            error(json!({"args": ["a", 2]})),
            "\"args\" must be a list of strings, not [\"a\",2]"
        );
        assert_eq!(
            error(json!({"env": {"PORT": 8080}})),
            "\"env.PORT\" must be a string, not 8080"
        );
        assert_eq!(
            error(json!(["program"])),
            "invalid debug configuration: an object was expected, not [\"program\"]"
        );
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(LaunchConfig::parse("{")
            .err()
            .unwrap()
            .starts_with("invalid debug configuration: "));
    }

    #[test]
    fn the_request_is_launch_unless_it_says_attach() {
        assert!(request_kind(&json!({})) == Ok(Request::Launch));
        assert!(request_kind(&json!({"request": null})) == Ok(Request::Launch));
        assert!(request_kind(&json!({"request": "launch"})) == Ok(Request::Launch));
        assert!(request_kind(&json!({"request": "attach"})) == Ok(Request::Attach));
        assert_eq!(
            request_kind(&json!({"request": "run"})).err().unwrap(),
            "\"request\" must be \"launch\" or \"attach\", not \"run\""
        );
        assert!(request_kind(&json!({"request": true})).is_err());
    }

    #[test]
    fn request_names() {
        assert_eq!(request_name(Request::Launch), "launch");
        assert_eq!(request_name(Request::Attach), "attach");
    }

    #[test]
    fn the_device_goes_into_the_tool_arguments() {
        let run = |config: Value| parse(config).unwrap().flutter.tool_args_with_device();
        assert_eq!(
            run(json!({"deviceId": "chrome", "toolArgs": ["--flavor", "dev"]})),
            ["--flavor", "dev", "-d", "chrome"]
        );
        assert_eq!(
            run(json!({"toolArgs": ["--flavor", "dev"]})),
            ["--flavor", "dev"]
        );
    }

    #[test]
    fn a_device_in_the_tool_arguments_wins() {
        let run = |config: Value| parse(config).unwrap().flutter.tool_args_with_device();
        assert_eq!(
            run(json!({"deviceId": "chrome", "toolArgs": ["-d", "macos"]})),
            ["-d", "macos"]
        );
        assert_eq!(
            run(json!({"deviceId": "chrome", "toolArgs": ["--device-id=macos"]})),
            ["--device-id=macos"]
        );
    }
}
