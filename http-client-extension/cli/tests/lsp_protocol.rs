//! The language server: lenses, code actions, commands, go-to-definition and hover.

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

fn sent(out: &[Value], method: &str) -> Vec<Value> {
    out.iter().filter(|m| m["method"] == json!(method)).cloned().collect()
}

const EXPRESS: &str = "import express from 'express';\napp.get('/users/:id', h);\napp.post('/users', h);\n";

#[test]
fn initialize_advertises_capabilities() {
    let dir = Dir::new();
    let out = talk(vec![init(&dir), request(2, "shutdown", json!(null)), json!({"jsonrpc": "2.0", "method": "exit"})]);
    let caps = &result(&out, 1)["capabilities"];
    assert_eq!(caps["codeLensProvider"]["resolveProvider"], json!(false));
    assert_eq!(caps["codeActionProvider"], json!(true));
    assert_eq!(caps["definitionProvider"], json!(true));
    assert_eq!(caps["hoverProvider"], json!(true));
    assert_eq!(result(&out, 2), json!(null), "shutdown");
}

#[test]
fn initialize_accepts_root_uri_or_nothing() {
    let dir = Dir::new();
    let out = talk(vec![request(1, "initialize", json!({"rootUri": uri(dir.path())}))]);
    assert!(result(&out, 1)["capabilities"].is_object());

    let out = talk(vec![request(1, "initialize", json!({}))]);
    assert!(result(&out, 1)["capabilities"].is_object(), "no workspace folder is fine");
}

#[test]
fn unknown_methods_and_client_responses() {
    let dir = Dir::new();
    let out = talk(vec![
        init(&dir),
        request(2, "textDocument/somethingElse", json!({})),
        // A response from the client to one of our requests carries no method.
        json!({"jsonrpc": "2.0", "id": "zed-http-1", "result": {"applied": true}}),
        // A notification we don't handle produces no reply.
        json!({"jsonrpc": "2.0", "method": "$/setTrace", "params": {}}),
    ]);
    let err = out.iter().find(|m| m["id"] == json!(2)).unwrap();
    assert_eq!(err["error"]["code"], json!(-32601));
    assert_eq!(out.len(), 2, "only initialize and the error: {out:#?}");
}

#[test]
fn malformed_input_stops_the_server() {
    assert!(parse(&run_raw("Content-Length: nonsense\r\n\r\n{}")).is_empty());
    assert!(parse(&run_raw("no headers at all")).is_empty());
    assert!(parse(&run_raw("Content-Length: 500\r\n\r\n{\"short\": true}")).is_empty(), "body shorter than declared");
    assert!(parse(&run_raw("Content-Length: 4\r\n\r\nnot-json")).is_empty());
}

fn run_raw(input: &str) -> String {
    let mut out = Vec::new();
    lsp::serve_io(&mut std::io::Cursor::new(input.as_bytes().to_vec()), &mut out);
    String::from_utf8(out).unwrap()
}

#[test]
fn code_lenses_for_routes_and_none_for_http_files() {
    let dir = Dir::new();
    let src = dir.write("server.ts", EXPRESS);
    let http = dir.write("api.http", "GET http://x/y\n");
    let out = talk(vec![
        init(&dir),
        open(&src, EXPRESS),
        open(&http, "GET http://x/y\n"),
        request(2, "textDocument/codeLens", json!({"textDocument": {"uri": uri(&src)}})),
        request(3, "textDocument/codeLens", json!({"textDocument": {"uri": uri(&http)}})),
        request(4, "textDocument/codeLens", json!({"textDocument": {"uri": "file:///not/open.ts"}})),
    ]);
    let lenses = result(&out, 2);
    assert_eq!(lenses.as_array().unwrap().len(), 2);
    assert_eq!(lenses[0]["command"]["title"], json!("🌐 Generate .http request"));
    assert_eq!(lenses[0]["range"]["start"]["line"], json!(1));
    assert_eq!(result(&out, 3), json!([]), "no lenses inside .http files");
    assert_eq!(result(&out, 4), json!([]), "unknown document");
}

#[test]
fn generate_writes_the_request_and_opens_it() {
    let dir = Dir::new();
    let src = dir.write("server.ts", EXPRESS);
    let generate = |line: u32| {
        talk(vec![
            init(&dir),
            open(&src, EXPRESS),
            request(
                2,
                "workspace/executeCommand",
                json!({"command": "zed-http.generate", "arguments": [uri(&src), line]}),
            ),
        ])
    };

    // The .http file is not open in Zed: it is written to disk, where the runner reads it,
    // instead of an edit that would leave it unsaved.
    let out = generate(1);
    assert!(sent(&out, "workspace/applyEdit").is_empty(), "{out:#?}");
    let written = dir.read(".zed/http/endpoints/server.ts.http");
    assert!(
        written.starts_with("### get-users-by-id\n") && written.contains("GET {{baseUrl}}/users/{{id}}"),
        "{written}"
    );
    assert_eq!(dir.read(".zed/http/http-client.env.json"), zed_http::endpoint::DEFAULT_ENV);
    assert!(!dir.exists(".zed/http/.gitignore"), "no .gitignore");
    let shown = sent(&out, "window/showDocument");
    assert_eq!(shown[0]["params"]["uri"], json!(uri(&dir.join(".zed/http/endpoints/server.ts.http"))));
    assert_eq!(shown[0]["params"]["selection"]["start"]["line"], json!(0));

    // The next request is appended after a blank line, and the environment file is left alone.
    dir.write(".zed/http/http-client.env.json", ENVS);
    let out = generate(2);
    let written = dir.read(".zed/http/endpoints/server.ts.http");
    assert!(written.contains("{{id}}\nAccept: application/json\n\n### post-users\n"), "{written}");
    assert_eq!(dir.read(".zed/http/http-client.env.json"), ENVS);
    let at = written.lines().position(|l| l == "### post-users").unwrap();
    assert_eq!(sent(&out, "window/showDocument")[0]["params"]["selection"]["start"]["line"], json!(at));
}

#[test]
fn generate_edits_a_http_file_that_is_open() {
    let dir = Dir::new();
    let src = dir.write("server.ts", EXPRESS);
    let target = dir.join(".zed/http/endpoints/server.ts.http");
    let unsaved = "### mine\nGET {{baseUrl}}/mine\n";
    let out = talk(vec![
        init(&dir),
        open(&src, EXPRESS),
        open(&target, unsaved),
        request(2, "workspace/executeCommand", json!({"command": "zed-http.generate", "arguments": [uri(&src), 1]})),
    ]);
    let edits = sent(&out, "workspace/applyEdit");
    assert_eq!(edits.len(), 1, "{out:#?}");
    let edit = &edits[0]["params"]["edit"]["changes"][uri(&target)][0];
    assert_eq!(edit["range"]["start"], json!({"line": 2, "character": 0}), "after the buffer's text");
    assert!(edit["newText"].as_str().unwrap().starts_with("\n### get-users-by-id\n"), "{edit}");
    assert!(!target.exists(), "the open buffer is edited, not the file behind it");
    assert!(dir.exists(".zed/http/http-client.env.json"));
    assert_eq!(sent(&out, "window/showDocument")[0]["params"]["selection"]["start"]["line"], json!(3));
}

#[test]
fn a_http_file_does_not_generate_requests_from_itself() {
    let dir = Dir::new();
    let api = dir.write("api.http", "GET https://example.com/users\n");
    let out = talk(vec![
        init(&dir),
        open(&api, "GET https://example.com/users\n"),
        request(2, "workspace/executeCommand", json!({"command": "zed-http.generate", "arguments": [uri(&api), 0]})),
    ]);
    assert_eq!(result(&out, 2), json!(null));
    assert!(sent(&out, "window/showDocument").is_empty());
    assert!(!dir.exists(".zed"));
}

#[test]
fn generate_reports_a_file_it_cannot_write() {
    let dir = Dir::new();
    let src = dir.write("server.ts", EXPRESS);
    dir.write(".zed", "a file where the .zed directory should be");
    let out = talk(vec![
        init(&dir),
        open(&src, EXPRESS),
        request(2, "workspace/executeCommand", json!({"command": "zed-http.generate", "arguments": [uri(&src), 1]})),
    ]);
    assert_eq!(result(&out, 2), json!(null));
    assert!(sent(&out, "window/showDocument").is_empty(), "nothing to show");

    // Same when only the request file is in the way.
    let dir = Dir::new();
    let src = dir.write("server.ts", EXPRESS);
    dir.write(".zed/http/http-client.env.json", ENVS);
    dir.write(".zed/http/endpoints", "a file where the directory should be");
    let out = talk(vec![
        init(&dir),
        open(&src, EXPRESS),
        request(2, "workspace/executeCommand", json!({"command": "zed-http.generate", "arguments": [uri(&src), 1]})),
    ]);
    assert!(sent(&out, "window/showDocument").is_empty());
}

#[test]
fn an_already_generated_request_is_opened_instead() {
    let dir = Dir::new();
    let src = dir.write("server.ts", EXPRESS);
    let generated = "### get-users-by-id\n# @name get-users-by-id\n@id = 1\nGET {{baseUrl}}/users/{{id}}\n";
    dir.write(".zed/http/endpoints/server.ts.http", generated);
    dir.write(".zed/http/http-client.env.json", r#"{"dev": {"baseUrl": "http://x"}}"#);

    let out = talk(vec![
        init(&dir),
        open(&src, EXPRESS),
        request(2, "textDocument/codeLens", json!({"textDocument": {"uri": uri(&src)}})),
        request(
            3,
            "textDocument/codeAction",
            json!({"textDocument": {"uri": uri(&src)},
            "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 0}}}),
        ),
        request(4, "workspace/executeCommand", json!({"command": "zed-http.generate", "arguments": [uri(&src), 1]})),
    ]);
    let lens = result(&out, 2);
    assert_eq!(lens[0]["command"]["title"], json!("🔗 Open server.ts.http"));
    assert_eq!(lens[0]["command"]["command"], json!("zed-http.open"));
    let action = result(&out, 3);
    assert!(action[0]["title"].as_str().unwrap().starts_with("HTTP: open get-users-by-id"), "{action}");
    assert!(sent(&out, "workspace/applyEdit").is_empty(), "nothing is appended twice");
    let shown = sent(&out, "window/showDocument");
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0]["params"]["selection"]["start"]["line"], json!(0));
}

#[test]
fn code_actions_offer_generation() {
    let dir = Dir::new();
    let src = dir.write("server.ts", EXPRESS);
    let http = dir.write("api.http", "GET http://x/y\n");
    let out = talk(vec![
        init(&dir),
        open(&src, EXPRESS),
        open(&http, "GET http://x/y\n"),
        request(
            2,
            "textDocument/codeAction",
            json!({"textDocument": {"uri": uri(&src)},
            "range": {"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 0}}}),
        ),
        request(
            3,
            "textDocument/codeAction",
            json!({"textDocument": {"uri": uri(&src)},
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}}),
        ),
        request(
            4,
            "textDocument/codeAction",
            json!({"textDocument": {"uri": uri(&http)},
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}}),
        ),
        request(5, "textDocument/codeAction", json!({"textDocument": {"uri": uri(&src)}})),
    ]);
    let action = result(&out, 2);
    assert_eq!(action[0]["command"]["command"], json!("zed-http.generate"));
    assert!(action[0]["title"].as_str().unwrap().contains("POST {{baseUrl}}/users"), "{action}");
    assert_eq!(result(&out, 3), json!([]), "no route on that line");
    assert_eq!(result(&out, 4), json!([]), "inside an .http file");
    assert_eq!(result(&out, 5), json!([]), "request without a range");
}

#[test]
fn open_command_and_bad_arguments() {
    let dir = Dir::new();
    let src = dir.write("server.ts", EXPRESS);
    let target = dir.write(".zed/http/endpoints/server.ts.http", "### x\n# @name x\nGET http://x\n");
    let out = talk(vec![
        init(&dir),
        open(&src, EXPRESS),
        request(2, "workspace/executeCommand", json!({"command": "zed-http.open", "arguments": [uri(&target), 2]})),
        request(3, "workspace/executeCommand", json!({"command": "zed-http.open", "arguments": [42]})),
        request(
            4,
            "workspace/executeCommand",
            json!({"command": "zed-http.generate", "arguments": ["file:///nope.ts", 1]}),
        ),
        request(5, "workspace/executeCommand", json!({"command": "zed-http.generate", "arguments": [uri(&src)]})),
        request(6, "workspace/executeCommand", json!({"command": "zed-http.unknown", "arguments": []})),
    ]);
    let shown = sent(&out, "window/showDocument");
    assert_eq!(shown.len(), 1, "only the valid open command shows a document");
    assert_eq!(shown[0]["params"]["selection"]["start"]["line"], json!(2));
    assert_eq!(result(&out, 2), json!(null));
}

#[test]
fn documents_can_be_changed_and_closed() {
    let dir = Dir::new();
    let src = dir.write("server.ts", EXPRESS);
    let changed = "import express from 'express';\napp.put('/items/:id', h);\n";
    let out = talk(vec![
        init(&dir),
        open(&src, EXPRESS),
        json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
            "textDocument": {"uri": uri(&src)}, "contentChanges": [{"text": changed}]}}),
        request(2, "textDocument/codeLens", json!({"textDocument": {"uri": uri(&src)}})),
        // A change without full text (unsupported incremental sync) keeps the old content.
        json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
            "textDocument": {"uri": uri(&src)}, "contentChanges": [{"range": {}}]}}),
        request(3, "textDocument/codeLens", json!({"textDocument": {"uri": uri(&src)}})),
        // A didClose without a uri is ignored, the next one closes the document.
        json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {}}}),
        json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {"uri": uri(&src)}}}),
        request(4, "textDocument/codeLens", json!({"textDocument": {"uri": uri(&src)}})),
        // A didOpen without text is ignored.
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri(&src)}}}),
        request(5, "textDocument/codeLens", json!({"textDocument": {"uri": uri(&src)}})),
    ]);
    assert_eq!(result(&out, 2).as_array().unwrap().len(), 1);
    assert_eq!(result(&out, 3).as_array().unwrap().len(), 1);
    assert_eq!(result(&out, 4), json!([]), "closed document");
    assert_eq!(result(&out, 5), json!([]), "didOpen without text");
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
fn go_to_definition_finds_variables() {
    let (dir, file) = http_project();
    let out = talk(vec![
        init(&dir),
        open(&file, HTTP_FILE),
        request(2, "textDocument/definition", position(&file, 4, 27)), // {{token}} → @token
        request(3, "textDocument/definition", position(&file, 4, 8)),  // {{baseUrl}} → env file
        request(4, "textDocument/definition", position(&file, 5, 15)), // {{apiVersion}} → $shared
        request(5, "textDocument/definition", position(&file, 6, 15)), // {{secret}} → private env
        request(6, "textDocument/definition", position(&file, 7, 11)), // {{$uuid}} → nothing
        request(7, "textDocument/definition", position(&file, 8, 15)), // {{nope}} → nothing
        request(8, "textDocument/definition", position(&file, 11, 30)), // response reference → @name
        request(9, "textDocument/definition", position(&file, 0, 0)),  // not on a variable
        request(
            10,
            "textDocument/definition",
            json!({"textDocument": {"uri": "file:///gone.http"},
            "position": {"line": 0, "character": 0}}),
        ),
    ]);
    assert_eq!(result(&out, 2)["range"]["start"]["line"], json!(0), "file variable");
    let env_def = result(&out, 3);
    assert!(env_def[0]["uri"].as_str().unwrap().ends_with("http-client.env.json"), "{env_def}");
    assert!(result(&out, 4)[0]["uri"].as_str().unwrap().ends_with("http-client.env.json"));
    assert!(result(&out, 5)[0]["uri"].as_str().unwrap().ends_with("http-client.private.env.json"));
    assert_eq!(result(&out, 6), json!(null), "dynamic variables have no definition");
    assert_eq!(result(&out, 7), json!(null), "unknown variable");
    assert_eq!(result(&out, 8)["range"]["start"]["line"], json!(3), "@name login");
    assert_eq!(result(&out, 9), json!(null));
    assert_eq!(result(&out, 10), json!(null), "unopened document");
}

#[test]
fn definition_without_environments() {
    let dir = Dir::new();
    let text = "GET {{baseUrl}}/x\n";
    let file = dir.write("api.http", text);
    let out = talk(vec![init(&dir), open(&file, text), request(2, "textDocument/definition", position(&file, 0, 8))]);
    assert_eq!(result(&out, 2), json!(null), "no environment files anywhere");
}

#[test]
fn hover_shows_values_and_sources() {
    let (dir, file) = http_project();
    let out = talk(vec![
        init(&dir),
        open(&file, HTTP_FILE),
        request(2, "textDocument/hover", position(&file, 4, 27)), // file variable
        request(3, "textDocument/hover", position(&file, 4, 8)),  // environment variable
        request(4, "textDocument/hover", position(&file, 7, 11)), // dynamic
        request(5, "textDocument/hover", position(&file, 11, 30)), // stored response
        request(6, "textDocument/hover", position(&file, 8, 15)), // unknown → warning
        request(7, "textDocument/hover", position(&file, 0, 0)),  // not on a variable
    ]);
    let text = |id| result(&out, id)["contents"]["value"].as_str().unwrap().to_string();
    assert!(text(2).contains("file variable") && text(2).contains("local-token"), "{}", text(2));
    assert!(text(3).contains("environment `dev`") && text(3).contains("http://dev.test"), "{}", text(3));
    assert!(text(4).contains("dynamic variable") && text(4).contains("example:"), "{}", text(4));
    assert!(text(5).contains("stored response") && text(5).contains("42"), "{}", text(5));
    assert!(text(6).contains("⚠️"), "{}", text(6));
    assert_eq!(result(&out, 7), json!(null));
}

#[test]
fn hover_without_environments_says_so() {
    let dir = Dir::new();
    let text = "@a = 1\nGET http://x/{{b}}\n";
    let file = dir.write("api.http", text);
    let out = talk(vec![init(&dir), open(&file, text), request(2, "textDocument/hover", position(&file, 1, 15))]);
    let value = result(&out, 2)["contents"]["value"].as_str().unwrap().to_string();
    assert!(value.contains("environment") && value.contains("⚠️"), "{value}");
}

#[test]
fn a_project_catalog_is_loaded_and_reloaded() {
    let dir = Dir::new();
    let code = "import 'my_client.dart';\napi.call('POST', '/x');\n";
    let src = dir.write("lib/api.dart", code);
    let catalog = |name: &str| {
        format!(
            r#"{{"languages": {{"dart": {{"frameworks": {{"mine": {{"name": "{name}", "imports": ["my_client"],
               "routes": ["api.call(\"{{method}}\", \"{{route}}\""]}}}}}}}}}}"#
        )
    };
    dir.write(".zed/http/frameworks.json", &catalog("Mine"));
    let lens = |dir: &Dir, src: &Path| {
        let out = talk(vec![
            init(dir),
            open(src, code),
            request(2, "textDocument/codeLens", json!({"textDocument": {"uri": uri(src)}})),
        ]);
        result(&out, 2)
    };
    assert_eq!(lens(&dir, &src).as_array().unwrap().len(), 1, "project framework is used");

    // A broken catalog falls back to the built-in one instead of failing.
    dir.write(".zed/http/frameworks.json", "{ broken");
    assert_eq!(lens(&dir, &src), json!([]), "built-in catalog has no such framework");
}

#[test]
fn variable_requests_outside_http_files_are_ignored() {
    let dir = Dir::new();
    let src = dir.write("server.ts", EXPRESS);
    let out = talk(vec![
        init(&dir),
        open(&src, EXPRESS),
        request(
            2,
            "textDocument/definition",
            json!({"textDocument": {"uri": uri(&src)},
            "position": {"line": 1, "character": 12}}),
        ),
        request(
            3,
            "textDocument/hover",
            json!({"textDocument": {"uri": uri(&src)},
            "position": {"line": 1, "character": 12}}),
        ),
    ]);
    assert_eq!(result(&out, 2), json!(null));
    assert_eq!(result(&out, 3), json!(null));
}

#[test]
fn the_project_catalog_is_cached_between_requests() {
    let dir = Dir::new();
    let code = "import 'my_client.dart';\napi.call('POST', '/x');\n";
    let src = dir.write("lib/api.dart", code);
    dir.write(
        ".zed/http/frameworks.json",
        r#"{"languages": {"dart": {"frameworks": {"mine": {"name": "Mine", "imports": ["my_client"],
           "routes": ["api.call(\"{method}\", \"{route}\""]}}}}}"#,
    );
    let out = talk(vec![
        init(&dir),
        open(&src, code),
        request(2, "textDocument/codeLens", json!({"textDocument": {"uri": uri(&src)}})),
        request(3, "textDocument/codeLens", json!({"textDocument": {"uri": uri(&src)}})),
    ]);
    assert_eq!(result(&out, 2), result(&out, 3), "the second request reuses the loaded catalog");
    assert_eq!(result(&out, 3).as_array().unwrap().len(), 1);
}

#[test]
fn definitions_with_one_environment_file_and_escaped_keys() {
    let dir = Dir::new();
    // Only the shared file exists, and one key contains an escaped quote.
    dir.write("http-client.env.json", "{\"dev\": {\"baseUrl\": \"http://x\", \"qu\\\"oted\": \"v\"}}");
    let text = "GET {{baseUrl}}/x\n";
    let file = dir.write("api.http", text);
    let out = talk(vec![init(&dir), open(&file, text), request(2, "textDocument/definition", position(&file, 0, 8))]);
    let def = result(&out, 2);
    assert!(def[0]["uri"].as_str().unwrap().ends_with("http-client.env.json"), "{def}");
}

#[test]
fn without_a_workspace_root_the_file_directory_is_used() {
    let dir = Dir::new();
    let src = dir.write("server.ts", EXPRESS);
    let out = talk(vec![
        request(1, "initialize", json!({})),
        open(&src, EXPRESS),
        request(2, "textDocument/codeLens", json!({"textDocument": {"uri": uri(&src)}})),
        request(3, "workspace/executeCommand", json!({"command": "zed-http.generate", "arguments": [uri(&src), 1]})),
    ]);
    assert_eq!(result(&out, 2).as_array().unwrap().len(), 2);
    assert!(sent(&out, "window/showDocument").len() == 1, "{out:#?}");
    assert!(dir.exists(".zed/http/endpoints/server.ts.http"), "written next to the source file");
}

const ENVS: &str = r#"{ "dev": { "baseUrl": "http://localhost" }, "prod": { "baseUrl": "https://api.example.com" } }"#;

/// Titles of the lenses the server returns for `path`, opened with `text`.
fn lens_titles(dir: &Dir, path: &Path, text: &str) -> Vec<String> {
    let out = talk(vec![
        init(dir),
        open(path, text),
        request(2, "textDocument/codeLens", json!({"textDocument": {"uri": uri(path)}})),
    ]);
    let lenses = result(&out, 2);
    lenses.as_array().unwrap().iter().map(|l| l["command"]["title"].as_str().unwrap().to_string()).collect()
}

#[test]
fn http_files_get_an_environment_switcher() {
    let dir = Dir::new();
    dir.write("http-client.env.json", ENVS);
    let api = dir.join("api.http");

    // Nothing chosen: the default, then the other environments to switch to.
    assert_eq!(lens_titles(&dir, &api, "GET {{baseUrl}}/users\n"), ["env: dev", "prod"]);
    // Chosen in the file.
    assert_eq!(lens_titles(&dir, &api, "# @env prod\n\nGET {{baseUrl}}/users\n"), ["env: prod", "dev"]);
    // Chosen, but not defined anywhere: said so, and every defined one is offered.
    assert_eq!(
        lens_titles(&dir, &api, "// @env staging\nGET {{baseUrl}}/users\n"),
        ["env: staging (not defined)", "dev", "prod"]
    );

    // Without environment files there is nothing to choose from.
    let bare = Dir::new();
    assert!(lens_titles(&bare, &bare.join("api.http"), "GET https://example.com\n").is_empty());
}

#[test]
fn switching_the_environment_edits_the_file() {
    let dir = Dir::new();
    dir.write("http-client.env.json", ENVS);
    let api = dir.join("api.http");
    let switch = |text: &str, name: &str| {
        let out = talk(vec![
            init(&dir),
            open(&api, text),
            request(2, "workspace/executeCommand", json!({"command": "zed-http.env", "arguments": [uri(&api), name]})),
        ]);
        assert_eq!(result(&out, 2), json!(null));
        let edits = sent(&out, "workspace/applyEdit");
        assert_eq!(edits.len(), 1, "{out:#?}");
        assert!(sent(&out, "window/showDocument").is_empty(), "the file is already open");
        edits[0]["params"]["edit"]["changes"][uri(&api)][0].clone()
    };

    // No choice yet: one is written at the top.
    let edit = switch("GET {{baseUrl}}/users\n", "prod");
    assert_eq!(edit["newText"], "# @env prod\n");
    assert_eq!(edit["range"]["start"], json!({"line": 0, "character": 0}));
    assert_eq!(edit["range"]["end"], json!({"line": 0, "character": 0}));

    // A choice already there, above the first `###`, is replaced where it is.
    let edit = switch("// first line\n// @env  prod\n\n### users\nGET {{baseUrl}}/users\n", "dev");
    assert_eq!(edit["newText"], "# @env dev");
    assert_eq!(edit["range"]["start"], json!({"line": 1, "character": 0}));
    assert_eq!(edit["range"]["end"], json!({"line": 1, "character": 13}));

    // Below a `###` the line is that request's, so the file still has no choice of its own.
    let edit = switch("### users\n# @env prod\nGET {{baseUrl}}/users\n", "dev");
    assert_eq!(edit["newText"], "# @env dev\n");
    assert_eq!(edit["range"]["start"], json!({"line": 0, "character": 0}));
}

/// The server's response to `zed-http.env` with `arguments`, for a document with `text`.
fn env_command(text: &str, arguments: Value) -> Vec<Value> {
    let dir = Dir::new();
    dir.write("http-client.env.json", ENVS);
    let api = dir.join("api.http");
    let mut args = arguments.as_array().unwrap().clone();
    args.insert(0, json!(uri(&api)));
    let out = talk(vec![
        init(&dir),
        open(&api, text),
        request(2, "workspace/executeCommand", json!({"command": "zed-http.env", "arguments": args})),
    ]);
    sent(&out, "workspace/applyEdit").iter().map(|e| e["params"]["edit"]["changes"][uri(&api)][0].clone()).collect()
}

const TWO_REQUESTS: &str = "# @env prod\n\n### users\n# @name users\nGET {{baseUrl}}/users\n\n### health\n# @env dev\nGET {{baseUrl}}/health\n";

#[test]
fn a_request_can_have_an_environment_of_its_own() {
    // Given one: inserted right below its `###`.
    let edits = env_command(TWO_REQUESTS, json!(["dev", 4]));
    assert_eq!(edits[0]["newText"], "# @env dev\n");
    assert_eq!(edits[0]["range"]["start"], json!({"line": 3, "character": 0}));

    // Changed where it is.
    let edits = env_command(TWO_REQUESTS, json!(["prod", 8]));
    assert_eq!(edits[0]["newText"], "# @env prod");
    assert_eq!(edits[0]["range"]["start"], json!({"line": 7, "character": 0}));
    assert_eq!(edits[0]["range"]["end"], json!({"line": 7, "character": 10}));

    // Dropped, so that the file's applies again.
    let edits = env_command(TWO_REQUESTS, json!([null, 7]));
    assert_eq!(edits[0]["newText"], "");
    assert_eq!(edits[0]["range"]["start"], json!({"line": 7, "character": 0}));
    assert_eq!(edits[0]["range"]["end"], json!({"line": 8, "character": 0}));

    // Nothing to drop, nothing to do.
    assert!(env_command(TWO_REQUESTS, json!([null, 4])).is_empty());
    assert!(env_command(TWO_REQUESTS, json!([null])).is_empty(), "the file's choice is not removed this way");

    // A request in the file's header has none of its own: its choice is the file's.
    let edits = env_command("# @env prod\nGET {{baseUrl}}/users\n", json!(["dev", 1]));
    assert_eq!(edits[0]["newText"], "# @env dev");
    assert_eq!(edits[0]["range"]["start"], json!({"line": 0, "character": 0}));
}

#[test]
fn a_request_with_its_own_environment_gets_its_own_switcher() {
    let dir = Dir::new();
    dir.write("http-client.env.json", ENVS);
    let api = dir.join("api.http");
    let out = talk(vec![
        init(&dir),
        open(&api, TWO_REQUESTS),
        request(2, "textDocument/codeLens", json!({"textDocument": {"uri": uri(&api)}})),
    ]);
    let lenses = result(&out, 2);
    let row = |line: u64| -> Vec<(String, Value)> {
        lenses
            .as_array()
            .unwrap()
            .iter()
            .filter(|l| l["range"]["start"]["line"] == json!(line))
            .map(|l| (l["command"]["title"].as_str().unwrap().to_string(), l["command"]["arguments"].clone()))
            .collect()
    };
    let file: Vec<String> = row(0).into_iter().map(|(t, _)| t).collect();
    assert_eq!(file, ["env: prod", "dev"]);
    let own = row(7);
    let titles: Vec<&str> = own.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(titles, ["env: dev", "prod", "from file"]);
    assert_eq!(own[1].1, json!([uri(&api), "prod", 7]));
    assert_eq!(own[2].1, json!([uri(&api), null, 7]));
}

#[test]
fn cmd_dot_on_a_request_offers_an_environment_of_its_own() {
    let dir = Dir::new();
    dir.write("http-client.env.json", ENVS);
    let api = dir.join("api.http");
    let header = dir.join("header.http");
    let action_at = |out: &[Value], id: u32| result(out, id).as_array().unwrap().clone();
    let at = |id: u32, path: &Path, line: u32| {
        request(
            id,
            "textDocument/codeAction",
            json!({"textDocument": {"uri": uri(path)},
                   "range": {"start": {"line": line, "character": 0}, "end": {"line": line, "character": 0}}}),
        )
    };
    let out = talk(vec![
        init(&dir),
        open(&api, TWO_REQUESTS),
        open(&header, "GET {{baseUrl}}/x\n"),
        at(2, &api, 4),
        at(3, &api, 8),
        at(4, &header, 0),
        at(5, &api, 0),
    ]);
    let offered = action_at(&out, 2);
    assert_eq!(offered[0]["title"], json!("HTTP: choose an environment for this request"));
    assert_eq!(offered[0]["command"]["arguments"], json!([uri(&api), "prod", 4]), "starts from the file's");
    assert!(action_at(&out, 3).is_empty(), "already has one: its lens switches it");
    assert!(action_at(&out, 4).is_empty(), "in the file's header");
    assert!(action_at(&out, 5).is_empty(), "not on a request");

    // Without environment files there is nothing to choose.
    let bare = Dir::new();
    let file = bare.join("api.http");
    let out = talk(vec![init(&bare), open(&file, TWO_REQUESTS), at(2, &file, 4)]);
    assert!(action_at(&out, 2).is_empty());
}

#[test]
fn variables_resolve_in_the_environment_of_their_request() {
    let dir = Dir::new();
    dir.write("http-client.env.json", ENVS);
    let api = dir.join("api.http");
    let hover = |line: u32| {
        request(
            line + 10,
            "textDocument/hover",
            json!({"textDocument": {"uri": uri(&api)}, "position": {"line": line, "character": 7}}),
        )
    };
    let out = talk(vec![init(&dir), open(&api, TWO_REQUESTS), hover(4), hover(8)]);
    let text = |id: u32| result(&out, id)["contents"]["value"].as_str().unwrap().to_string();
    assert!(text(14).contains("environment `prod`"), "the file's: {}", text(14));
    assert!(text(18).contains("environment `dev`"), "the request's own: {}", text(18));
}

#[test]
fn switching_needs_a_known_document_and_a_name() {
    let dir = Dir::new();
    let api = dir.join("api.http");
    let out = talk(vec![
        init(&dir),
        open(&api, "GET https://example.com\n"),
        request(2, "workspace/executeCommand", json!({"command": "zed-http.env", "arguments": [uri(&api)]})),
        request(
            3,
            "workspace/executeCommand",
            json!({"command": "zed-http.env", "arguments": [uri(&dir.join("closed.http")), "dev"]}),
        ),
    ]);
    assert_eq!(result(&out, 2), json!(null));
    assert_eq!(result(&out, 3), json!(null));
    assert!(sent(&out, "workspace/applyEdit").is_empty());
}

#[test]
fn hover_uses_the_environment_chosen_in_the_file() {
    let dir = Dir::new();
    dir.write("http-client.env.json", ENVS);
    let api = dir.join("api.http");
    let text = "# @env prod\nGET {{baseUrl}}/users\n";
    let out = talk(vec![
        init(&dir),
        open(&api, text),
        request(
            2,
            "textDocument/hover",
            json!({"textDocument": {"uri": uri(&api)}, "position": {"line": 1, "character": 7}}),
        ),
        request(
            3,
            "textDocument/definition",
            json!({"textDocument": {"uri": uri(&api)}, "position": {"line": 1, "character": 7}}),
        ),
    ]);
    let hover = result(&out, 2)["contents"]["value"].as_str().unwrap().to_string();
    assert!(hover.contains("environment `prod`") && hover.contains("https://api.example.com"), "{hover}");
    let definition = result(&out, 3);
    let targets = definition.as_array().unwrap();
    assert_eq!(targets.len(), 1, "only the chosen environment: {definition:#}");
}
