//! The words of a task's command line, the way a shell splits them: `command` (which may hold a
//! whole command line, as the tasks of `languages/dart/tasks.json` do) followed by `args`.

use zed_extension_api::TaskTemplate;

pub struct TaskCommand {
    words: Vec<String>,
}

impl TaskCommand {
    pub fn of(task: &TaskTemplate) -> Self {
        let mut words = split_words(&task.command);
        words.extend(task.args.iter().cloned());
        Self { words }
    }

    /// The words after the script whose file name is [script]: `["web"]` of
    /// `dart "$HOME/.zed-flutter/flutter_run.dart" web` for `flutter_run.dart`.
    pub fn after_script(&self, script: &str) -> Option<&[String]> {
        let index = self
            .words
            .iter()
            .position(|word| file_name(word) == script)?;
        Some(&self.words[index + 1..])
    }
}

/// Splits on blanks, keeping what is in single or double quotes together, quotes removed.
/// Backslashes are kept as they are: they separate the folders of Windows paths.
fn split_words(command_line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for character in command_line.chars() {
        match (quote, character) {
            (Some(open), _) if character == open => quote = None,
            (Some(_), _) => word.push(character),
            (None, '"' | '\'') => {
                quote = Some(character);
                in_word = true;
            }
            (None, _) if character.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            (None, _) => {
                word.push(character);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(word);
    }
    words
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(command: &str, args: &[&str]) -> TaskTemplate {
        TaskTemplate {
            label: "task".into(),
            command: command.into(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            env: Vec::new(),
            cwd: None,
        }
    }

    #[test]
    fn splits_on_blanks_outside_quotes() {
        assert_eq!(
            split_words(r#"dart "$HOME/.zed-flutter/dart_run.dart"  "$ZED_FILE" --port 8080"#),
            [
                "dart",
                "$HOME/.zed-flutter/dart_run.dart",
                "$ZED_FILE",
                "--port",
                "8080"
            ]
        );
        assert_eq!(
            split_words("dart 'my dir/a b.dart' x"),
            ["dart", "my dir/a b.dart", "x"]
        );
        assert_eq!(split_words(r#"a"b c"d"#), ["ab cd"]);
        assert_eq!(split_words(r#"run """#), ["run", ""]);
        assert!(split_words("   ").is_empty());
    }

    #[test]
    fn keeps_backslashes() {
        assert_eq!(
            split_words(r#"dart "C:\Users\me\.zed-flutter\dart_run.dart" a"#),
            ["dart", r"C:\Users\me\.zed-flutter\dart_run.dart", "a"]
        );
    }

    #[test]
    fn the_args_follow_the_command() {
        let command = TaskCommand::of(&task("dart", &["test", "--name", "a b"]));
        assert_eq!(command.words, ["dart", "test", "--name", "a b"]);
    }

    #[test]
    fn finds_the_words_after_a_script() {
        let command = TaskCommand::of(&task(
            r#"dart "$HOME/.zed-flutter/flutter_run.dart" web --release"#,
            &[],
        ));
        assert_eq!(
            command.after_script("flutter_run.dart"),
            Some(&["web".to_string(), "--release".to_string()][..])
        );
        assert_eq!(command.after_script("dart_run.dart"), None);
    }

    #[test]
    fn a_script_matches_by_its_whole_file_name() {
        let command = TaskCommand::of(&task(r"dart C:\tools\my_flutter_run.dart web", &[]));
        assert_eq!(command.after_script("flutter_run.dart"), None);
        let command = TaskCommand::of(&task(r"dart C:\tools\flutter_run.dart web", &[]));
        assert_eq!(
            command.after_script("flutter_run.dart"),
            Some(&["web".to_string()][..])
        );
    }
}
