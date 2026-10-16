//! The installed `zed-http` binary: the thin wrapper around the library.

mod support;

use std::process::Command;
use support::Dir;

const BIN: &str = env!("CARGO_BIN_EXE_zed-http");

#[test]
fn binary_runs_commands_and_reports_exit_codes() {
    let dir = Dir::new();
    let src = dir.write("server.ts", "import express from 'express';\napp.get('/x', h);\n");

    let out = Command::new(BIN).arg("detect").arg(&src).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("GET\t{{baseUrl}}/x\ttypescript/express"), "{stdout}");

    let out = Command::new(BIN).arg("--help").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("USAGE"));

    // `new` with no root argument uses the current directory.
    let out = Command::new(BIN)
        .args(["new", "-m", "GET", "-u", "{{baseUrl}}/here", "-n", "here", "--no-open"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.exists(".zed/http/endpoints/here.http"));

    // `frameworks` with no root argument also uses the current directory.
    let out = Command::new(BIN).arg("frameworks").current_dir(dir.path()).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Dart / Flutter"));
}

#[test]
fn binary_speaks_the_language_server_protocol() {
    use std::io::Write;
    let dir = Dir::new();
    let src = dir.write("server.ts", "import express from 'express';\napp.get('/users/:id', h);\n");
    let uri = format!("file://{}", src.display());
    let messages = [
        format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"rootUri":"file://{}"}}}}"#,
            dir.path().display()
        ),
        format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","text":"import express from 'express';\napp.get('/users/:id', h);\n"}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/codeLens","params":{{"textDocument":{{"uri":"{uri}"}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"workspace/executeCommand","params":{{"command":"zed-http.generate","arguments":["{uri}",1]}}}}"#
        ),
        r#"{"jsonrpc":"2.0","id":4,"method":"shutdown"}"#.to_string(),
        r#"{"jsonrpc":"2.0","method":"exit"}"#.to_string(),
    ];
    let mut child = Command::new(BIN)
        .arg("lsp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = child.stdin.as_mut().unwrap();
    for body in &messages {
        write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"serverInfo\""));
    assert!(stdout.contains("🌐 Generate .http request"), "{stdout}");
    assert!(stdout.contains("workspace/applyEdit"), "{stdout}");
}

#[test]
fn new_tries_to_open_the_result_in_zed() {
    let dir = Dir::new();
    let empty = dir.join("empty-path");
    std::fs::create_dir_all(&empty).unwrap();
    // Without --no-open the runner asks the `zed` CLI to open the file; with an empty PATH
    // there is none, which must not fail the command.
    let out = Command::new(BIN)
        .args(["new", dir.path().to_str().unwrap(), "-m", "GET", "-u", "{{baseUrl}}/opened", "-n", "opened"])
        .env("PATH", &empty)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.exists(".zed/http/endpoints/opened.http"));
}
