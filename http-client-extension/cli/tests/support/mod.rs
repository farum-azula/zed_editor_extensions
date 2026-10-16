//! Test helpers: a scratch directory and a tiny local HTTP server.
//!
//! Shared by every test binary; each one uses a subset.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fs, thread};

/// A unique directory under the system temp dir, removed when the value is dropped.
pub struct Dir(PathBuf);

impl Dir {
    pub fn new() -> Dir {
        static N: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "zed-http-test-{}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Dir(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    /// Writes a file, creating parent directories.
    pub fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let path = self.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }

    pub fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.join(rel)).unwrap_or_default()
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.join(rel).exists()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// A local HTTP server with a fixed set of routes, used to exercise the runner.
pub struct Server {
    pub port: u16,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl Server {
    pub fn start() -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let log = log.clone();
                thread::spawn(move || handle(stream, log));
            }
        });
        Server { port, requests }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }

    pub fn last(&self) -> Recorded {
        self.requests().last().cloned().expect("a request was made")
    }
}

fn handle(mut stream: TcpStream, log: Arc<Mutex<Vec<Recorded>>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut start = String::new();
    if reader.read_line(&mut start).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = start.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();

    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_lowercase(), v.trim().to_string());
        }
    }
    let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut body = vec![0; len];
    if len > 0 {
        let _ = reader.read_exact(&mut body);
    }
    let body = String::from_utf8_lossy(&body).to_string();
    log.lock().unwrap().push(Recorded { method, path: path.clone(), headers, body: body.clone() });

    let route = path.split('?').next().unwrap_or_default();
    let (status, content_type, payload): (u16, Option<&str>, Vec<u8>) = match route {
        "/json" => (
            200,
            Some("application/json"),
            br#"{"id":1,"name":"demo","items":[{"id":7}],"nested":{"ok":true}}"#.to_vec(),
        ),
        "/echo" => (
            200,
            Some("application/json"),
            format!(r#"{{"body":{}}}"#, if body.is_empty() { "null".into() } else { body.clone() }).into_bytes(),
        ),
        "/text" => (200, Some("text/plain"), b"plain text".to_vec()),
        "/html" => (200, Some("text/html"), b"<html></html>".to_vec()),
        "/xml" => (200, Some("application/xml"), b"<root/>".to_vec()),
        "/yaml" => (200, Some("application/yaml"), b"a: 1".to_vec()),
        "/png" => (200, Some("image/png"), vec![0x89, b'P', b'N', b'G', 0, 1, 2, 3]),
        "/jpeg" => (200, Some("image/jpeg"), vec![0xff, 0xd8, 0xff, 0xe0]),
        "/binary" => (200, Some("application/octet-stream"), vec![0u8, 1, 2, 3, 4, 5]),
        "/no-content-type" => (200, None, b"body without a type".to_vec()),
        "/created" => (201, Some("application/json"), br#"{"created":true}"#.to_vec()),
        "/moved" => (302, Some("text/plain"), b"moved".to_vec()),
        "/missing" => (404, Some("application/json"), br#"{"error":"not found"}"#.to_vec()),
        "/boom" => (500, Some("text/plain"), b"server error".to_vec()),
        "/truncated" => {
            // Promises more than it sends and hangs up: reading the body must fail.
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 100\r\nConnection: close\r\n\r\nshort",
            );
            let _ = stream.flush();
            return;
        }
        "/slow" => {
            thread::sleep(Duration::from_secs(3));
            (200, Some("text/plain"), b"late".to_vec())
        }
        p if p.starts_with("/size/") => {
            let n: usize = p.trim_start_matches("/size/").parse().unwrap_or(0);
            (200, Some("text/plain"), vec![b'x'; n])
        }
        _ => (200, Some("text/plain"), b"ok".to_vec()),
    };

    let mut head = format!("HTTP/1.1 {status} {}\r\n", reason(status));
    if let Some(ct) = content_type {
        head.push_str(&format!("Content-Type: {ct}\r\n"));
    }
    if route == "/moved" {
        head.push_str("Location: /json\r\n");
    }
    head.push_str("X-Demo: demo-value\r\n");
    head.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n", payload.len()));
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&payload);
    let _ = stream.flush();
}

fn reason(status: u16) -> &'static str {
    match status {
        201 => "Created",
        302 => "Found",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    }
}
