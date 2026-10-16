//! Unit tests for `pattern.rs`.

use std::collections::HashMap;
use zed_http::pattern::{keys_of, Matcher};

fn caps(p: &str, text: &str) -> Option<HashMap<String, String>> {
    Matcher::one(p, &[]).unwrap().unwrap().first(text).map(|h| h.caps)
}

#[test]
fn placeholders() {
    let c = caps(r#"@{method}Mapping("{path}")"#, r#"  @GetMapping( "/users/{id}" )"#).unwrap();
    assert_eq!(c["method"], "Get");
    assert_eq!(c["path"], "/users/{id}");
    assert!(caps(r#"@{method}Mapping"#, "@RequestMapping").is_none());
    let c = caps(r#".{method}{generic}("{route}""#, "_dio.post<Map<String, dynamic>>('/auth', data: x)").unwrap();
    assert_eq!((c["method"].as_str(), c["route"].as_str()), ("post", "/auth"));
    assert!(caps(r#".{method}("{route}""#, r#"map.get("key")"#).is_none());
    let c = caps("@RequestParam {type} {name}", "@RequestParam  Map<String, Integer> filters,").unwrap();
    assert_eq!((c["type"].as_str(), c["name"].as_str()), ("Map<String, Integer>", "filters"));
}

#[test]
fn first_listed_pattern_wins() {
    let m = Matcher::new(
        &[r#"@RequestParam(..."{name}"...) {type} {_}"#.into(), "@RequestParam(...) {type} {name}".into()],
        &[],
    )
    .unwrap()
    .unwrap();
    let hits: Vec<_> = m
        .hits(r#"@RequestParam(defaultValue = "0") int page, @RequestParam(value = "q") String query"#)
        .map(|h| (h.alt, h.get("name").unwrap().to_string()))
        .collect();
    // `"0"` is not a name, so the second pattern takes `page`; `...` must not run into the next `(...)`.
    assert_eq!(hits, vec![(1, "page".into()), (0, "q".into())]);
}

#[test]
fn string_prefixes() {
    assert_eq!(
        caps(r#".{method}Async("{route}""#, r#"_client.DeleteAsync($"/todos/{id}")"#).unwrap()["route"],
        "/todos/{id}"
    );
    assert_eq!(caps(r#"{_}.{method}("{route}""#, r#"requests.get(f"/users/{id}")"#).unwrap()["route"], "/users/{id}");
    assert!(caps(r#""{path}""#, r#""abc""#).is_some());
}

#[test]
fn keys() {
    assert_eq!(keys_of("{'email': e, 'password': p}"), vec!["email", "password"]);
    assert_eq!(keys_of("<String, int>{'page': 1}"), vec!["page"]);
    assert_eq!(keys_of("{ name, email: mail, ...rest }"), vec!["name", "email", "rest"]);
    assert_eq!(keys_of("['title' => 'required', 'body' => 'x']"), vec!["title", "body"]);
}

#[test]
fn literal_braces_and_optional_parts() {
    let c = caps("{method} {{{name}}}", "GET {id}").unwrap();
    assert_eq!((c["method"].as_str(), c["name"].as_str()), ("GET", "id"));
    let m = Matcher::one(r#"@{method}("{path}"{generic}{name?})"#, &[]).unwrap().unwrap();
    assert_eq!(m.first(r#"@Get("/x")"#).unwrap().get("path"), Some("/x"));
    assert_eq!(m.first(r#"@Get("/x"extra)"#).unwrap().get("name"), Some("extra"));
}

#[test]
fn a_pattern_too_large_to_compile_is_reported() {
    // Patterns become one regular expression; an absurd one hits the regex size limit.
    let huge = "{type} ".repeat(400);
    let err = Matcher::one(&huge, &[]).err().unwrap();
    assert!(err.contains("patterns ["), "{err}");
}

#[test]
fn errors() {
    assert!(Matcher::one("{nope}", &[]).is_err());
    assert!(Matcher::one("{name", &[]).is_err());
}
