//! The Dart language server and its completion labels, as Zed's Dart extension provides them
//! (https://github.com/zed-extensions/dart, Apache-2.0; from its `src/dart.rs`). Flutter Tools
//! defines the Dart language itself so that its tasks can sit next to Dart's (see
//! `languages/dart`), which takes the place of that extension.

use zed_extension_api::lsp::{Completion, CompletionKind};
use zed_extension_api::serde_json::{json, Value};
use zed_extension_api::{self as zed, CodeLabel, CodeLabelSpan, Worktree};

use super::{lsp_settings, LanguageServer};
use crate::registry::Named;
use crate::sdk::DART;

/// The arguments of `dart` unless `lsp.dart.binary.arguments` says otherwise.
const DEFAULT_ARGUMENTS: [&str; 2] = ["language-server", "--protocol=lsp"];

pub struct DartLanguageServer;

impl Named for DartLanguageServer {
    /// Also the key of its settings (`lsp.dart`).
    fn name(&self) -> &'static str {
        "dart"
    }
}

impl LanguageServer for DartLanguageServer {
    fn command(&self, worktree: &Worktree) -> zed::Result<zed::Command> {
        let binary = lsp_settings(self.name(), worktree)?.binary;
        let args = binary
            .as_ref()
            .and_then(|binary| binary.arguments.clone())
            .unwrap_or_else(|| DEFAULT_ARGUMENTS.map(str::to_string).to_vec());
        let command = match binary.and_then(|binary| binary.path) {
            Some(path) => path,
            None => DART.find(worktree)?,
        };
        Ok(zed::Command {
            command,
            args,
            env: worktree.shell_env(),
        })
    }

    fn workspace_configuration(&self, worktree: &Worktree) -> zed::Result<Option<Value>> {
        let settings = lsp_settings(self.name(), worktree)?
            .settings
            .unwrap_or_default();
        Ok(Some(json!({ "dart": settings })))
    }

    fn label_for_completion(&self, completion: Completion) -> Option<CodeLabel> {
        completion_label(completion)
    }
}

/// A class, function, property or variable shown highlighted as Dart code: `name(params) → type`.
fn completion_label(completion: Completion) -> Option<CodeLabel> {
    let arrow = " → ";

    match completion.kind? {
        CompletionKind::Class => Some(CodeLabel {
            filter_range: (0..completion.label.len()).into(),
            spans: vec![CodeLabelSpan::literal(
                completion.label,
                Some("type".into()),
            )],
            code: String::new(),
        }),
        CompletionKind::Function | CompletionKind::Constructor | CompletionKind::Method => {
            let mut parts = completion.detail.as_ref()?.split(arrow);
            let (name, _) = completion.label.split_once('(')?;
            let parameter_list = parts.next()?;
            let return_type = parts.next()?;
            let fn_name = " a";
            let fat_arrow = " => ";
            let call_expr = "();";

            let code =
                format!("{return_type}{fn_name}{parameter_list}{fat_arrow}{name}{call_expr}");

            let parameter_list_start = return_type.len() + fn_name.len();

            Some(CodeLabel {
                spans: vec![
                    CodeLabelSpan::code_range(
                        code.len() - call_expr.len() - name.len()..code.len() - call_expr.len(),
                    ),
                    CodeLabelSpan::code_range(
                        parameter_list_start..parameter_list_start + parameter_list.len(),
                    ),
                    CodeLabelSpan::literal(arrow, None),
                    CodeLabelSpan::code_range(0..return_type.len()),
                ],
                filter_range: (0..name.len()).into(),
                code,
            })
        }
        CompletionKind::Property => {
            let class_start = "class A {";
            let get = " get ";
            let property_end = " => a; }";
            let ty = completion.detail?;
            let name = completion.label;

            let code = format!("{class_start}{ty}{get}{name}{property_end}");
            let name_start = class_start.len() + ty.len() + get.len();

            Some(CodeLabel {
                spans: vec![
                    CodeLabelSpan::code_range(name_start..name_start + name.len()),
                    CodeLabelSpan::literal(arrow, None),
                    CodeLabelSpan::code_range(class_start.len()..class_start.len() + ty.len()),
                ],
                filter_range: (0..name.len()).into(),
                code,
            })
        }
        CompletionKind::Variable => {
            let name = completion.label;

            Some(CodeLabel {
                filter_range: (0..name.len()).into(),
                spans: vec![CodeLabelSpan::literal(name, Some("variable".into()))],
                code: String::new(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completion(kind: CompletionKind, label: &str, detail: Option<&str>) -> Completion {
        Completion {
            label: label.into(),
            label_details: None,
            detail: detail.map(str::to_string),
            kind: Some(kind),
            insert_text_format: None,
        }
    }

    #[test]
    fn a_method_reads_as_dart_code() {
        let label = completion_label(completion(
            CompletionKind::Method,
            "add(…)",
            Some("(int a, int b) → int"),
        ))
        .unwrap();
        assert_eq!(label.code, "int a(int a, int b) => add();");
        assert_eq!((label.filter_range.start, label.filter_range.end), (0, 3));
    }

    #[test]
    fn a_property_reads_as_a_getter() {
        let label =
            completion_label(completion(CompletionKind::Property, "name", Some("String"))).unwrap();
        assert_eq!(label.code, "class A {String get name => a; }");
    }

    #[test]
    fn a_method_without_its_signature_has_no_label() {
        assert!(completion_label(completion(CompletionKind::Method, "add(…)", None)).is_none());
        assert!(completion_label(completion(CompletionKind::Keyword, "class", None)).is_none());
    }
}
