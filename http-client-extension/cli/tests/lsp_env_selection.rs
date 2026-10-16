//! Environment selection via `HTTP_CLIENT_ENV`, in its own test binary so the variable
//! cannot leak into other tests.

mod support;

use serde_json::{json, Value};
use std::path::Path;
use support::Dir;
use zed_http::lsp;

/// Frames messages, runs the server over them and returns everything it wrote.
fn talk(messages: Vec<Value>) -> Vec<Value> {
    let mut input = Vec::new();
    for m in &messages {
        let body = m.to_string();
        input.extend_from_slice(format!("Content-Length: {}\r\n\r\n{body}", body.len()).as_bytes());
    }
    let mut out = Vec::new();
    lsp::serve_io(&mut std::io::Cursor::new(input), &mut out);
    parse(&String::from_utf8(out).unwrap())
}

fn parse(raw: &str) -> Vec<Value> {
    raw.split("Content-Length: ")
        .skip(1)
        .filter_map(|part| part.split_once("\r\n\r\n").map(|(_, body)| body))
        .filter_map(|body| {
            // Each body is a complete JSON value; the next header follows immediately.
            let mut de = serde_json::Deserializer::from_str(body).into_iter::<Value>();
            de.next().and_then(Result::ok)
        })
        .collect()
}

fn uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

fn init(root: &Dir) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
           "params": {"workspaceFolders": [{"uri": uri(root.path())}]}})
}

fn open(path: &Path, text: &str) -> Value {
    json!({"jsonrpc": "2.0", "method": "textDocument/didOpen",
           "params": {"textDocument": {"uri": uri(path), "text": text}}})
}

fn request(id: u32, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

fn result(out: &[Value], id: u32) -> Value {
    out.iter().find(|m| m["id"] == json!(id)).unwrap_or_else(|| panic!("no response with id {id} in {out:#?}"))
        ["result"]
        .clone()
}


const HTTP_FILE: &str = "\
@token = local-token

### login
# @name login
GET {{baseUrl}}/login?t={{token}}
X-Api: {{apiVersion}}
X-Secret: {{secret}}
X-Dyn: {{$uuid}}
X-Missing: {{nope}}

### next
GET {{baseUrl}}/me?id={{login.response.body.id}}
";

fn http_project() -> (Dir, std::path::PathBuf) {
    let dir = Dir::new();
    dir.write(
        "http-client.env.json",
        r#"{"$shared": {"apiVersion": "v1"}, "dev": {"baseUrl": "http://dev.test"}, "prod": {"baseUrl": "http://prod.test"}}"#,
    );
    dir.write("http-client.private.env.json", r#"{"dev": {"secret": "s3cret"}}"#);
    dir.write(".zed-http/responses.json", r#"{"login": {"status": 200, "headers": {}, "body": {"id": 42}}}"#);
    let file = dir.write("api.http", HTTP_FILE);
    (dir, file)
}

fn position(file: &Path, line: u32, character: u32) -> Value {
    json!({"textDocument": {"uri": uri(file)}, "position": {"line": line, "character": character}})
}

#[test]
fn definition_prefers_the_selected_environment() {
    let (dir, file) = http_project();
    std::env::set_var("HTTP_CLIENT_ENV", "prod");
    let out =
        talk(vec![init(&dir), open(&file, HTTP_FILE), request(2, "textDocument/definition", position(&file, 4, 8))]);
    std::env::remove_var("HTTP_CLIENT_ENV");
    let def = result(&out, 2);
    assert_eq!(def.as_array().unwrap().len(), 1, "only the prod entry: {def}");
    assert_eq!(def[0]["range"]["start"]["line"], json!(0));
}

#[test]
fn the_environment_variable_selects_the_profile_for_run() {
    let dir = Dir::new();
    dir.write(
        "http-client.env.json",
        r#"{"dev": {"baseUrl": "http://127.0.0.1:1"}, "prod": {"baseUrl": "http://127.0.0.1:2"}}"#,
    );
    let file = dir.write("api.http", "GET {{baseUrl}}/x\n");
    std::env::set_var("HTTP_CLIENT_ENV", "prod");
    let code = zed_http::cli::run_args(vec!["run".into(), file.to_string_lossy().into(), "--all".into()]);
    std::env::remove_var("HTTP_CLIENT_ENV");
    assert_eq!(code, 1, "the request fails: nothing listens on that port");
}
