//! Go-to-definition and hover of the `{{variable}}` under the cursor in a `.http` file. Each kind
//! of variable is a [`VariableKind`] in [`VARIABLE_KINDS`]: what hover says about it, and where
//! its definition is.

use super::documents::is_http_file;
use super::environments::env_at;
use super::protocol::{
    byte_index, decode, location, uri_to_path, utf16_len, Client, Position, TextDocumentPositionParams,
};
use super::{Handler, Outcome, Session};
use crate::cli::load_store;
use crate::parser::{self, HttpFile};
use crate::vars::{find_env_dir, Context, Environments};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) struct Definition;
pub(super) struct Hover;

/// The environment files, private first: a definition in it is the one that applies.
const ENVIRONMENT_FILES: [&str; 2] = ["http-client.private.env.json", "http-client.env.json"];

/// The variable under the cursor and the file it is in.
struct VariableTarget<'a> {
    /// The cursor line (0-based).
    line: usize,
    path: PathBuf,
    directory: PathBuf,
    text: &'a str,
    name: String,
}

/// The variable under the cursor in an open `.http` file, if there is one.
fn target<'a>(session: &'a Session, params: Value) -> Option<VariableTarget<'a>> {
    let params: TextDocumentPositionParams = decode(params)?;
    let uri = params.text_document.uri;
    if !is_http_file(&uri) {
        return None;
    }
    let Position { line, character } = params.position;
    let text = session.documents.get(&uri)?;
    let name = variable_at(text.lines().nth(line)?, character)?;
    let path = uri_to_path(&uri)?;
    let directory = path.parent().map(Path::to_path_buf).unwrap_or_default();
    Some(VariableTarget { line, path, directory, text, name })
}

impl Handler for Definition {
    fn method(&self) -> &'static str {
        "textDocument/definition"
    }

    fn handle(&self, params: Value, session: &mut Session, _client: &mut Client) -> Outcome {
        let definition = target(session, params).and_then(|target| {
            let kind = VARIABLE_KINDS.iter().find(|kind| kind.defines(&target))?;
            kind.definition(&target)
        });
        Outcome::Result(definition.unwrap_or(Value::Null))
    }
}

impl Handler for Hover {
    fn method(&self) -> &'static str {
        "textDocument/hover"
    }

    fn handle(&self, params: Value, session: &mut Session, _client: &mut Client) -> Outcome {
        Outcome::Result(target(session, params).and_then(|target| hover(&target)).unwrap_or(Value::Null))
    }
}

fn hover(target: &VariableTarget) -> Option<Value> {
    let name = target.name.as_str();
    let environments = Environments::load(&target.directory).ok()?;
    let environment = environments.select(env_at(target.text, target.line).as_deref()).ok().flatten();
    let file = parser::parse(target.text);
    let mut context = Context {
        vars: environments.vars(environment.as_deref()),
        responses: load_store(&target.directory.join(".zed-http").join("responses.json")),
        ..Default::default()
    };
    context.vars.extend(file.variable_pairs());
    let kind = VARIABLE_KINDS.iter().find(|kind| kind.describes(name, &file))?;
    let value = match context.resolve(&format!("{{{{{name}}}}}")) {
        Ok(value) => kind.show_value(&value),
        Err(error) => format!("⚠️ {error}"),
    };
    let source = kind.source(environment.as_deref());
    Some(json!({ "contents": { "kind": "markdown", "value": format!("**{name}** · {source}\n\n{value}") } }))
}

/// One kind of variable. Kinds are asked in order; the last one takes every variable.
trait VariableKind {
    /// Whether hover describes the variable `name` of `file` as this kind.
    fn describes(&self, name: &str, file: &HttpFile) -> bool;
    /// Where hover says the value comes from; `environment` is the one the request runs in.
    fn source(&self, environment: Option<&str>) -> String;
    /// How hover shows the resolved value.
    fn show_value(&self, value: &str) -> String {
        format!("```\n{value}\n```")
    }
    /// Whether go-to-definition looks for the target as this kind.
    fn defines(&self, target: &VariableTarget) -> bool;
    /// The location(s) the target is defined at.
    fn definition(&self, target: &VariableTarget) -> Option<Value>;
}

/// `{{$uuid}}`, `{{$timestamp}}`…: generated when the request runs, declared nowhere.
struct DynamicVariable;
/// `{{login.response.body.id}}`: a value of the stored response of the request `# @name login`.
struct ResponseReference;
/// `{{token}}` declared in the file with `@token = …`.
struct FileVariable;
/// Anything else: a variable of the environment files.
struct EnvironmentVariable;

const VARIABLE_KINDS: [&dyn VariableKind; 4] =
    [&DynamicVariable, &ResponseReference, &FileVariable, &EnvironmentVariable];

impl VariableKind for DynamicVariable {
    fn describes(&self, name: &str, _file: &HttpFile) -> bool {
        name.starts_with('$')
    }

    fn source(&self, _environment: Option<&str>) -> String {
        "dynamic variable".into()
    }

    /// The value changes on every run: the one shown is only an example.
    fn show_value(&self, value: &str) -> String {
        format!("example: `{value}`")
    }

    fn defines(&self, target: &VariableTarget) -> bool {
        target.name.starts_with('$')
    }

    fn definition(&self, _target: &VariableTarget) -> Option<Value> {
        None
    }
}

impl VariableKind for ResponseReference {
    fn describes(&self, name: &str, _file: &HttpFile) -> bool {
        name.contains(".response")
    }

    fn source(&self, _environment: Option<&str>) -> String {
        "stored response".into()
    }

    fn defines(&self, target: &VariableTarget) -> bool {
        target.name.contains(".response")
    }

    /// The `# @name` of the request whose response it reads.
    fn definition(&self, target: &VariableTarget) -> Option<Value> {
        let (request, _) = target.name.split_once(".response")?;
        Some(location(&target.path, find_request_name(target.text, request)?, request))
    }
}

impl VariableKind for FileVariable {
    fn describes(&self, name: &str, file: &HttpFile) -> bool {
        file.variables.iter().any(|variable| variable.name == name)
    }

    fn source(&self, _environment: Option<&str>) -> String {
        "file variable".into()
    }

    fn defines(&self, target: &VariableTarget) -> bool {
        find_file_variable(target.text, &target.name).is_some()
    }

    fn definition(&self, target: &VariableTarget) -> Option<Value> {
        Some(location(&target.path, find_file_variable(target.text, &target.name)?, &target.name))
    }
}

impl VariableKind for EnvironmentVariable {
    fn describes(&self, _name: &str, _file: &HttpFile) -> bool {
        true
    }

    fn source(&self, environment: Option<&str>) -> String {
        match environment {
            Some(environment) => format!("environment `{environment}`"),
            None => "environment".to_string(),
        }
    }

    fn defines(&self, _target: &VariableTarget) -> bool {
        true
    }

    /// Its keys in the environment files: those of the environment the request runs in and of
    /// `$shared`, or else every one.
    fn definition(&self, target: &VariableTarget) -> Option<Value> {
        let name = target.name.as_str();
        let directory = find_env_dir(&target.directory)?;
        let selected = Environments::load(&target.directory)
            .ok()
            .and_then(|environments| environments.select(env_at(target.text, target.line).as_deref()).ok().flatten());
        let mut all: Vec<(String, Value)> = Vec::new();
        for file in ENVIRONMENT_FILES {
            let path = directory.join(file);
            if let Ok(source) = fs::read_to_string(&path) {
                for key in find_env_keys(&source, name) {
                    all.push((key.environment, location(&path, key.position, name)));
                }
            }
        }
        let preferred: Vec<Value> = all
            .iter()
            .filter(|(environment, _)| Some(environment) == selected.as_ref())
            .chain(all.iter().filter(|(environment, _)| environment == "$shared"))
            .map(|(_, location)| location.clone())
            .collect();
        let locations: Vec<Value> =
            if preferred.is_empty() { all.into_iter().map(|(_, location)| location).collect() } else { preferred };
        (!locations.is_empty()).then(|| Value::Array(locations))
    }
}

/// The `{{name}}` under the cursor, if any.
pub fn variable_at(line: &str, utf16_column: usize) -> Option<String> {
    let cursor = byte_index(line, utf16_column);
    let upto = (cursor..=cursor + 2).rev().find(|&end| end <= line.len() && line.is_char_boundary(end))?;
    let open = line[..upto].rfind("{{")?;
    let close = line[open + 2..].find("}}")? + open + 2;
    if cursor > close + 2 {
        return None;
    }
    let name = line[open + 2..close].trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// The position of an `@name =` declaration.
pub fn find_file_variable(text: &str, name: &str) -> Option<Position> {
    text.lines().enumerate().find_map(|(line, content)| {
        let trimmed = content.trim_start();
        let rest = trimmed.strip_prefix('@')?.strip_prefix(name)?;
        let indent = &content[..content.len() - trimmed.len()];
        rest.trim_start().starts_with('=').then(|| Position { line, character: utf16_len(indent) + 1 })
    })
}

/// The position of the name in a `# @name` declaration.
pub fn find_request_name(text: &str, name: &str) -> Option<Position> {
    text.lines().enumerate().find_map(|(line, content)| {
        let comment = parser::comment_of(content)?;
        let value =
            parser::directive_argument(comment, "@name")?.trim_start_matches(|c: char| c == '=' || c.is_whitespace());
        // `value` lies within `content`: where it starts there.
        let start = value.as_ptr() as usize - content.as_ptr() as usize;
        (value.trim() == name).then(|| Position { line, character: utf16_len(&content[..start]) })
    })
}

/// A key declared in an environment file.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvKey {
    /// The environment it is declared in (`dev`, `$shared`…).
    pub environment: String,
    pub position: Position,
}

/// Every place a key is declared at in an environment file.
pub fn find_env_keys(source: &str, name: &str) -> Vec<EnvKey> {
    let chars: Vec<char> = source.chars().collect();
    let mut found = Vec::new();
    let (mut index, mut line, mut column, mut depth) = (0, 0, 0, 0);
    let mut environment = String::new();
    while index < chars.len() {
        let c = chars[index];
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            '"' => {
                let start = Position { line, character: column + 1 };
                let mut key = String::new();
                index += 1;
                column += 1;
                while index < chars.len() && chars[index] != '"' {
                    if chars[index] == '\\' && index + 1 < chars.len() {
                        index += 1;
                        column += 1;
                    }
                    key.push(chars[index]);
                    column += chars[index].len_utf16();
                    index += 1;
                }
                let mut next = index + 1;
                while next < chars.len() && chars[next].is_whitespace() {
                    next += 1;
                }
                if next < chars.len() && chars[next] == ':' {
                    if depth == 1 {
                        environment = key;
                    } else if depth == 2 && key == name {
                        found.push(EnvKey { environment: environment.clone(), position: start });
                    }
                }
            }
            _ => {}
        }
        if c == '\n' {
            line += 1;
            column = 0;
        } else {
            column += c.len_utf16();
        }
        index += 1;
    }
    found
}
