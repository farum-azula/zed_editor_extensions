//! Unit tests for `parser.rs`.

use zed_http::parser::{self, parse_var_decl, Body};

#[test]
fn variable_declarations_and_version_suffix() {
    assert_eq!(parse_var_decl("@ok-name.v2 = 1"), Some(("ok-name.v2".into(), "1".into())));
    assert_eq!(parse_var_decl("@ = 2"), None, "empty name");
    assert_eq!(parse_var_decl("@bad name = 3"), None, "space in the name");
    assert_eq!(parse_var_decl("@no-equals"), None);
    assert_eq!(parse_var_decl("name = 4"), None, "no @");
    let f = parser::parse("@ok = 1\n\nGET /x HTTP/1.1\n");
    assert_eq!(f.variables, vec![("ok".to_string(), "1".to_string())]);
    assert_eq!(f.requests[0].url, "/x", "the HTTP version is dropped");

    let f = parser::parse("GET /y?q=a b\n");
    assert_eq!(f.requests[0].url, "/y?q=a b", "only an HTTP/x version suffix is dropped");
}

#[test]
fn odd_lines_inside_a_request() {
    let f = parser::parse("# @name\nGET /x\nnot-a-header\nAccept: text/plain\n\n# @name ignored\nbody line\n");
    let r = &f.requests[0];
    assert_eq!(r.name, None, "`# @name` without a value");
    assert_eq!(r.headers, vec![("Accept".into(), "text/plain".into())], "a line without `:` is skipped");
    assert_eq!(r.body, Some(Body::Inline("body line".into())), "a leading @-comment is not body");
}

#[test]
fn comments_between_headers_are_skipped() {
    let f = parser::parse("GET /x\n# a comment\nAccept: text/plain\n// another\nX-Trace: 1\n");
    let r = &f.requests[0];
    assert_eq!(r.headers, vec![("Accept".into(), "text/plain".into()), ("X-Trace".into(), "1".into())]);
}

#[test]
fn parses_sections() {
    let src = "@host = http://localhost\n\n### Get users\n# @name users\nGET {{host}}/users\nAccept: application/json\n\n###\nPOST {{host}}/users\nContent-Type: application/json\n\n{\"a\": 1}\n\n> {%\n client.log(1)\n%}\n";
    let f = parser::parse(src);
    assert_eq!(f.variables, vec![("host".into(), "http://localhost".into())]);
    assert_eq!(f.requests.len(), 2);
    assert_eq!(f.requests[0].name.as_deref(), Some("users"));
    assert_eq!(f.requests[0].title.as_deref(), Some("Get users"));
    assert_eq!(f.requests[0].headers.len(), 1);
    assert_eq!(f.requests[1].method, "POST");
    assert_eq!(f.requests[1].body, Some(Body::Inline("{\"a\": 1}".to_string())));
}

#[test]
fn multiline_url_and_no_method() {
    let f = parser::parse("https://example.com/api\n    ?a=1\n    &b=2\n");
    assert_eq!(f.requests[0].method, "GET");
    assert_eq!(f.requests[0].url, "https://example.com/api?a=1&b=2");
}
