//! `zed-http run`: request execution, response handling and chaining, against a local server.

mod support;

use support::{Dir, Server};
use zed_http::cli::run_args;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// Writes `api.http` plus a `dev` profile pointing at the test server.
fn project(dir: &Dir, server: &Server, requests: &str) -> String {
    dir.write(
        "http-client.env.json",
        &format!(
            r#"{{"$shared": {{"ua": "zed-http-test"}}, "dev": {{"baseUrl": "http://127.0.0.1:{}"}}}}"#,
            server.port
        ),
    );
    dir.write("api.http", requests).to_str().unwrap().to_string()
}

#[test]
fn runs_one_request_by_line_name_and_all() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(
        &dir,
        &server,
        "### first\n# @name one\nGET {{baseUrl}}/json\nAccept: application/json\n\n### second\nGET {{baseUrl}}/text\n",
    );

    assert_eq!(run_args(args(&["run", &file, "--line", "3", "--env", "dev"])), 0);
    assert_eq!(server.last().path, "/json");

    assert_eq!(run_args(args(&["run", &file, "--name", "one"])), 0);
    assert_eq!(run_args(args(&["run", &file, "-n", "second"])), 0, "falls back to the ### title");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert_eq!(server.requests().len(), 5);

    // The response is stored for later requests and saved next to the file.
    assert!(dir.exists(".zed-http/responses.json"));
    assert!(dir.exists(".zed-http/one.json"));
}

#[test]
fn selects_a_request_by_surrounding_line() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "### only\nGET {{baseUrl}}/text\n");
    assert_eq!(run_args(args(&["run", &file, "--line", "99"])), 0, "a line past the end picks the last request");
    assert_eq!(run_args(args(&["run", &file])), 2, "no --line/--name/--all");
}

#[test]
fn selection_errors() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "### only\nGET {{baseUrl}}/text\n");
    assert_eq!(run_args(args(&["run", &file, "--name", "nope"])), 2);

    let empty = dir.write("empty.http", "# just a comment\n");
    assert_eq!(run_args(args(&["run", empty.to_str().unwrap(), "--all"])), 2, "no requests in file");
}

#[test]
fn environment_selection() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "GET {{baseUrl}}/text\n");
    assert_eq!(run_args(args(&["run", &file, "--env", "nope"])), 2, "unknown environment");

    // Without --env the `dev` profile is picked automatically.
    assert_eq!(run_args(args(&["run", &file, "--line", "1"])), 0);

    // An unknown variable is reported as a failed request, not a crash.
    let bad = dir.write("bad.http", "GET {{nope}}/x\n");
    assert_eq!(run_args(args(&["run", bad.to_str().unwrap(), "--all"])), 1);
}

#[test]
fn the_file_chooses_its_environment() {
    let dir = Dir::new();
    let server = Server::start();
    dir.write(
        "http-client.env.json",
        &format!(
            r#"{{"dev": {{"baseUrl": "http://127.0.0.1:1"}}, "live": {{"baseUrl": "http://127.0.0.1:{}"}}}}"#,
            server.port
        ),
    );
    let file = dir.write("api.http", "# @env live\n\nGET {{baseUrl}}/text\n");
    let file = file.to_str().unwrap();

    assert_eq!(run_args(args(&["run", file, "--all"])), 0, "`# @env live` reaches the server");
    assert_eq!(server.last().path, "/text");
    assert_eq!(run_args(args(&["run", file, "--all", "--env", "dev"])), 1, "--env wins: nothing listens there");

    let typo = dir.write("typo.http", "# @env lve\nGET {{baseUrl}}/text\n");
    assert_eq!(run_args(args(&["run", typo.to_str().unwrap(), "--all"])), 2, "an unknown environment is an error");
}

#[test]
fn a_request_can_run_in_an_environment_of_its_own() {
    let dir = Dir::new();
    let server = Server::start();
    dir.write(
        "http-client.env.json",
        &format!(
            r#"{{"dev": {{"baseUrl": "http://127.0.0.1:1"}}, "live": {{"baseUrl": "http://127.0.0.1:{}"}}}}"#,
            server.port
        ),
    );
    // The file runs in dev, where nothing listens; one request runs in live.
    let file = dir.write(
        "api.http",
        "# @env dev\n\n### down\nGET {{baseUrl}}/text\n\n### up\n# @env live\nGET {{baseUrl}}/json\n",
    );
    let file = file.to_str().unwrap();
    assert_eq!(run_args(args(&["run", file, "--name", "up"])), 0, "its own environment wins over the file's");
    assert_eq!(server.last().path, "/json");
    assert_eq!(run_args(args(&["run", file, "--name", "down"])), 1, "the file's environment");
    assert_eq!(run_args(args(&["run", file, "--all"])), 1, "each in its own: one fails, one succeeds");
    assert_eq!(server.requests().len(), 2);
    assert_eq!(run_args(args(&["run", file, "--name", "up", "--env", "dev"])), 1, "--env wins over both");

    // An unknown environment anywhere in the run stops it before anything is sent.
    let typo = dir.write("typo.http", "### a\nGET {{baseUrl}}/text\n\n### b\n# @env lve\nGET {{baseUrl}}/json\n");
    assert_eq!(run_args(args(&["run", typo.to_str().unwrap(), "--all"])), 2);
    assert_eq!(server.requests().len(), 2, "nothing was sent");
}

#[test]
fn status_codes_decide_the_exit_code() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "GET {{baseUrl}}/missing\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 1, "4xx fails");

    let file = project(&dir, &server, "GET {{baseUrl}}/boom\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 1, "5xx fails");

    let file = project(&dir, &server, "GET {{baseUrl}}/created\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0, "2xx succeeds");
}

#[test]
fn response_bodies_of_every_kind() {
    let dir = Dir::new();
    let server = Server::start();
    for (path, saved) in [
        ("/json", "line-1.json"),
        ("/text", "line-1.txt"),
        ("/html", "line-1.html"),
        ("/xml", "line-1.xml"),
        ("/yaml", "line-1.yaml"),
        ("/png", "line-1.png"),
        ("/jpeg", "line-1.jpg"),
        ("/binary", "line-1.bin"),
        ("/no-content-type", "line-1.txt"),
    ] {
        let sub = Dir::new();
        let file = project(&sub, &server, &format!("GET {{{{baseUrl}}}}{path}\n"));
        assert_eq!(run_args(args(&["run", &file, "--all"])), 0, "{path}");
        assert!(sub.exists(&format!(".zed-http/{saved}")), "{path} -> {saved}");
    }
    let _ = dir;
}

#[test]
fn raw_flag_and_response_sizes() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "GET {{baseUrl}}/json\n");
    assert_eq!(run_args(args(&["run", &file, "--all", "--raw"])), 0);

    // Sizes are printed in B, KB and MB.
    for n in ["10", "2048", "1200000"] {
        let file = project(&dir, &server, &format!("GET {{{{baseUrl}}}}/size/{n}\n"));
        assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    }
}

#[test]
fn headers_body_and_variables() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(
        &dir,
        &server,
        "@who = world\n\n### post\nPOST {{baseUrl}}/echo\nContent-Type: application/json\nUser-Agent: {{ua}}\nX-Who: {{who}}\n\n{\"hello\": \"{{who}}\"}\n",
    );
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    let last = server.last();
    assert_eq!(last.method, "POST");
    assert_eq!(last.body, "{\"hello\": \"world\"}");
    assert_eq!(last.headers.get("x-who").unwrap(), "world");
    assert_eq!(last.headers.get("user-agent").unwrap(), "zed-http-test", "$shared variable");
}

#[test]
fn body_from_file_and_inline_file_include() {
    let dir = Dir::new();
    let server = Server::start();
    dir.write("payload.json", "{\"from\": \"file\"}");
    let file = project(&dir, &server, "POST {{baseUrl}}/echo\nContent-Type: application/json\n\n< ./payload.json\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert_eq!(server.last().body, "{\"from\": \"file\"}");

    // `< file` inside an inline body is substituted.
    let file = project(
        &dir,
        &server,
        "POST {{baseUrl}}/echo\nContent-Type: multipart/form-data\n\n--boundary\n< ./payload.json\n--boundary--\n",
    );
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert!(server.last().body.contains("{\"from\": \"file\"}"));

    // Missing files are reported.
    let file = project(&dir, &server, "POST {{baseUrl}}/echo\n\n< ./missing.json\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 1);
    let file = project(&dir, &server, "POST {{baseUrl}}/echo\nContent-Type: text/plain\n\nx\n< ./missing.json\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 1);
}

#[test]
fn basic_auth_is_expanded() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "GET {{baseUrl}}/text\nAuthorization: Basic user password\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert_eq!(server.last().headers.get("authorization").unwrap(), "Basic dXNlcjpwYXNzd29yZA==");

    let file = project(&dir, &server, "GET {{baseUrl}}/text\nAuthorization: Basic user:password\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert_eq!(server.last().headers.get("authorization").unwrap(), "Basic dXNlcjpwYXNzd29yZA==");

    let file = project(&dir, &server, "GET {{baseUrl}}/text\nAuthorization: Bearer token-value\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert_eq!(server.last().headers.get("authorization").unwrap(), "Bearer token-value");
}

#[test]
fn redirects_can_be_followed_or_disabled() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "GET {{baseUrl}}/moved\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert_eq!(server.last().path, "/json", "redirect followed");

    let file = project(&dir, &server, "# @no-redirect\nGET {{baseUrl}}/moved\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert_eq!(server.last().path, "/moved", "redirect not followed");
}

#[test]
fn response_can_be_written_to_a_file() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "GET {{baseUrl}}/json\n\n>> ./out/report.json\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert!(dir.exists("out/report.json"));

    // A second run keeps the first file and adds report-1.json.
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert!(dir.exists("out/report-1.json"));

    // `>>!` overwrites.
    let file = project(&dir, &server, "GET {{baseUrl}}/text\n\n>>! ./out/forced.txt\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert_eq!(dir.read("out/forced.txt"), "plain text");
    assert!(!dir.exists("out/forced-1.txt"));

    // An unwritable target is an error.
    dir.write("blocked", "file");
    let file = project(&dir, &server, "GET {{baseUrl}}/text\n\n>>! ./blocked/deep/out.txt\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 1);
}

#[test]
fn request_chaining_uses_stored_responses() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(
        &dir,
        &server,
        "### login\n# @name login\nGET {{baseUrl}}/json\n\n### use it\nPOST {{baseUrl}}/echo\nContent-Type: application/json\nX-Id: {{login.response.body.$.id}}\nX-Status: {{login.response.status}}\nX-Demo: {{login.response.headers.x-demo}}\n\n{\"nested\": \"{{login.response.body.nested.ok}}\", \"item\": \"{{login.response.body.items[0].id}}\"}\n",
    );
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    let last = server.last();
    assert_eq!(last.headers.get("x-id").unwrap(), "1");
    assert_eq!(last.headers.get("x-status").unwrap(), "200");
    assert_eq!(last.headers.get("x-demo").unwrap(), "demo-value");
    assert_eq!(last.body, "{\"nested\": \"true\", \"item\": \"7\"}");

    // Stored responses survive into a later process run.
    let file2 = project(&dir, &server, "GET {{baseUrl}}/echo?id={{login.response.body.name}}\n");
    assert_eq!(run_args(args(&["run", &file2, "--all"])), 0);
    assert_eq!(server.last().path, "/echo?id=demo");

    // A reference to a request that never ran fails.
    let file3 = project(&dir, &server, "GET {{baseUrl}}/echo?x={{nothing.response.body.id}}\n");
    assert_eq!(run_args(args(&["run", &file3, "--all"])), 1);
}

#[test]
fn a_corrupt_response_store_is_ignored() {
    let dir = Dir::new();
    let server = Server::start();
    dir.write(".zed-http/responses.json", "{ not json");
    let file = project(&dir, &server, "GET {{baseUrl}}/text\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
}

#[test]
fn transport_errors_and_timeouts() {
    let dir = Dir::new();
    let server = Server::start();

    // Nothing is listening on this port.
    dir.write("http-client.env.json", r#"{"dev": {"baseUrl": "http://127.0.0.1:1"}}"#);
    let file = dir.write("refused.http", "GET {{baseUrl}}/x\n");
    assert_eq!(run_args(args(&["run", file.to_str().unwrap(), "--all"])), 1);

    let dir2 = Dir::new();
    let file = project(&dir2, &server, "GET {{baseUrl}}/slow\n");
    assert_eq!(run_args(args(&["run", &file, "--all", "--timeout", "1"])), 1, "times out");
}

#[test]
fn url_without_a_scheme_gets_http() {
    let dir = Dir::new();
    let server = Server::start();
    let file = dir.write("api.http", &format!("GET 127.0.0.1:{}/text\n", server.port));
    assert_eq!(run_args(args(&["run", file.to_str().unwrap(), "--all"])), 0);
    assert_eq!(server.last().path, "/text");
}

#[test]
fn a_request_name_is_sanitized_for_the_saved_file() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "# @name weird name/with:chars\nGET {{baseUrl}}/text\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    assert!(dir.exists(".zed-http/weird_name_with_chars.txt"));
}

#[test]
fn runs_a_file_in_the_current_directory() {
    // Exercises the "no parent directory" path of `run`.
    let dir = Dir::new();
    let server = Server::start();
    project(&dir, &server, "GET {{baseUrl}}/text\n");
    let guard = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();
    let code = run_args(args(&["run", "api.http", "--all"]));
    std::env::set_current_dir(guard).unwrap();
    assert_eq!(code, 0);
}

#[test]
fn saving_the_response_is_best_effort() {
    let dir = Dir::new();
    let server = Server::start();
    // `.zed-http` is a file, so the response directory cannot be created.
    dir.write(".zed-http", "not a directory");
    let file = project(&dir, &server, "GET {{baseUrl}}/text\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0, "the request still succeeds");

    // The directory exists but the target name is taken by a directory.
    let dir = Dir::new();
    std::fs::create_dir_all(dir.join(".zed-http/line-1.txt")).unwrap();
    let file = project(&dir, &server, "GET {{baseUrl}}/text\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
}

#[cfg(unix)]
#[test]
fn a_redirect_target_without_a_parent_fails() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "GET {{baseUrl}}/text\n\n>>! /\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 1, "the root directory is not a file");
}

#[test]
fn a_truncated_response_body_is_an_error() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "GET {{baseUrl}}/truncated\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 1);
}

#[test]
fn a_line_before_the_first_request_has_no_match() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "@a = 1\n@b = 2\n\n### only\nGET {{baseUrl}}/text\n");
    assert_eq!(run_args(args(&["run", &file, "--line", "1"])), 2, "no request starts at or before line 1");
}

#[test]
fn an_unreadable_environment_file_is_reported() {
    let dir = Dir::new();
    std::fs::create_dir_all(dir.join("http-client.env.json")).unwrap();
    let file = dir.write("api.http", "GET http://127.0.0.1:1/x\n");
    assert_eq!(run_args(args(&["run", file.to_str().unwrap(), "--all"])), 2);
}

#[test]
fn no_log_keeps_the_response_off_disk() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(
        &dir,
        &server,
        "### login\n# @name login\n# @no-log\nGET {{baseUrl}}/json\n\n### use it\nGET {{baseUrl}}/echo?id={{login.response.body.$.id}}\n",
    );
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    // Later requests of the same run still see the response.
    assert_eq!(server.last().path, "/echo?id=1");
    let out = std::path::Path::new(&file).parent().unwrap().join(".zed-http");
    assert!(!out.join("login.json").exists(), "no saved response file");
    let store = std::fs::read_to_string(out.join("responses.json")).unwrap_or_default();
    assert!(!store.contains("\"login\""), "not kept for later runs: {store}");
}

#[test]
fn only_no_log_requests_write_nothing() {
    let dir = Dir::new();
    let server = Server::start();
    let file = project(&dir, &server, "# @no-log\nGET {{baseUrl}}/json\n");
    assert_eq!(run_args(args(&["run", &file, "--all"])), 0);
    let out = std::path::Path::new(&file).parent().unwrap().join(".zed-http");
    assert!(!out.exists(), "no .zed-http/ at all");
}
