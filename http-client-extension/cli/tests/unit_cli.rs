//! Unit tests for `cli.rs`.

use serde_json::Value;
use std::fs;
use std::path::Path;
use zed_http::cli::*;

#[test]
fn basic_auth_shorthand() {
    assert_eq!(expand_basic_auth("Basic user pass"), "Basic dXNlcjpwYXNz");
    assert_eq!(expand_basic_auth("Basic user:pass"), "Basic dXNlcjpwYXNz");
    assert_eq!(expand_basic_auth("Basic dXNlcjpwYXNz="), "Basic dXNlcjpwYXNz=", "already encoded");
    assert_eq!(expand_basic_auth("Basic a b c"), "Basic a b c", "not a credential pair");
    assert_eq!(expand_basic_auth("Basic token"), "Basic token", "no colon");
    assert_eq!(expand_basic_auth("Bearer t"), "Bearer t");
}

#[test]
fn content_types_and_sizes() {
    assert!(is_textual("text/csv") && is_textual("application/json") && is_textual(""));
    assert!(!is_textual("application/octet-stream"));
    assert_eq!(extension_for("application/json", true), "json");
    assert_eq!(extension_for("text/html", false), "html");
    assert_eq!(extension_for("application/xml", false), "xml");
    assert_eq!(extension_for("application/yaml", false), "yaml");
    assert_eq!(extension_for("image/png", false), "png");
    assert_eq!(extension_for("image/jpeg", false), "jpg");
    assert_eq!(extension_for("text/plain", false), "txt");
    assert_eq!(extension_for("application/octet-stream", false), "bin");
    assert_eq!(human_size(512), "512 B");
    assert_eq!(human_size(2048), "2.0 KB");
    assert_eq!(human_size(3 * 1024 * 1024), "3.0 MB");
    assert_eq!(sanitize("a b/c:d-e_f"), "a_b_c_d-e_f");
}

/// A sink that fails on write, on flush, or neither.
struct Sink {
    fail_write: bool,
    fail_flush: bool,
}

impl std::io::Write for Sink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.fail_write {
            return Err(std::io::Error::other("write failed"));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.fail_flush {
            return Err(std::io::Error::other("flush failed"));
        }
        Ok(())
    }
}

/// A reader that always fails.
struct Unreadable;

impl std::io::Read for Unreadable {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("read failed"))
    }
}

impl std::io::BufRead for Unreadable {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        Err(std::io::Error::other("read failed"))
    }

    fn consume(&mut self, _: usize) {}
}

#[test]
fn prompts_report_io_failures() {
    let mut input = std::io::Cursor::new(b"x\n".to_vec());
    let mut sink = Sink { fail_write: true, fail_flush: false };
    assert_eq!(prompt_from("Method", "GET", true, &mut input, &mut sink).unwrap_err(), "write failed");

    let mut sink = Sink { fail_write: false, fail_flush: true };
    assert_eq!(prompt_from("Method", "GET", true, &mut input, &mut sink).unwrap_err(), "flush failed");

    let mut sink = Sink { fail_write: false, fail_flush: false };
    assert_eq!(prompt_from("Method", "GET", true, &mut Unreadable, &mut sink).unwrap_err(), "read failed");
}

#[test]
fn prompts_fall_back_to_the_default_without_a_terminal() {
    let mut out = Vec::new();
    let answer = prompt_from("Method", "GET", false, &mut std::io::Cursor::new(b"POST\n".to_vec()), &mut out);
    assert_eq!(answer.unwrap(), "GET", "input is not read without a terminal");
    assert!(out.is_empty(), "nothing is printed");

    let mut out = Vec::new();
    let answer = prompt_from("Method", "GET", true, &mut std::io::Cursor::new(b"post\n".to_vec()), &mut out);
    assert_eq!(answer.unwrap(), "post");
    assert!(String::from_utf8(out).unwrap().contains("Method"), "the label is shown");

    let mut out = Vec::new();
    let answer = prompt_from("Method", "GET", true, &mut std::io::Cursor::new(b"\n".to_vec()), &mut out);
    assert_eq!(answer.unwrap(), "GET", "an empty answer keeps the default");
}

#[test]
fn unique_paths_do_not_overwrite() {
    let dir = std::env::temp_dir().join(format!("zed-http-unique-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let taken = dir.join("out.json");
    fs::write(&taken, "{}").unwrap();
    assert_eq!(unique_path(&taken), dir.join("out-1.json"));
    fs::write(dir.join("out-1.json"), "{}").unwrap();
    assert_eq!(unique_path(&taken), dir.join("out-2.json"));
    let no_ext = dir.join("report");
    fs::write(&no_ext, "x").unwrap();
    assert_eq!(unique_path(&no_ext), dir.join("report-1"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn store_paths_without_a_parent() {
    // `Path::new("")` has no parent directory; writing simply fails and is ignored.
    save_store(Path::new(""), &serde_json::Map::new());
    assert!(load_store(Path::new("")).is_empty());
}

#[test]
fn response_store_survives_bad_input() {
    let dir = std::env::temp_dir().join(format!("zed-http-store-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("nested").join("responses.json");
    assert!(load_store(&path).is_empty(), "missing file");
    let mut store = serde_json::Map::new();
    store.insert("one".into(), Value::from(1));
    save_store(&path, &store);
    assert_eq!(load_store(&path), store);
    fs::write(&path, "{ broken").unwrap();
    assert!(load_store(&path).is_empty(), "unparsable file");
    let _ = fs::remove_dir_all(&dir);
}
