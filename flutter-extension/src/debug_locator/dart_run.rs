//! The `dart` locator: a `dart run: <file>` task (`dart dart_run.dart <file> [arguments]`) becomes
//! `Dart: debug <file>`, the same program with the same arguments under the `Dart` adapter.

use zed_extension_api::serde_json::json;
use zed_extension_api::{DebugScenario, TaskTemplate};

use super::{debug_scenario, DebugLocator};
use crate::debug_adapter::DART_ADAPTER;
use crate::registry::Named;
use crate::task_command::TaskCommand;

/// The start of the tasks' labels, which the scenarios' labels replace.
const RUN_LABEL: &str = "dart run: ";

pub struct DartRunLocator {
    /// The runner the tasks start, followed by the file and the program's arguments.
    runner: &'static str,
}

impl DartRunLocator {
    pub fn new(runner: &'static str) -> Self {
        Self { runner }
    }
}

impl Named for DartRunLocator {
    fn name(&self) -> &'static str {
        "dart"
    }
}

impl DebugLocator for DartRunLocator {
    /// The task's file stays as the task has it (`$ZED_FILE`): Zed substitutes the variables of
    /// the configuration it sends the adapter.
    fn scenario(&self, task: &TaskTemplate, resolved_label: &str) -> Option<DebugScenario> {
        let command = TaskCommand::of(task);
        let (program, args) = command.after_script(self.runner)?.split_first()?;
        let mut config = json!({
            "type": "dart",
            "request": "launch",
            "program": program,
        });
        if !args.is_empty() {
            config["args"] = json!(args);
        }
        let what = resolved_label
            .strip_prefix(RUN_LABEL)
            .unwrap_or(resolved_label);
        Some(debug_scenario(
            format!("Dart: debug {what}"),
            DART_ADAPTER,
            config,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zed_extension_api::serde_json::{self, Value};

    fn locator() -> DartRunLocator {
        DartRunLocator::new("dart_run.dart")
    }

    fn task(command: &str) -> TaskTemplate {
        TaskTemplate {
            label: "dart run: $ZED_RELATIVE_FILE".into(),
            command: command.into(),
            args: Vec::new(),
            env: Vec::new(),
            cwd: Some("$ZED_WORKTREE_ROOT".into()),
        }
    }

    fn config(scenario: &DebugScenario) -> Value {
        serde_json::from_str(&scenario.config).unwrap()
    }

    #[test]
    fn a_dart_run_task_becomes_a_debug_scenario_of_the_dart_adapter() {
        let scenario = locator()
            .scenario(
                &task(r#"dart "$HOME/.zed-flutter/dart_run.dart" "$ZED_FILE""#),
                "dart run: bin/server.dart",
            )
            .unwrap();
        assert_eq!(scenario.label, "Dart: debug bin/server.dart");
        assert_eq!(scenario.adapter, "Dart");
        assert!(scenario.build.is_none());
        assert_eq!(
            config(&scenario),
            json!({"type": "dart", "request": "launch", "program": "$ZED_FILE"})
        );
    }

    #[test]
    fn the_program_keeps_its_arguments() {
        let scenario = locator()
            .scenario(
                &task(r#"dart "$HOME/.zed-flutter/dart_run.dart" "$ZED_WORKTREE_ROOT/bin/main.dart" --port 8080"#),
                "dart run: bin/main.dart",
            )
            .unwrap();
        assert_eq!(
            config(&scenario),
            json!({
                "type": "dart",
                "request": "launch",
                "program": "$ZED_WORKTREE_ROOT/bin/main.dart",
                "args": ["--port", "8080"],
            })
        );
    }

    #[test]
    fn a_task_of_another_label_keeps_it() {
        let scenario = locator()
            .scenario(&task("dart /tools/dart_run.dart bin/a.dart"), "Serve")
            .unwrap();
        assert_eq!(scenario.label, "Dart: debug Serve");
    }

    #[test]
    fn other_tasks_are_not_dart_runs() {
        assert!(locator()
            .scenario(&task(r#"dart "$HOME/.zed-flutter/dart_run.dart""#), "x")
            .is_none());
        assert!(locator()
            .scenario(
                &task(r#"dart "$HOME/.zed-flutter/flutter_run.dart" web"#),
                "x"
            )
            .is_none());
        assert!(locator()
            .scenario(
                &task(r#"dart "$HOME/.zed-flutter/build_runner_run.dart" build "$ZED_FILE""#),
                "x"
            )
            .is_none());
        assert!(locator().scenario(&task("dart test"), "x").is_none());
    }
}
