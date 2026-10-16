//! `zed-http list | envs | detect | frameworks | new` and argument handling.

mod support;

use support::Dir;
use zed_http::cli::run_args;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

const SAMPLE: &str = "\
@token = demo

### first
# @name ip
GET {{baseUrl}}/get
Accept: application/json

### second
POST {{baseUrl}}/post
Content-Type: application/json

{\"a\": 1}
";

#[test]
fn list_and_envs() {
    let dir = Dir::new();
    let file = dir.write("api.http", SAMPLE);
    dir.write(
        "http-client.env.json",
        r#"{"$shared": {"apiVersion": "v1"}, "dev": {"baseUrl": "http://localhost"}, "prod": {"baseUrl": "https://x"}}"#,
    );
    assert_eq!(run_args(args(&["list", file.to_str().unwrap()])), 0);
    assert_eq!(run_args(args(&["envs", file.to_str().unwrap()])), 0);
}

#[test]
fn unknown_command_and_missing_file() {
    let dir = Dir::new();
    let file = dir.write("api.http", SAMPLE);
    assert_eq!(run_args(args(&["bogus", file.to_str().unwrap()])), 2);
    assert_eq!(run_args(args(&["list", dir.join("nope.http").to_str().unwrap()])), 2);
}

#[test]
fn usage_errors() {
    assert_eq!(run_args(args(&["--help"])), 2);
    assert_eq!(run_args(args(&["help"])), 2);
    assert_eq!(run_args(args(&["-h"])), 2);
    assert_eq!(run_args(vec![]), 2);
    assert_eq!(run_args(args(&["list"])), 2, "missing file");
    assert_eq!(run_args(args(&["list", "--bogus", "x.http"])), 2);
    assert_eq!(run_args(args(&["run", "x.http", "--line"])), 2, "flag without a value");
    assert_eq!(run_args(args(&["run", "x.http", "--line", "abc"])), 2);
    assert_eq!(run_args(args(&["run", "x.http", "--timeout", "abc"])), 2);
}

#[test]
fn detect_prints_routes() {
    let dir = Dir::new();
    let src =
        dir.write("server.ts", "import express from 'express';\napp.get('/users/:id', h);\napp.post('/users', h);\n");
    let path = src.to_str().unwrap();
    assert_eq!(run_args(args(&["detect", path])), 0);
    assert_eq!(run_args(args(&["detect", path, "--line", "2"])), 0);
    assert_eq!(run_args(args(&["detect", path, "-l", "1"])), 1, "no route on that line");

    let plain = dir.write("notes.txt", "nothing here\n");
    assert_eq!(run_args(args(&["detect", plain.to_str().unwrap()])), 1);
    assert_eq!(run_args(args(&["detect"])), 2, "missing file");
    assert_eq!(run_args(args(&["detect", path, "--line", "abc"])), 2);
    assert_eq!(run_args(args(&["detect", path, "--zzz"])), 2);
    assert_eq!(run_args(args(&["detect", dir.join("gone.ts").to_str().unwrap()])), 2);
}

#[test]
fn frameworks_lists_builtin_and_project_catalog() {
    let dir = Dir::new();
    assert_eq!(run_args(args(&["frameworks", dir.path().to_str().unwrap()])), 0);

    dir.write(
        ".zed/http/frameworks.json",
        r#"{"languages": {"dart": {"frameworks": {"mine": {"name": "Mine", "routes": ["api.call(\"{method}\", \"{route}\""]}}}}}"#,
    );
    assert_eq!(run_args(args(&["frameworks", dir.path().to_str().unwrap()])), 0);

    dir.write(".zed/http/frameworks.json", "{ not json");
    assert_eq!(run_args(args(&["frameworks", dir.path().to_str().unwrap()])), 2);
}

#[test]
fn detect_uses_project_catalog() {
    let dir = Dir::new();
    dir.write(
        ".zed/http/frameworks.json",
        r#"{"languages": {"dart": {"frameworks": {"mine": {"name": "Mine", "imports": ["my_client"], "routes": ["api.call(\"{method}\", \"{route}\""]}}}}}"#,
    );
    let src = dir.write("lib/api.dart", "import 'my_client.dart';\napi.call('POST', '/x');\n");
    assert_eq!(run_args(args(&["detect", src.to_str().unwrap()])), 0);
}

#[test]
fn new_endpoint_from_flags() {
    let dir = Dir::new();
    let root = dir.path().to_str().unwrap().to_string();
    assert_eq!(
        run_args(args(&["new", &root, "-m", "post", "-u", "{{baseUrl}}/users", "-n", "Create User", "--no-open"])),
        0
    );
    let file = dir.read(".zed/http/endpoints/create-user.http");
    assert!(file.contains("POST {{baseUrl}}/users"), "{file}");
    assert!(file.contains("### create-user"), "{file}");
    assert!(dir.exists(".zed/http/http-client.env.json"), "a default profile is created");
    assert!(!dir.exists(".zed/http/.gitignore"), "no .gitignore is generated");

    // A second endpoint with the same name goes to <name>-1.http.
    assert_eq!(
        run_args(args(&["new", &root, "-m", "GET", "-u", "{{baseUrl}}/users", "-n", "Create User", "--no-open"])),
        0
    );
    assert!(dir.exists(".zed/http/endpoints/create-user-1.http"));
}

#[test]
fn new_endpoint_defaults_without_a_terminal() {
    let dir = Dir::new();
    // No -m/-u/-n: the prompts fall back to defaults because tests have no TTY.
    assert_eq!(run_args(args(&["new", dir.path().to_str().unwrap(), "--no-open"])), 0);
    let file = dir.read(".zed/http/endpoints/get.http");
    assert!(file.contains("GET {{baseUrl}}/"), "{file}");
}

#[test]
fn new_endpoint_from_source() {
    let dir = Dir::new();
    let root = dir.path().to_str().unwrap().to_string();
    let src = dir.write(
        "server.ts",
        "import express from 'express';\napp.post('/users', (req, res) => {\n  const { name } = req.body;\n});\n",
    );
    let from = format!("{}:2", src.to_str().unwrap());
    assert_eq!(run_args(args(&["new", &root, "--from", &from, "--no-open"])), 0);
    let file = dir.read(".zed/http/endpoints/server.ts.http");
    assert!(file.contains("POST {{baseUrl}}/users"), "{file}");
    assert!(file.contains("\"name\""), "the body is inferred: {file}");

    // Same endpoint again: reported as existing, not appended twice.
    assert_eq!(run_args(args(&["new", &root, "--from", &from, "--no-open"])), 0);
    assert_eq!(dir.read(".zed/http/endpoints/server.ts.http").matches("@name post-users").count(), 1);

    // A different endpoint from the same file is appended.
    let src2 =
        dir.write("server.ts", "import express from 'express';\napp.post('/users', h);\napp.get('/health', h);\n");
    let from2 = format!("{}:3", src2.to_str().unwrap());
    assert_eq!(run_args(args(&["new", &root, "--from", &from2, "--no-open"])), 0);
    let file = dir.read(".zed/http/endpoints/server.ts.http");
    assert!(file.contains("GET {{baseUrl}}/health"), "{file}");

    // A custom name replaces the generated one.
    let from3 = format!("{}:2", src2.to_str().unwrap());
    assert_eq!(run_args(args(&["new", &root, "--from", &from3, "-n", "signup", "--no-open"])), 0);
    assert!(dir.read(".zed/http/endpoints/server.ts.http").contains("@name signup"));
}

#[test]
fn new_endpoint_errors() {
    let dir = Dir::new();
    let root = dir.path().to_str().unwrap().to_string();
    let src = dir.write("server.ts", "import express from 'express';\napp.get('/x', h);\n");
    let path = src.to_str().unwrap().to_string();

    assert_eq!(run_args(args(&["new", &root, "--zzz"])), 2, "unknown option");
    assert_eq!(run_args(args(&["new", &root, "--name"])), 2, "flag without a value");
    assert_eq!(run_args(args(&["new", &root, "--from", "no-colon"])), 2);
    assert_eq!(run_args(args(&["new", &root, "--from", &format!("{path}:abc")])), 2);
    assert_eq!(run_args(args(&["new", &root, "--from", "missing.ts:1"])), 2);
    assert_eq!(run_args(args(&["new", &root, "--from", &format!("{path}:1")])), 2, "no route on line 1");
}

#[test]
fn new_endpoint_keeps_an_existing_profile() {
    let dir = Dir::new();
    dir.write(".zed/http/http-client.env.json", r#"{"dev": {"baseUrl": "http://example.test"}}"#);
    assert_eq!(run_args(args(&["new", dir.path().to_str().unwrap(), "-m", "GET", "-u", "/x", "--no-open"])), 0);
    assert_eq!(
        dir.read(".zed/http/http-client.env.json"),
        r#"{"dev": {"baseUrl": "http://example.test"}}"#,
        "the existing profile is left alone"
    );
}

#[test]
fn new_endpoint_with_an_unwritable_root_fails() {
    let dir = Dir::new();
    // A file where the directory should go: create_dir_all must fail.
    dir.write(".zed", "not a directory");
    assert_eq!(run_args(args(&["new", dir.path().to_str().unwrap(), "-m", "GET", "-u", "/x", "--no-open"])), 2);
}

#[test]
fn new_endpoint_with_an_unnameable_endpoint_falls_back() {
    let dir = Dir::new();
    assert_eq!(
        run_args(args(&["new", dir.path().to_str().unwrap(), "-m", "GET", "-u", "/x", "-n", "???", "--no-open"])),
        0
    );
    assert!(dir.exists(".zed/http/endpoints/endpoint.http"), "an empty name falls back to `endpoint`");
}

#[cfg(unix)]
#[test]
fn creating_the_default_profile_can_fail() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Dir::new();
    // The directories already exist but `.zed/http` is read-only, so the profile cannot be written.
    std::fs::create_dir_all(dir.join(".zed/http/endpoints")).unwrap();
    std::fs::set_permissions(dir.join(".zed/http"), std::fs::Permissions::from_mode(0o555)).unwrap();
    let code = run_args(args(&["new", dir.path().to_str().unwrap(), "-m", "GET", "-u", "/x", "--no-open"]));
    std::fs::set_permissions(dir.join(".zed/http"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(code, 2);
}

#[test]
fn new_endpoint_reports_write_failures() {
    // The per-source request file is taken by a directory (`--from` does not pick a free name).
    let dir = Dir::new();
    let src = dir.write("server.ts", "import express from 'express';\napp.get('/x', h);\n");
    std::fs::create_dir_all(dir.join(".zed/http/endpoints/server.ts.http")).unwrap();
    let from = format!("{}:2", src.to_str().unwrap());
    assert_eq!(run_args(args(&["new", dir.path().to_str().unwrap(), "--from", &from, "--no-open"])), 2);
}
