//! The wire: framed JSON-RPC messages, the client the server writes to, and the LSP types the
//! handlers share.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// The JSON-RPC error code of a request no handler answers.
const METHOD_NOT_FOUND: i64 = -32601;

/// A position in a document: 0-based line, and column in UTF-16 code units.
#[derive(Deserialize, Serialize, Default, Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub line: usize,
    #[serde(default)]
    pub character: usize,
}

#[derive(Deserialize, Default)]
pub(super) struct TextDocumentIdentifier {
    #[serde(default)]
    pub uri: String,
}

/// `textDocument` and `position`: what definition and hover are asked about.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TextDocumentPositionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// The typed parameters of a message, or `None` when they do not have the expected shape.
pub(super) fn decode<P: DeserializeOwned>(params: Value) -> Option<P> {
    serde_json::from_value(params).ok()
}

/// Reads one framed message; `None` at the end of the input or on a malformed message.
pub(super) fn read_message(input: &mut dyn BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    input.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn write_message(out: &mut dyn Write, message: &Value) -> io::Result<()> {
    let body = message.to_string();
    write!(out, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    out.flush()
}

/// The client the server writes to: responses, and requests of its own (ids `zed-http-<n>`).
pub(crate) struct Client<'a> {
    out: &'a mut dyn Write,
    next_id: u64,
    /// The first write that failed; nothing is written after it.
    failure: Option<io::Error>,
}

impl<'a> Client<'a> {
    pub fn new(out: &'a mut dyn Write) -> Client<'a> {
        Client { out, next_id: 0, failure: None }
    }

    fn send(&mut self, message: Value) {
        if self.failure.is_none() {
            self.failure = write_message(self.out, &message).err();
        }
    }

    /// The write that failed, if one did: the client is gone, and serving is over.
    pub fn take_failure(&mut self) -> Option<io::Error> {
        self.failure.take()
    }

    pub fn respond(&mut self, id: Value, result: Value) {
        self.send(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }

    pub fn respond_method_not_found(&mut self, id: Value, method: &str) {
        let error = json!({ "code": METHOD_NOT_FOUND, "message": format!("method not found: {method}") });
        self.send(json!({ "jsonrpc": "2.0", "id": id, "error": error }));
    }

    /// Sends a request of the server's own; its response comes back as a message without a method.
    pub fn request(&mut self, method: &str, params: Option<Value>) {
        self.next_id += 1;
        let mut message = json!({ "jsonrpc": "2.0", "id": format!("zed-http-{}", self.next_id), "method": method });
        if let Some(params) = params {
            message["params"] = params;
        }
        self.send(message);
    }

    /// Asks Zed to request the code lenses again: whether a route's request exists ("Open") or
    /// not ("Generate") depends on `.http` files, which change apart from the source file.
    pub fn refresh_lenses(&mut self) {
        self.request("workspace/codeLens/refresh", None);
    }

    pub fn apply_edit(&mut self, label: &str, edit: Value) {
        self.request("workspace/applyEdit", Some(json!({ "label": label, "edit": edit })));
    }

    pub fn show_document(&mut self, path: &Path, line: usize) {
        let params = json!({ "uri": path_to_uri(path), "takeFocus": true, "selection": line_range(line) });
        self.request("window/showDocument", Some(params));
    }
}

/// The empty range at the start of `line`.
pub(super) fn line_range(line: usize) -> Value {
    let start = Position { line, character: 0 };
    json!({ "start": start, "end": start })
}

/// The location of `name` at `position` in the file at `path`.
pub(super) fn location(path: &Path, position: Position, name: &str) -> Value {
    let end = Position { line: position.line, character: position.character + utf16_len(name) };
    json!({ "uri": path_to_uri(path), "range": { "start": position, "end": end } })
}

pub(super) fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The byte index of the UTF-16 column `utf16_column` in `line`, clamped to its end.
pub(super) fn byte_index(line: &str, utf16_column: usize) -> usize {
    let mut units = 0;
    for (index, c) in line.char_indices() {
        if units >= utf16_column {
            return index;
        }
        units += c.len_utf16();
    }
    line.len()
}

/// `file://` URI to a path, undoing percent-encoding and the leading slash of `/C:/…`.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let raw = uri.strip_prefix("file://")?;
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&raw[index + 1..index + 3], 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    let mut path = String::from_utf8(out).ok()?;
    if path.len() > 2 && path.as_bytes()[0] == b'/' && path.as_bytes()[2] == b':' {
        path.remove(0);
    }
    Some(PathBuf::from(path))
}

/// Path to a `file://` URI.
pub fn path_to_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from(if text.starts_with('/') { "file://" } else { "file:///" });
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The name of the file at `path`, for titles.
pub(super) fn file_name(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}
