//! The debug locators (`[debug_locators.<name>]` of `extension.toml`), one type each. Zed asks
//! them about each task of the Dart language (it names debuggers in `languages/dart/config.toml`),
//! and offers a scenario one of them makes in the Debug tab.

mod dart_run;
mod flutter_run;

pub use dart_run::DartRunLocator;
pub use flutter_run::FlutterRunLocator;

use zed_extension_api::serde_json::Value;
use zed_extension_api::{DebugScenario, TaskTemplate};

use crate::registry::Named;

pub trait DebugLocator: Named {
    /// The scenario that debugs what [task] runs, when it is a task this locator knows.
    fn scenario(&self, task: &TaskTemplate, resolved_label: &str) -> Option<DebugScenario>;
}

/// A scenario of [adapter] that needs no build first.
fn debug_scenario(label: String, adapter: &str, config: Value) -> DebugScenario {
    DebugScenario {
        label,
        adapter: adapter.into(),
        build: None,
        config: config.to_string(),
        tcp_connection: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DART_RUNNER, RUNNER, TARGETS};
    use zed_extension_api::serde_json;

    const TASKS: &str = include_str!("../../languages/dart/tasks.json");

    fn tasks() -> Vec<TaskTemplate> {
        let tasks: Vec<Value> = serde_json::from_str(TASKS).unwrap();
        tasks
            .iter()
            .map(|task| TaskTemplate {
                label: task["label"].as_str().unwrap().into(),
                command: task["command"].as_str().unwrap().into(),
                args: task["args"]
                    .as_array()
                    .map(|args| {
                        args.iter()
                            .map(|arg| arg.as_str().unwrap().to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
                env: Vec::new(),
                cwd: task["cwd"].as_str().map(str::to_string),
            })
            .collect()
    }

    fn locators() -> Vec<Box<dyn DebugLocator>> {
        vec![
            Box::new(FlutterRunLocator::new(RUNNER, TARGETS)),
            Box::new(DartRunLocator::new(DART_RUNNER)),
        ]
    }

    /// The names of the locators that offer [task] in the debugger.
    fn offered_by(task: &TaskTemplate) -> Vec<&'static str> {
        locators()
            .iter()
            .filter(|locator| locator.scenario(task, &task.label).is_some())
            .map(|locator| locator.name())
            .collect()
    }

    #[test]
    fn every_run_task_of_the_dart_language_is_offered_in_the_debugger_by_one_locator() {
        let tasks = tasks();
        let runs: Vec<_> = tasks
            .iter()
            .filter(|task| {
                task.label.starts_with("Flutter: run on ") || task.label.starts_with("dart run: ")
            })
            .collect();
        assert!(runs.len() >= 8, "the run tasks: {}", runs.len());
        for task in runs {
            let expected = if task.label.starts_with("Flutter") {
                "flutter"
            } else {
                "dart"
            };
            assert_eq!(offered_by(task), [expected], "{}", task.label);
        }
    }

    #[test]
    fn other_tasks_are_not() {
        for task in tasks().iter().filter(|task| {
            !task.label.starts_with("Flutter: run on ") && !task.label.starts_with("dart run: ")
        }) {
            assert!(offered_by(task).is_empty(), "{}", task.label);
        }
    }
}
