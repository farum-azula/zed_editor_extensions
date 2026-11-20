//! The tree-sitter queries of `languages/http/`, run against the grammar `extension.toml` pins.
//! Zed drops a query file that does not compile without a word, so every query must compile; the
//! highlights must also colour `.http` files the way they are meant to.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator, Tree};

fn extension_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn language() -> Language {
    http_grammar::LANGUAGE.into()
}

fn query(file: &str) -> Query {
    let path = extension_dir().join("languages/http").join(file);
    let source = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    Query::new(&language(), &source).unwrap_or_else(|error| panic!("{file}: {error}"))
}

fn parse(text: &str) -> Tree {
    let mut parser = Parser::new();
    parser.set_language(&language()).expect("a compatible grammar");
    parser.parse(text, None).expect("a parse tree")
}

/// Every `.http` file of `examples/`.
fn example_files() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut directories = vec![extension_dir().join("examples")];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).expect("examples/").flatten() {
            let path = entry.path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "http") {
                let text = fs::read_to_string(&path).expect("an example");
                found.push((path, text));
            }
        }
    }
    assert!(!found.is_empty(), "no .http files in examples/");
    found
}

/// Every construct the highlights have a pattern for.
const ALL_CONSTRUCTS: &str = r#"@token = demo-token

### Create a user
# @name created
# @no-redirect
# @env dev
# @timeout=5
POST {{baseUrl}}/users?page={{$randomInt}} HTTP/1.1
Authorization: Bearer {{token}}
X-Request-Id: {{$uuid}}

< ./user.json

> {%
  client.global.set("id", response.body.id);
%}

>> ./out/user.json

### Run scripts
< ./before.js
GET https://example.com/health

> ./handler.js

### A response
HTTP/1.1 200 OK
Content-Type: application/json
"#;

/// The capture Zed colours each byte of `text` with. Zed pushes captures in the order tree-sitter
/// returns them (by start, then by pattern) and pops those that have ended: the top one wins.
fn highlights(text: &str) -> Vec<Option<String>> {
    let query = query("highlights.scm");
    let tree = parse(text);
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&query, tree.root_node(), text.as_bytes());
    let mut ordered: Vec<(usize, usize, String)> = Vec::new();
    while let Some((found, index)) = captures.next() {
        let capture = found.captures[*index];
        let name = query.capture_names()[capture.index as usize].to_string();
        ordered.push((capture.node.start_byte(), capture.node.end_byte(), name));
    }
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut next = 0;
    let mut colours = vec![None; text.len()];
    for (byte, colour) in colours.iter_mut().enumerate() {
        while stack.last().is_some_and(|(end, _)| *end <= byte) {
            stack.pop();
        }
        while next < ordered.len() && ordered[next].0 <= byte {
            stack.push((ordered[next].1, ordered[next].2.clone()));
            next += 1;
        }
        *colour = stack.last().map(|(_, name)| name.clone());
    }
    colours
}

/// The colour of the first `token` after the first `context` in `text`, the same for all of it.
fn colour(text: &str, colours: &[Option<String>], context: &str, token: &str) -> String {
    let after = text.find(context).unwrap_or_else(|| panic!("no `{context}` in the text"));
    let start = after + text[after..].find(token).unwrap_or_else(|| panic!("no `{token}` after `{context}`"));
    let span = &colours[start..start + token.len()];
    let first = span[0].clone().unwrap_or_else(|| panic!("`{token}` after `{context}` is not coloured"));
    assert!(span.iter().all(|c| c.as_deref() == Some(first.as_str())), "`{token}` is coloured unevenly: {span:?}");
    first
}

#[test]
fn every_query_compiles_against_the_pinned_grammar() {
    let directory = extension_dir().join("languages/http");
    let mut compiled = Vec::new();
    for entry in fs::read_dir(&directory).expect("languages/http").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".scm") {
            query(&name);
            compiled.push(name);
        }
    }
    compiled.sort();
    assert_eq!(compiled, ["highlights.scm", "injections.scm", "outline.scm", "runnables.scm"]);
}

#[test]
fn the_examples_parse_without_errors() {
    for (path, text) in example_files() {
        assert!(!parse(&text).root_node().has_error(), "{} does not parse", path.display());
    }
    assert!(!parse(ALL_CONSTRUCTS).root_node().has_error(), "{}", parse(ALL_CONSTRUCTS).root_node().to_sexp());
}

/// A pattern that never matches is as good as missing: its node exists but sits elsewhere.
#[test]
fn every_highlight_pattern_matches() {
    let query = query("highlights.scm");
    let mut matched = BTreeSet::new();
    let texts: Vec<String> =
        example_files().into_iter().map(|(_, text)| text).chain([ALL_CONSTRUCTS.to_string()]).collect();
    for text in &texts {
        let tree = parse(text);
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), text.as_bytes());
        while let Some(found) = matches.next() {
            matched.insert(found.pattern_index);
        }
    }
    let unmatched: Vec<usize> = (0..query.pattern_count()).filter(|index| !matched.contains(index)).collect();
    let starts: Vec<usize> = unmatched.iter().map(|&index| query.start_byte_for_pattern(index)).collect();
    assert!(unmatched.is_empty(), "patterns at bytes {starts:?} of highlights.scm match nothing");
}

/// Theme keys Zed knows; a capture named otherwise is never coloured.
#[test]
fn captures_are_named_after_theme_keys() {
    let roots = [
        "attribute",
        "comment",
        "constant",
        "function",
        "keyword",
        "label",
        "link_uri",
        "number",
        "operator",
        "property",
        "punctuation",
        "string",
        "title",
        "type",
        "variable",
    ];
    let query = query("highlights.scm");
    for name in query.capture_names() {
        let root = name.split('.').next().unwrap_or_default();
        assert!(roots.contains(&root), "@{name} is not a capture Zed themes colour");
    }
}

#[test]
fn highlights_tell_the_parts_of_a_request_apart() {
    let text = ALL_CONSTRUCTS;
    let colours = highlights(text);
    let of = |context: &str, token: &str| colour(text, &colours, context, token);

    assert_eq!(of("", "POST"), "function.method");
    assert_eq!(of("POST ", "/users?page="), "link_uri");
    assert_eq!(of("POST ", "baseUrl"), "variable.special", "a user variable inside the URL");
    assert_eq!(of("page=", "$randomInt"), "variable.builtin", "a built-in variable");
    assert_eq!(of("POST ", "{{"), "punctuation.special");
    assert_eq!(of("baseUrl", "}}"), "punctuation.special");
    assert_eq!(of("users?", "HTTP/1.1"), "constant");

    assert_eq!(of("", "Authorization"), "property");
    assert_eq!(of("Authorization", ":"), "punctuation.delimiter");
    assert_eq!(of("Authorization", "Bearer"), "string");
    assert_eq!(of("Bearer ", "token"), "variable.special");
    assert_eq!(of("X-Request-Id: ", "$uuid"), "variable.builtin");

    assert_eq!(of("", "@"), "punctuation.special", "the `@` of a file variable");
    assert_eq!(of("@", "token"), "variable");
    assert_eq!(of("@token ", "="), "operator");
    assert_eq!(of("@token = ", "demo-token"), "string");

    assert_eq!(of("### Create", "###"), "comment");
    assert_eq!(of("### ", "Create a user"), "title");
    assert_eq!(of("# @name", "@name"), "attribute");
    assert_eq!(of("# @name ", "created"), "label", "a request's name");
    assert_eq!(of("# ", "@no-redirect"), "attribute");
    assert_eq!(of("# @env ", "dev"), "string.special", "a directive's value");
    assert_eq!(of("# @timeout", "="), "operator");
    assert_eq!(of("# @timeout=", "5"), "string.special");

    assert_eq!(of("< ", "./user.json"), "string.special");
    assert_eq!(of("> ", "{%"), "punctuation.special");
    assert_eq!(of("client.global", "%}"), "punctuation.special");
    assert_eq!(of(">> ", "./out/user.json"), "string.special");
    assert_eq!(of("< ", "./before.js"), "string.special");
    assert_eq!(of("> ", "./handler.js"), "string.special");

    assert_eq!(of("### A response\n", "HTTP/1.1"), "constant");
    assert_eq!(of("HTTP/1.1 ", "200"), "number");
    assert_eq!(of("200 ", "OK"), "string");
}

/// The ▶ of every request in the example: one run button per request line.
#[test]
fn every_example_request_gets_a_run_button() {
    let query = query("runnables.scm");
    let run = query.capture_index_for_name("run").expect("a @run capture");
    for (path, text) in example_files() {
        let tree = parse(&text);
        let mut cursor = QueryCursor::new();
        let mut captures = cursor.captures(&query, tree.root_node(), text.as_bytes());
        let mut lines = BTreeSet::new();
        while let Some((found, index)) = captures.next() {
            let capture = found.captures[*index];
            if capture.index == run {
                lines.insert(capture.node.start_position().row);
            }
        }
        let requests = zed_http::parser::parse(&text).requests.len();
        assert_eq!(lines.len(), requests, "{}: run buttons on lines {lines:?}", path.display());
    }
}
