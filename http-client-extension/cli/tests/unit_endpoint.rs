//! Unit tests for `endpoint.rs`.

use std::path::{Path, PathBuf};
use zed_http::endpoint::*;
use zed_http::infer::{self, Spec};

#[test]
fn renders_a_full_request() {
    let spec = Spec {
        query: vec![("page".into(), infer::Ty::Int), ("q".into(), infer::Ty::Str)],
        headers: vec!["X-Request-Id".into()],
        auth: true,
        body: Some(vec![("name".into(), infer::Ty::Str)]),
    };
    let out = render("POST", "{{baseUrl}}/users/{{id}}", "post-users", &spec);
    let expected = "\
### post-users
# @name post-users
# @no-log
# Requires `token` in .zed/http/http-client.env.json
@id = 1
@page = 1
@q = value
@xRequestId = value
POST {{baseUrl}}/users/{{id}}?page={{page}}&q={{q}}
Accept: application/json
Authorization: Bearer {{token}}
X-Request-Id: {{xRequestId}}
Content-Type: application/json

{
  \"name\": \"\"
}
";
    assert_eq!(out, expected);

    // A URL that already has a query string gets `&`, and a plain GET stays minimal.
    let spec = Spec { query: vec![("b".into(), infer::Ty::Str)], ..Default::default() };
    assert!(render("GET", "{{baseUrl}}/x?a=1", "get-x", &spec).contains("/x?a=1&b={{b}}"));
    assert_eq!(
        render("GET", "{{baseUrl}}/x", "get-x", &Spec::default()),
        "### get-x\n# @name get-x\n# @no-log\nGET {{baseUrl}}/x\nAccept: application/json\n"
    );
}

#[test]
fn base_url_prefixing() {
    assert_eq!(to_http_url("/users"), "{{baseUrl}}/users");
    assert_eq!(to_http_url("users"), "{{baseUrl}}/users");
    assert_eq!(to_http_url("https://x.io/a"), "https://x.io/a");
    assert_eq!(to_http_url("http://x.io/a"), "http://x.io/a");
    assert_eq!(to_http_url("{{baseUrl}}/items"), "{{baseUrl}}/items", "already a full URL");
    assert_eq!(to_http_url("{{API_HOST}}/items"), "{{API_HOST}}/items");
    assert_eq!(to_http_url("{{id}}"), "{{baseUrl}}/{{id}}", "a path that starts with a parameter");
    assert_eq!(to_http_url("{{id}}/avatar"), "{{baseUrl}}/{{id}}/avatar");
    assert_eq!(to_http_url("{{unterminated"), "{{baseUrl}}/{{unterminated");
}

#[test]
fn per_source_file() {
    assert_eq!(file_for_source(Path::new("/r"), Path::new("/p/server.ts")), PathBuf::from("/r/server.ts.http"));
    let text = "### a\n# @name a\nGET /a\n\n### b\n# @name b\n@id = 1\nGET /b/{{id}}\n";
    assert_eq!(find_request(text, "b"), Some(4));
    assert_eq!(find_request(text, "c"), None);
    assert_eq!(append_text("x\n", "y"), "\ny");
    assert_eq!(append_text("x\n\n", "y"), "y", "already separated");
    assert_eq!(append_text("x", "y"), "\n\ny", "no trailing newline");
    assert_eq!(append_text("", "y"), "y");
}

#[test]
fn request_names_come_from_the_method_and_path() {
    assert_eq!(default_name("GET", "{{baseUrl}}/users/{{id}}"), "get-users-by-id");
    assert_eq!(default_name("POST", "{{baseUrl}}/users"), "post-users");
    assert_eq!(default_name("GET", "https://api.example.com/v1/a/b"), "get-v1-a-b", "the host is dropped");
    assert_eq!(default_name("GET", "https://api.example.com"), "get", "a bare host");
    assert_eq!(slug("Create User!"), "create-user");
    assert_eq!(slug("???"), "");
}
