//! Unit tests for `lsp.rs`.

use std::path::{Path, PathBuf};
use zed_http::lsp::{find_env_keys, find_file_variable, find_request_name, path_to_uri, uri_to_path, variable_at};

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
fn windows_and_escaped_uris() {
    assert_eq!(uri_to_path("file:///C:/code/api.http").unwrap(), PathBuf::from("C:/code/api.http"));
    assert_eq!(uri_to_path("file:///tmp/a%20b.http").unwrap(), PathBuf::from("/tmp/a b.http"));
    assert_eq!(uri_to_path("file:///tmp/%zz.http").unwrap(), PathBuf::from("/tmp/%zz.http"), "invalid escape kept");
    assert_eq!(uri_to_path("http://example.com/x"), None, "only file URIs");
    assert_eq!(path_to_uri(Path::new("C:/code/api.http")), "file:///C:/code/api.http");
}

#[test]
fn cursor_past_the_end_of_a_line() {
    assert_eq!(variable_at("GET {{a}}", 99), Some("a".into()), "clamped to the end of the line");
    assert_eq!(variable_at("GET {{a}} and a long tail", 99), None, "too far from the variable");
    assert_eq!(variable_at("", 0), None);
}

#[test]
fn uri_roundtrip() {
    let p = PathBuf::from("/tmp/my dir/env.json");
    assert_eq!(uri_to_path(&path_to_uri(&p)).unwrap(), p);
}
