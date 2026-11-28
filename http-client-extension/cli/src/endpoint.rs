//! Turning detected endpoints into `.http` requests and files.

use crate::catalog::Catalog;
use crate::infer::{self, FieldType, RequestSpec};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// A `{{name}}` variable.
static URL_VARIABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{\s*(\w+)\s*\}\}").expect("the variable pattern is a valid regex"));

#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    pub method: String,
    /// `{{baseUrl}}/users/{{id}}` or an absolute URL.
    pub url: String,
    /// Matched only as a bare URL literal (no method/route context).
    pub weak: bool,
}

/// The request generated for a route: its endpoint, its name and its `.http` text.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedRequest {
    pub endpoint: Endpoint,
    pub name: String,
    pub content: String,
}

/// Prefixes relative paths with `{{baseUrl}}`.
///
/// A path may already start with a variable: `{{baseUrl}}/items` (from `'$baseUrl/items'`) is the
/// whole URL, while `{{id}}` (from `@HttpGet("{id}")`) is just the first segment of a path.
pub fn to_http_url(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") || names_a_base_url(path) {
        path.to_string()
    } else {
        format!("{{{{baseUrl}}}}/{}", path.trim_start_matches('/'))
    }
}

fn names_a_base_url(path: &str) -> bool {
    let Some((name, _)) = path.strip_prefix("{{").and_then(|rest| rest.split_once("}}")) else {
        return false;
    };
    names_a_url(name)
}

/// Does `name` name a whole URL (`apiUrl`, `_endpoint`, `GROQ_URL`) rather than one path segment?
pub fn names_a_url(name: &str) -> bool {
    let name = name.to_lowercase();
    ["url", "uri", "base", "host", "server", "endpoint", "origin", "api"].iter().any(|k| name.contains(k))
}

pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

pub fn default_name(method: &str, url: &str) -> String {
    let path = url.trim_start_matches("{{baseUrl}}");
    let path = path.split_once("://").map_or(path, |(_, r)| r.split_once('/').map_or("", |(_, p)| p));
    slug(&format!("{method} {}", path.replace("{{", "by-").replace("}}", "")))
}

pub fn template(method: &str, url: &str, name: &str) -> String {
    render(method, url, name, &RequestSpec::default())
}

/// `{{id}}`-style variables in a URL, except `baseUrl`.
pub fn url_vars(url: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for captures in URL_VARIABLE.captures_iter(url) {
        let name = &captures[1];
        if name != "baseUrl" && !out.contains(&name.to_string()) {
            out.push(name.to_string());
        }
    }
    out
}

/// The boundary between the parts of a generated `multipart/form-data` body.
const BOUNDARY: &str = "boundary";

/// A `@name = value` declaration a generated request starts with.
struct FileVariable {
    name: String,
    value: &'static str,
}

/// Full `.http` request: file variables for path/query/header/cookie values, query string, headers,
/// a JSON or multipart body.
pub fn render(method: &str, url: &str, name: &str, spec: &RequestSpec) -> String {
    // A path parameter stands for one segment; a variable naming the whole URL needs a URL.
    let path_sample = |variable: &str| if names_a_url(variable) { LOCAL_SERVER } else { "1" };
    let mut variables: Vec<FileVariable> = url_vars(url)
        .into_iter()
        .map(|variable| FileVariable { value: path_sample(&variable), name: variable })
        .collect();
    let mut url = url.to_string();
    for (index, parameter) in spec.query.iter().enumerate() {
        let variable = infer::variable_name(&parameter.name);
        let separator = if index == 0 && !url.contains('?') { '?' } else { '&' };
        url.push_str(&format!("{separator}{}={{{{{variable}}}}}", parameter.name));
        variables.push(FileVariable { name: variable, value: parameter.kind.sample_variable() });
    }
    let mut headers = Vec::new();
    if !spec.header_values.iter().any(|(name, _)| name.eq_ignore_ascii_case("accept")) {
        headers.push("Accept: application/json".to_string());
    }
    for (name, value) in &spec.header_values {
        headers.push(format!("{name}: {value}"));
    }
    if spec.auth {
        headers.push("Authorization: Bearer {{token}}".into());
    }
    for header in &spec.headers {
        let variable = infer::variable_name(header);
        headers.push(format!("{header}: {{{{{variable}}}}}"));
        variables.push(FileVariable { name: variable, value: "value" });
    }
    if !spec.cookies.is_empty() {
        let mut cookies = Vec::new();
        for cookie in &spec.cookies {
            let variable = infer::variable_name(cookie);
            cookies.push(format!("{cookie}={{{{{variable}}}}}"));
            variables.push(FileVariable { name: variable, value: "value" });
        }
        headers.push(format!("Cookie: {}", cookies.join("; ")));
    }
    let body = if spec.parts.is_empty() {
        spec.body.as_ref().map(|fields| {
            let sample = FieldType::Object(fields.clone()).sample_value();
            headers.push("Content-Type: application/json".into());
            serde_json::to_string_pretty(&sample).expect("sample values always serialize")
        })
    } else {
        headers.push(format!("Content-Type: multipart/form-data; boundary={BOUNDARY}"));
        let parts: Vec<String> =
            spec.parts.iter().map(|part| format!("--{BOUNDARY}\n{}", part.kind.form_part(&part.name))).collect();
        Some(format!("{}\n--{BOUNDARY}--", parts.join("\n")))
    };

    // Generated requests keep no response log; delete `# @no-log` to save responses to `.zed-http/`.
    let mut out = format!("### {name}\n# @name {name}\n# @no-log\n");
    if spec.auth {
        out.push_str("# Requires `token` in .zed/http/http-client.env.json\n");
    }
    for variable in &variables {
        out.push_str(&format!("@{} = {}\n", variable.name, variable.value));
    }
    out.push_str(&format!("{method} {url}\n{}\n", headers.join("\n")));
    if let Some(body) = body {
        out.push_str(&format!("\n{body}\n"));
    }
    out
}

/// Detects the route on `line` of `source` and renders a full request for it. When the
/// application's configuration gives another address than the environment's `baseUrl` (set by
/// hand), the request says so in a comment.
pub fn generate(catalog: &Catalog, text: &str, source: &Path, line: usize) -> Option<GeneratedRequest> {
    let inferred = catalog.request(text, source, line)?;
    let endpoint = inferred.endpoint;
    let name = default_name(&endpoint.method, &endpoint.url);
    let mut content = render(&endpoint.method, &endpoint.url, &name, &inferred.spec);
    if endpoint.url.starts_with("{{baseUrl}}") {
        if let (Some(server), Some(current)) = (local_server_for(source), env_base_url(source)) {
            if current != server && current != LOCAL_SERVER {
                let note = format!(
                    "# The application's configuration gives {server}; baseUrl in http-client.env.json is {current}\n"
                );
                let header = format!("### {name}\n# @name {name}\n");
                content = content.replacen(&header, &format!("{header}{note}"), 1);
            }
        }
    }
    Some(GeneratedRequest { endpoint, name, content })
}

/// Where the application `source` belongs to listens, from its Spring configuration
/// (`server.port`, `server.servlet.context-path`, TLS): `None` without one.
pub fn local_server_for(source: &Path) -> Option<String> {
    crate::server::project_properties(source).map(|props| crate::server::spring_base_url(&props))
}

/// The `baseUrl` of the default environment found above `source`.
fn env_base_url(source: &Path) -> Option<String> {
    let dir = crate::vars::find_env_dir(source.parent()?)?;
    let envs = crate::vars::Environments::load(&dir).ok()?;
    let env = envs.select(None).ok()??;
    envs.vars(Some(&env)).get("baseUrl").cloned()
}

/// The environment file for a project whose server is at `base_url`.
pub fn default_env(base_url: &str) -> String {
    format!("{{\n  \"dev\": {{\n    \"baseUrl\": \"{base_url}\",\n    \"token\": \"\"\n  }}\n}}\n")
}

/// Makes sure `<base>/http-client.env.json` (or one above) points at the application of `source`:
/// creates it with the address from the application's configuration, or sets that address in
/// the default environment when its `baseUrl` is missing or still the generic default. A
/// `baseUrl` set by hand is kept. Returns the file written, if any.
pub fn ensure_env(base: &Path, source: &Path) -> std::io::Result<Option<PathBuf>> {
    let server = local_server_for(source);
    let Some(dir) = crate::vars::find_env_dir(base) else {
        let file = base.join("http-client.env.json");
        std::fs::create_dir_all(base)?;
        std::fs::write(&file, default_env(server.as_deref().unwrap_or(LOCAL_SERVER)))?;
        return Ok(Some(file));
    };
    let Some(server) = server else { return Ok(None) };
    let file = dir.join("http-client.env.json");
    let Ok(text) = std::fs::read_to_string(&file) else { return Ok(None) };
    let Ok(serde_json::Value::Object(mut root)) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(None);
    };
    let names: Vec<String> = root.keys().filter(|k| !k.starts_with('$')).cloned().collect();
    let env = ["dev", "local", "development"]
        .iter()
        .map(|s| s.to_string())
        .find(|d| names.contains(d))
        .or_else(|| names.first().cloned())
        .unwrap_or_else(|| "dev".into());
    let entry = root.entry(env).or_insert_with(|| serde_json::Value::Object(Default::default()));
    let Some(values) = entry.as_object_mut() else { return Ok(None) };
    let current = values.get("baseUrl").and_then(|v| v.as_str()).map(str::to_string);
    if current.as_deref().is_some_and(|c| c != LOCAL_SERVER) || current.as_deref() == Some(server.as_str()) {
        return Ok(None);
    }
    values.insert("baseUrl".into(), serde_json::Value::String(server));
    let pretty = serde_json::to_string_pretty(&serde_json::Value::Object(root)).expect("JSON values serialize");
    std::fs::write(&file, format!("{pretty}\n"))?;
    Ok(Some(file))
}

/// `.http` file collecting all requests generated from `source`: `<dir>/<source file name>.http`
/// (`server.ts` → `server.ts.http`).
pub fn file_for_source(dir: &Path, source: &Path) -> PathBuf {
    // A source path always has a file name; `endpoints.http` is only a safety net.
    let name = source.file_name().map_or("endpoints".into(), |n| n.to_string_lossy().into_owned());
    dir.join(format!("{name}.http"))
}

/// 0-based line of the `### name` header of the request `# @name name` in `text`.
pub fn find_request(text: &str, name: &str) -> Option<usize> {
    let lines: Vec<&str> = text.lines().collect();
    let at = lines.iter().position(|l| {
        let l = l.trim_start().trim_start_matches(['#', '/']).trim_start();
        l.strip_prefix("@name").is_some_and(|r| r.trim_start_matches([' ', '=']).trim() == name)
    })?;
    Some((0..=at).rev().find(|&i| lines[i].trim_start().starts_with("###")).unwrap_or(at))
}

/// Text to append a request to an existing `.http` file (keeps one blank line between requests).
pub fn append_text(existing: &str, request: &str) -> String {
    let sep = if existing.is_empty() || existing.ends_with("\n\n") {
        ""
    } else if existing.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    format!("{sep}{request}")
}

/// `<root>/.zed/http/endpoints/<name>.http`, with `-1`, `-2`… if taken.
pub fn free_path(dir: &Path, name: &str) -> PathBuf {
    let mut path = dir.join(format!("{name}.http"));
    let mut i = 1;
    while path.exists() {
        path = dir.join(format!("{name}-{i}.http"));
        i += 1;
    }
    path
}

/// What a URL defaults to before the environment file is filled in.
pub const LOCAL_SERVER: &str = "http://localhost:8080";

pub const DEFAULT_ENV: &str =
    "{\n  \"dev\": {\n    \"baseUrl\": \"http://localhost:8080\",\n    \"token\": \"\"\n  }\n}\n";
