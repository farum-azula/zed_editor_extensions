//! The `flutter` locator: a `Flutter: run on <platform>` task (`dart flutter_run.dart <platform>`)
//! becomes `Flutter: debug on <platform>`, the same app on the same kind of device under the
//! `Flutter` adapter.

use zed_extension_api::serde_json::json;
use zed_extension_api::{DebugScenario, TaskTemplate};

use super::{debug_scenario, DebugLocator};
use crate::debug_adapter::FLUTTER_ADAPTER;
use crate::registry::Named;
use crate::task_command::TaskCommand;

pub struct FlutterRunLocator {
    /// The runner the tasks start, followed by the platform.
    runner: &'static str,
    /// The platforms the runner knows.
    targets: &'static [&'static str],
}

impl FlutterRunLocator {
    pub fn new(runner: &'static str, targets: &'static [&'static str]) -> Self {
        Self { runner, targets }
    }
}

impl Named for FlutterRunLocator {
    fn name(&self) -> &'static str {
        "flutter"
    }
}

impl DebugLocator for FlutterRunLocator {
    fn scenario(&self, task: &TaskTemplate, resolved_label: &str) -> Option<DebugScenario> {
        let command = TaskCommand::of(task);
        let target = command.after_script(self.runner)?.first()?;
        if !self.targets.contains(&target.as_str()) {
            return None;
        }
        Some(debug_scenario(
            resolved_label.replacen("Flutter: run on", "Flutter: debug on", 1),
            FLUTTER_ADAPTER,
            json!({
                "request": "launch",
                "program": "lib/main.dart",
                "target": target,
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zed_extension_api::serde_json::{self, Value};

    fn locator() -> FlutterRunLocator {
        FlutterRunLocator::new("flutter_run.dart", &["android", "web"])
    }

    fn task(command: &str) -> TaskTemplate {
        TaskTemplate {
            label: "Flutter: run on Web".into(),
            command: command.into(),
            args: Vec::new(),
            env: Vec::new(),
            cwd: Some("$ZED_WORKTREE_ROOT".into()),
        }
    }

    #[test]
    fn a_run_task_becomes_a_debug_scenario_of_the_flutter_adapter() {
        let scenario = locator()
            .scenario(
                &task(r#"dart "$HOME/.zed-flutter/flutter_run.dart" web"#),
                "Flutter: run on Web",
            )
            .unwrap();
        assert_eq!(scenario.label, "Flutter: debug on Web");
        assert_eq!(scenario.adapter, "Flutter");
        assert!(scenario.build.is_none());
        let config: Value = serde_json::from_str(&scenario.config).unwrap();
        assert_eq!(
            config,
            json!({"request": "launch", "program": "lib/main.dart", "target": "web"})
        );
    }

    #[test]
    fn the_platform_may_come_in_the_args() {
        let mut task = task("dart");
        task.args = vec![
            "/home/me/.zed-flutter/flutter_run.dart".into(),
            "android".into(),
        ];
        let scenario = locator()
            .scenario(&task, "Flutter: run on Android")
            .unwrap();
        assert!(scenario.config.contains("\"target\":\"android\""));
    }

    #[test]
    fn an_unknown_platform_or_another_program_is_not_a_run_task() {
        assert!(locator()
            .scenario(
                &task(r#"dart "$HOME/.zed-flutter/flutter_run.dart" fuchsia"#),
                "x"
            )
            .is_none());
        assert!(locator()
            .scenario(&task(r#"dart "$HOME/.zed-flutter/flutter_run.dart""#), "x")
            .is_none());
        assert!(locator()
            .scenario(
                &task(r#"dart "$HOME/.zed-flutter/dart_run.dart" "$ZED_FILE""#),
                "x"
            )
            .is_none());
        assert!(locator().scenario(&task("flutter run"), "x").is_none());
    }
}
