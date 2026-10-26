//! Where the application a source file belongs to listens, read from its Spring configuration
//! (`application.yml` / `application.properties`): `server.port`, `server.servlet.context-path`,
//! TLS. Also resolves `${...}` placeholders of client URLs (`@FeignClient(url = "${catalog.url}")`)
//! against the same configuration.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The configuration of the project `source` belongs to: the nearest directory with a build
/// file (`pubspec.yaml`, `pom.xml`, `build.gradle(.kts)`), its `application.yml` then
/// `application.properties` (properties win, as in Spring Boot) from the usual places.
pub fn project_properties(source: &Path) -> Option<HashMap<String, String>> {
    let root = project_root(source)?;
    let mut props = HashMap::new();
    let mut found = false;
    for dir in ["lib/resources", "src/main/resources", "resources", "config", "."] {
        for (file, yaml) in [("application.yaml", true), ("application.yml", true), ("application.properties", false)] {
            let path = root.join(dir).join(file);
            let Ok(text) = fs::read_to_string(&path) else { continue };
            found = true;
            let parsed = if yaml { parse_yaml(&text) } else { parse_properties(&text) };
            props.extend(parsed);
        }
    }
    found.then_some(props)
}

/// The nearest ancestor of `source` holding a build file.
pub fn project_root(source: &Path) -> Option<PathBuf> {
    let markers = ["pubspec.yaml", "pom.xml", "build.gradle", "build.gradle.kts"];
    let mut dir = if source.is_dir() { Some(source) } else { source.parent() };
    while let Some(d) = dir {
        if markers.iter().any(|m| d.join(m).exists()) {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

/// The local address of a Spring application with `props`: `http(s)://localhost:<port><context path>`.
/// `server.port` (a placeholder's default included; `0` means a random port, so the default 8080
/// stays), TLS when `server.ssl.*` names a certificate or key store, `server.address` unless it
/// listens on every interface, and `server.servlet.context-path`.
pub fn spring_base_url(props: &HashMap<String, String>) -> String {
    let get = |key: &str| props.get(key).map(|v| resolve(v, props).0).filter(|v| !v.trim().is_empty());
    let port = get("server.port").and_then(|p| p.trim().parse::<u16>().ok()).filter(|p| *p != 0).unwrap_or(8080);
    let ssl_off = get("server.ssl.enabled").is_some_and(|v| v.trim().eq_ignore_ascii_case("false"));
    let secure = !ssl_off
        && ["server.ssl.certificate", "server.ssl.key-store", "server.ssl.bundle"].iter().any(|k| get(k).is_some());
    let host = match get("server.address").as_deref().map(str::trim) {
        None | Some("0.0.0.0") | Some("::") | Some("[::]") | Some("127.0.0.1") => "localhost".to_string(),
        Some(other) => other.to_string(),
    };
    let context = get("server.servlet.context-path")
        .or_else(|| get("server.context-path"))
        .map(|c| {
            let c = c.trim().trim_end_matches('/').to_string();
            if c.is_empty() || c.starts_with('/') {
                c
            } else {
                format!("/{c}")
            }
        })
        .unwrap_or_default();
    format!("{}://{host}:{port}{context}", if secure { "https" } else { "http" })
}

/// Resolves `${key}` and `${key:default}` in `text` against `props` (nested ones too). Returns
/// the text and the keys that could not be resolved, which stay as `${key}`.
pub fn resolve(text: &str, props: &HashMap<String, String>) -> (String, Vec<String>) {
    let mut missing = Vec::new();
    let out = resolve_depth(text, props, 0, &mut missing);
    (out, missing)
}

fn resolve_depth(text: &str, props: &HashMap<String, String>, depth: usize, missing: &mut Vec<String>) -> String {
    if depth > 10 {
        return text.to_string();
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = closing(after) else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = &after[..end];
        let (key, default) = match split_default(inner) {
            Some((k, d)) => (k.trim(), Some(d)),
            None => (inner.trim(), None),
        };
        match props.get(key) {
            Some(value) => out.push_str(&resolve_depth(value, props, depth + 1, missing)),
            None => match default {
                Some(d) => out.push_str(&resolve_depth(d, props, depth + 1, missing)),
                None => {
                    missing.push(key.to_string());
                    out.push_str(&format!("${{{key}}}"));
                }
            },
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

// The `}` closing a placeholder body, skipping nested `${...}`.
fn closing(text: &str) -> Option<usize> {
    let mut depth = 0;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && bytes.get(i + 1) == Some(&b'{') {
            depth += 1;
            i += 2;
            continue;
        }
        if bytes[i] == b'}' {
            if depth == 0 {
                return Some(i);
            }
            depth -= 1;
        }
        i += 1;
    }
    None
}

// `key:default`, where the `:` is not inside a nested placeholder or a URL scheme of the key.
fn split_default(inner: &str) -> Option<(&str, &str)> {
    let mut depth = 0;
    for (i, c) in inner.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            ':' if depth == 0 => return Some((&inner[..i], &inner[i + 1..])),
            _ => {}
        }
    }
    None
}

/// A variable name for an unresolved property: `catalog.url` → `catalogUrl`,
/// `spring.cloud.openfeign.client.config.catalog.url` → `catalogUrl` (the last two segments).
pub fn variable_for(key: &str) -> String {
    let parts: Vec<&str> = key.split(['.', '-', '_']).filter(|p| !p.is_empty()).collect();
    let tail = if parts.len() > 2 { &parts[parts.len() - 2..] } else { &parts[..] };
    let mut out = String::new();
    for (i, part) in tail.iter().enumerate() {
        let lower = part.to_lowercase();
        if i == 0 {
            out.push_str(&lower);
        } else {
            let mut chars = lower.chars();
            out.extend(chars.next().map(|c| c.to_ascii_uppercase()));
            out.push_str(chars.as_str());
        }
    }
    if out.is_empty() {
        "baseUrl".into()
    } else {
        out
    }
}

/// `key=value` / `key: value` lines of a `.properties` file (comments, continuations with `\`).
pub fn parse_properties(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut pending = String::new();
    for raw in text.lines() {
        let line = raw.trim_start();
        if pending.is_empty() && (line.is_empty() || line.starts_with('#') || line.starts_with('!')) {
            continue;
        }
        if let Some(cont) = line.strip_suffix('\\') {
            pending.push_str(cont);
            continue;
        }
        pending.push_str(line);
        let entry = std::mem::take(&mut pending);
        let split = entry.find(['=', ':']).unwrap_or(entry.len());
        let key = entry[..split].trim();
        let value = entry.get(split + 1..).unwrap_or_default().trim();
        if !key.is_empty() {
            out.insert(key.to_string(), value.to_string());
        }
    }
    out
}

/// The scalar values of a YAML file as dotted keys (`server:\n  port: 8081` → `server.port`).
/// Only the first document counts, and documents that only apply to a profile are skipped; lists
/// are ignored. Enough for the keys this module reads.
pub fn parse_yaml(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for document in text.split("\n---") {
        let doc = parse_yaml_document(document);
        if doc.keys().any(|k| k.starts_with("spring.config.activate.on-profile") || k == "spring.profiles") {
            continue;
        }
        for (k, v) in doc {
            out.entry(k).or_insert(v);
        }
    }
    out
}

fn parse_yaml_document(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    for raw in text.lines() {
        let without_comment = strip_comment(raw);
        if without_comment.trim().is_empty() || without_comment.trim_start().starts_with('-') {
            continue;
        }
        let indent = without_comment.len() - without_comment.trim_start().len();
        let line = without_comment.trim();
        let Some(colon) = key_colon(line) else { continue };
        let key = unquote(line[..colon].trim());
        let value = line[colon + 1..].trim();
        while stack.last().is_some_and(|(i, _)| *i >= indent) {
            stack.pop();
        }
        let full = stack.iter().map(|(_, k)| k.as_str()).chain([key.as_str()]).collect::<Vec<_>>().join(".");
        if value.is_empty() {
            stack.push((indent, key));
        } else if !value.starts_with('|') && !value.starts_with('>') {
            out.insert(full, unquote(value));
        }
    }
    out
}

// The `:` ending a key (followed by a space or the end), outside quotes.
fn key_colon(line: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, ':') if line[i + 1..].is_empty() || line[i + 1..].starts_with(' ') => return Some(i),
            _ => {}
        }
    }
    None
}

fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '#') if i == 0 || line[..i].ends_with(' ') => return &line[..i],
            _ => {}
        }
    }
    line
}

fn unquote(value: &str) -> String {
    let v = value.trim();
    if v.len() >= 2 && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\''))) {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}
