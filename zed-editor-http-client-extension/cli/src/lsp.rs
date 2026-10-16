use crate::catalog::{self, Catalog};
use crate::endpoint;
use crate::parser;
use crate::vars::{find_env_dir, Context, Environments, ZED_HTTP_DIR};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

pub fn serve() {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut docs: HashMap<String, String> = HashMap::new();
    let mut root: Option<PathBuf> = None;
    let mut next_id = 0u64;
    let mut catalogs = Catalogs::default();
    while let Some(msg) = read_message(&mut input) {
        let id = msg.get("id").cloned();
        // Responses to our own requests (workspace/applyEdit, window/showDocument).
        let Some(method) = msg.get("method").and_then(Value::as_str) else { continue };
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => {
                root = params["workspaceFolders"][0]["uri"]
                    .as_str()
                    .or(params["rootUri"].as_str())
                    .and_then(uri_to_path);
                Some(json!({
                "capabilities": {
                    "textDocumentSync": 1,
                    "definitionProvider": true,
                    "hoverProvider": true,
                    "codeActionProvider": true,
                    "codeLensProvider": { "resolveProvider": false },
                    "executeCommandProvider": { "commands": [GENERATE, OPEN] }
                },
                "serverInfo": { "name": "zed-http" }
                }))
            }
            "shutdown" => Some(Value::Null),
            "exit" => return,
            "textDocument/didOpen" => {
                let d = &params["textDocument"];
                if let (Some(uri), Some(text)) = (d["uri"].as_str(), d["text"].as_str()) {
                    docs.insert(uri.to_string(), text.to_string());
                }
                None
            }
            "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
                if let Some(text) = params["contentChanges"]
                    .as_array()
                    .and_then(|c| c.last())
                    .and_then(|c| c["text"].as_str())
                {
                    docs.insert(uri.to_string(), text.to_string());
                }
                None
            }
            "textDocument/didClose" => {
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
                    docs.remove(uri);
                }
                None
            }
            "textDocument/codeAction" => {
                let c = catalogs.get(root.as_deref());
                Some(code_actions(c, &docs, &params, root.as_deref()))
            }
            "textDocument/codeLens" => {
                let c = catalogs.get(root.as_deref());
                Some(code_lenses(c, &docs, &params, root.as_deref()))
            }
            "workspace/executeCommand" => {
                let c = catalogs.get(root.as_deref());
                execute_command(c, &docs, &params, root.as_deref(), &mut next_id);
                Some(Value::Null)
            }
            "textDocument/definition" => Some(with_variable(&docs, &params, definition).unwrap_or(Value::Null)),
            "textDocument/hover" => Some(with_variable(&docs, &params, hover).unwrap_or(Value::Null)),
            _ => None,
        };
        if let Some(id) = id {
            let response = match result {
                Some(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
                None => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("method not found: {method}") }
                }),
            };
            write_message(&response);
        }
    }
}

fn read_message(input: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            length = v.trim().parse::<usize>().ok();
        }
    }
    let mut buf = vec![0; length?];
    input.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn write_message(msg: &Value) {
    let body = msg.to_string();
    let mut out = io::stdout().lock();
    let _ = write!(out, "Content-Length: {}\r\n\r\n{body}", body.len());
    let _ = out.flush();
}

struct Target<'a> {
    path: PathBuf,
    dir: PathBuf,
    text: &'a str,
    name: String,
}

fn with_variable(
    docs: &HashMap<String, String>,
    params: &Value,
    f: fn(&Target) -> Option<Value>,
) -> Option<Value> {
    let uri = params["textDocument"]["uri"].as_str()?;
    if !is_http_file(uri) {
        return None;
    }
    let line = params["position"]["line"].as_u64()? as usize;
    let character = params["position"]["character"].as_u64()? as usize;
    let text = docs.get(uri)?;
    let name = variable_at(text.lines().nth(line)?, character)?;
    let path = uri_to_path(uri)?;
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    f(&Target { path, dir, text, name })
}

fn is_http_file(uri: &str) -> bool {
    uri.ends_with(".http") || uri.ends_with(".rest")
}

/// The built-in catalog, or built-in + `<root>/.zed/http/frameworks.json` (reloaded when it changes).
#[derive(Default)]
struct Catalogs {
    project: Option<(std::time::SystemTime, Catalog)>,
}

impl Catalogs {
    fn get(&mut self, root: Option<&Path>) -> &Catalog {
        let Some(root) = root else { return Catalog::builtin() };
        let file = catalog::project_file(root);
        let Some(mtime) = fs::metadata(&file).and_then(|m| m.modified()).ok() else {
            return Catalog::builtin();
        };
        if self.project.as_ref().is_none_or(|(t, _)| *t != mtime) {
            match Catalog::for_root(root) {
                Ok(c) => self.project = Some((mtime, c)),
                Err(e) => {
                    // Shown in Zed's language server log; keep working with the built-in catalog.
                    eprintln!("zed-http: {e}");
                    return Catalog::builtin();
                }
            }
        }
        &self.project.as_ref().unwrap().1
    }
}

const GENERATE: &str = "zed-http.generate";
const OPEN: &str = "zed-http.open";

/// What generating the request for `line` would do. All requests from one source file go to
/// `.zed/http/endpoints/<source file>.http`; an already generated request is opened instead.
struct Generation {
    title: String,
    name: String,
    target: PathBuf,
    /// Line of the existing request in `target`, if it was generated before.
    existing: Option<usize>,
    edit: Value,
}

fn generation(catalog: &Catalog, docs: &HashMap<String, String>, uri: &str, line: usize, root: Option<&Path>) -> Option<Generation> {
    if is_http_file(uri) {
        return None;
    }
    let text = docs.get(uri)?;
    let file = uri_to_path(uri)?;
    let (ep, name, content) = endpoint::generate(catalog, text, &file, line)?;
    let root = root.map(Path::to_path_buf).or_else(|| file.parent().map(Path::to_path_buf))?;
    let base = root.join(ZED_HTTP_DIR);
    let dir = base.join("endpoints");
    let target = endpoint::file_for_source(&dir, &file);
    let current = docs
        .get(&path_to_uri(&target))
        .cloned()
        .or_else(|| fs::read_to_string(&target).ok());
    let existing = current.as_deref().and_then(|t| endpoint::find_request(t, &name));

    let mut changes = Vec::new();
    let mut insert = |path: &Path, at: (usize, usize), text: &str, create: bool| {
        let uri = path_to_uri(path);
        if create {
            changes.push(json!({ "kind": "create", "uri": uri, "options": { "ignoreIfExists": true } }));
        }
        let pos = json!({ "line": at.0, "character": at.1 });
        changes.push(json!({
            "textDocument": { "uri": uri, "version": null },
            "edits": [{ "range": { "start": pos, "end": pos }, "newText": text }]
        }));
    };
    if find_env_dir(&dir).is_none() {
        insert(&base.join("http-client.env.json"), (0, 0), endpoint::DEFAULT_ENV, true);
    }
    match &current {
        Some(cur) => insert(&target, end_position(cur), &endpoint::append_text(cur, &content), false),
        None => insert(&target, (0, 0), &content, true),
    }
    Some(Generation {
        title: format!("{} {}", ep.method, ep.url),
        name,
        target,
        existing,
        edit: json!({ "documentChanges": changes }),
    })
}

/// LSP position just past the end of `text`.
fn end_position(text: &str) -> (usize, usize) {
    let line = text.matches('\n').count();
    let last = text.rsplit('\n').next().unwrap_or_default();
    (line, utf16_len(last))
}

/// Line of the newly appended request (its `###` header) after applying the edit.
fn appended_line(target: &Path, docs: &HashMap<String, String>) -> usize {
    let cur = docs.get(&path_to_uri(target)).cloned().or_else(|| fs::read_to_string(target).ok());
    let cur = cur.unwrap_or_default();
    (cur.clone() + &endpoint::append_text(&cur, "")).matches('\n').count()
}

fn range_line(line: usize) -> Value {
    json!({ "start": { "line": line, "character": 0 }, "end": { "line": line, "character": 0 } })
}

/// "Generate .http request" / "Open request" for an endpoint on the cursor line in any source file.
fn code_actions(catalog: &Catalog, docs: &HashMap<String, String>, params: &Value, root: Option<&Path>) -> Value {
    let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
    let Some(line) = params["range"]["start"]["line"].as_u64() else { return json!([]) };
    let Some(g) = generation(catalog, docs, uri, line as usize, root) else { return json!([]) };
    json!([match g.existing {
        Some(at) => json!({
            "title": format!("HTTP: open {} in {}", g.name, file_name(&g.target)),
            "kind": "refactor",
            "command": { "title": "Open", "command": OPEN, "arguments": [path_to_uri(&g.target), at] }
        }),
        None => json!({
            "title": format!("HTTP: generate request {} → {}", g.title, file_name(&g.target)),
            "kind": "refactor",
            "command": { "title": "Generate", "command": GENERATE, "arguments": [uri, line] }
        }),
    }])
}

/// A clickable lens above every route / client call: generates the request, or opens it if it exists.
fn code_lenses(catalog: &Catalog, docs: &HashMap<String, String>, params: &Value, root: Option<&Path>) -> Value {
    let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
    let Some(text) = docs.get(uri).filter(|_| !is_http_file(uri)) else { return json!([]) };
    let Some(file) = uri_to_path(uri) else { return json!([]) };
    let mut lenses = Vec::new();
    for route in catalog.routes(text, &file).into_iter().filter(|r| !r.endpoint.weak) {
        let line = route.line;
        let Some(g) = generation(catalog, docs, uri, line, root) else { continue };
        let command = match g.existing {
            Some(at) => json!({ "title": format!("🔗 Open {}", file_name(&g.target)), "command": OPEN, "arguments": [path_to_uri(&g.target), at] }),
            None => json!({ "title": "🌐 Generate .http request", "command": GENERATE, "arguments": [uri, line] }),
        };
        lenses.push(json!({ "range": range_line(line), "command": command }));
    }
    json!(lenses)
}

fn execute_command(catalog: &Catalog, docs: &HashMap<String, String>, params: &Value, root: Option<&Path>, next_id: &mut u64) {
    let args = &params["arguments"];
    let (target, at) = match params["command"].as_str() {
        Some(GENERATE) => {
            let (Some(uri), Some(line)) = (args[0].as_str(), args[1].as_u64()) else { return };
            let Some(g) = generation(catalog, docs, uri, line as usize, root) else { return };
            if let Some(at) = g.existing {
                (g.target, at)
            } else {
                let at = appended_line(&g.target, docs);
                *next_id += 1;
                write_message(&json!({
                    "jsonrpc": "2.0", "id": format!("zed-http-{next_id}"), "method": "workspace/applyEdit",
                    "params": { "label": format!("Generate {}", g.title), "edit": g.edit }
                }));
                (g.target, at)
            }
        }
        Some(OPEN) => match args[0].as_str().and_then(uri_to_path) {
            Some(p) => (p, args[1].as_u64().unwrap_or(0) as usize),
            None => return,
        },
        _ => return,
    };
    *next_id += 1;
    write_message(&json!({
        "jsonrpc": "2.0", "id": format!("zed-http-{next_id}"), "method": "window/showDocument",
        "params": { "uri": path_to_uri(&target), "takeFocus": true, "selection": range_line(at) }
    }));
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn variable_at(line: &str, utf16_col: usize) -> Option<String> {
    let cursor = byte_index(line, utf16_col);
    let upto = (cursor..=cursor + 2).rev().find(|&e| e <= line.len() && line.is_char_boundary(e))?;
    let open = line[..upto].rfind("{{")?;
    let close = line[open + 2..].find("}}")? + open + 2;
    if cursor > close + 2 {
        return None;
    }
    let name = line[open + 2..close].trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn definition(t: &Target) -> Option<Value> {
    let name = t.name.as_str();
    if name.starts_with('$') {
        return None;
    }
    if let Some((request, _)) = name.split_once(".response") {
        let (line, col) = find_request_name(t.text, request)?;
        return Some(location(&t.path, line, col, request));
    }
    if let Some((line, col)) = find_file_variable(t.text, name) {
        return Some(location(&t.path, line, col, name));
    }
    let env_dir = find_env_dir(&t.dir)?;
    let selected = Environments::load(&t.dir)
        .ok()
        .and_then(|e| e.select(selected_env().as_deref()).ok().flatten());
    let mut all = Vec::new();
    for file in ["http-client.private.env.json", "http-client.env.json"] {
        let path = env_dir.join(file);
        if let Ok(src) = fs::read_to_string(&path) {
            for (env, line, col) in find_env_keys(&src, name) {
                all.push((env, location(&path, line, col, name)));
            }
        }
    }
    let preferred: Vec<Value> = all
        .iter()
        .filter(|(env, _)| Some(env) == selected.as_ref())
        .chain(all.iter().filter(|(env, _)| env == "$shared"))
        .map(|(_, l)| l.clone())
        .collect();
    let result: Vec<Value> = if preferred.is_empty() {
        all.into_iter().map(|(_, l)| l).collect()
    } else {
        preferred
    };
    (!result.is_empty()).then(|| Value::Array(result))
}

fn hover(t: &Target) -> Option<Value> {
    let name = t.name.as_str();
    let envs = Environments::load(&t.dir).ok()?;
    let env = envs.select(selected_env().as_deref()).ok().flatten();
    let file = parser::parse(t.text);
    let mut ctx = Context {
        vars: envs.vars(env.as_deref()),
        responses: fs::read_to_string(t.dir.join(".zed-http").join("responses.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
    };
    ctx.vars.extend(file.variables.iter().cloned());

    let source = if name.starts_with('$') {
        "dynamic variable".to_string()
    } else if name.contains(".response") {
        "stored response".to_string()
    } else if file.variables.iter().any(|(k, _)| k == name) {
        "file variable".to_string()
    } else {
        match &env {
            Some(e) => format!("environment `{e}`"),
            None => "environment".to_string(),
        }
    };
    let value = match ctx.resolve(&format!("{{{{{name}}}}}")) {
        Ok(v) if name.starts_with('$') => format!("example: `{v}`"),
        Ok(v) => format!("```\n{v}\n```"),
        Err(e) => format!("⚠️ {e}"),
    };
    Some(json!({
        "contents": { "kind": "markdown", "value": format!("**{name}** · {source}\n\n{value}") }
    }))
}

fn selected_env() -> Option<String> {
    std::env::var("HTTP_CLIENT_ENV").ok().filter(|s| !s.is_empty())
}

fn find_file_variable(text: &str, name: &str) -> Option<(usize, usize)> {
    text.lines().enumerate().find_map(|(i, line)| {
        let trimmed = line.trim_start();
        let rest = trimmed.strip_prefix('@')?.strip_prefix(name)?;
        let rest = rest.trim_start();
        rest.starts_with('=').then(|| (i, utf16_len(&line[..line.len() - trimmed.len()]) + 1))
    })
}

fn find_request_name(text: &str, name: &str) -> Option<(usize, usize)> {
    text.lines().enumerate().find_map(|(i, line)| {
        let at = line.find("@name")?;
        let rest = line[at + 5..].trim_start_matches(|c: char| c == '=' || c.is_whitespace());
        (rest.trim() == name).then(|| (i, utf16_len(&line[..line.len() - rest.len()])))
    })
}

fn find_env_keys(src: &str, name: &str) -> Vec<(String, usize, usize)> {
    let chars: Vec<char> = src.chars().collect();
    let mut found = Vec::new();
    let (mut i, mut line, mut col, mut depth) = (0, 0, 0, 0);
    let mut env = String::new();
    while i < chars.len() {
        let c = chars[i];
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            '"' => {
                let (start_line, start_col) = (line, col);
                let mut s = String::new();
                i += 1;
                col += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        col += 1;
                    }
                    s.push(chars[i]);
                    col += chars[i].len_utf16();
                    i += 1;
                }
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                if j < chars.len() && chars[j] == ':' {
                    if depth == 1 {
                        env = s;
                    } else if depth == 2 && s == name {
                        found.push((env.clone(), start_line, start_col + 1));
                    }
                }
            }
            _ => {}
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += c.len_utf16();
        }
        i += 1;
    }
    found
}

fn location(path: &Path, line: usize, col: usize, name: &str) -> Value {
    let end = col + name.encode_utf16().count();
    json!({
        "uri": path_to_uri(path),
        "range": {
            "start": { "line": line, "character": col },
            "end": { "line": line, "character": end }
        }
    })
}

fn byte_index(line: &str, utf16_col: usize) -> usize {
    let mut units = 0;
    for (i, c) in line.char_indices() {
        if units >= utf16_col {
            return i;
        }
        units += c.len_utf16();
    }
    line.len()
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let raw = uri.strip_prefix("file://")?;
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&raw[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let mut path = String::from_utf8(out).ok()?;
    if path.len() > 2 && path.as_bytes()[0] == b'/' && path.as_bytes()[2] == b':' {
        path.remove(0);
    }
    Some(PathBuf::from(path))
}

fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from(if s.starts_with('/') { "file://" } else { "file:///" });
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_variable_under_cursor() {
        let line = "GET {{baseUrl}}/get?x={{ token }}";
        assert_eq!(variable_at(line, 8).as_deref(), Some("baseUrl"));
        assert_eq!(variable_at(line, 4).as_deref(), Some("baseUrl"));
        assert_eq!(variable_at(line, 5).as_deref(), Some("baseUrl"));
        assert_eq!(variable_at(line, 14).as_deref(), Some("baseUrl"));
        assert_eq!(variable_at(line, 27).as_deref(), Some("token"));
        assert_eq!(variable_at(line, 17), None);
        assert_eq!(variable_at(line, 0), None);
    }

    #[test]
    fn finds_env_keys_per_environment() {
        let src = "{\n  \"$shared\": { \"baseUrl\": \"a\" },\n  \"dev\": {\n    \"baseUrl\": \"b\",\n    \"x\": {\"baseUrl\": 1}\n  }\n}";
        let keys = find_env_keys(src, "baseUrl");
        assert_eq!(keys, vec![("$shared".into(), 1, 16), ("dev".into(), 3, 5)]);
    }

    #[test]
    fn finds_declarations() {
        let text = "@token = abc\n### x\n# @name login\nGET /";
        assert_eq!(find_file_variable(text, "token"), Some((0, 1)));
        assert_eq!(find_file_variable(text, "tok"), None);
        assert_eq!(find_request_name(text, "login"), Some((2, 8)));
    }

    #[test]
    fn uri_roundtrip() {
        let p = PathBuf::from("/tmp/my dir/env.json");
        assert_eq!(uri_to_path(&path_to_uri(&p)).unwrap(), p);
    }
}
