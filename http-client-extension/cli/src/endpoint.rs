//! Turning detected endpoints into `.http` requests and files.

use crate::catalog::Catalog;
use crate::infer::{self, Spec};
use regex::Regex;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    pub method: String,
    /// `{{baseUrl}}/users/{{id}}` or an absolute URL.
    pub url: String,
    /// Matched only as a bare URL literal (no method/route context).
    pub weak: bool,
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
    render(method, url, name, &Spec::default())
}

/// `{{id}}`-style variables in a URL, except `baseUrl`.
pub fn url_vars(url: &str) -> Vec<String> {
    let re = Regex::new(r"\{\{\s*(\w+)\s*\}\}").unwrap();
    let mut out: Vec<String> = Vec::new();
    for c in re.captures_iter(url) {
        if &c[1] != "baseUrl" && !out.contains(&c[1].to_string()) {
            out.push(c[1].to_string());
        }
    }
    out
}

/// Full `.http` request: file variables for path/query/header values, query string, headers, JSON body.
pub fn render(method: &str, url: &str, name: &str, spec: &Spec) -> String {
    // A path parameter stands for one segment; a variable naming the whole URL needs a URL.
    let sample = |v: &str| if names_a_url(v) { LOCAL_SERVER } else { "1" };
    let mut vars: Vec<(String, &str)> = url_vars(url)
        .into_iter()
        .map(|v| {
            let value = sample(&v);
            (v, value)
        })
        .collect();
    let mut url = url.to_string();
    for (i, (q, ty)) in spec.query.iter().enumerate() {
        let v = infer::var_name(q);
        url.push_str(&format!("{}{q}={{{{{v}}}}}", if i == 0 && !url.contains('?') { '?' } else { '&' }));
        vars.push((v, infer::sample_var(ty)));
    }
    let mut headers = vec!["Accept: application/json".to_string()];
    if spec.auth {
        headers.push("Authorization: Bearer {{token}}".into());
    }
    for h in &spec.headers {
        let v = infer::var_name(h);
        headers.push(format!("{h}: {{{{{v}}}}}"));
        vars.push((v, "value"));
    }
    let body = spec.body.as_ref().map(|fields| {
        let obj = infer::sample(&infer::Ty::Obj(fields.clone()));
        headers.push("Content-Type: application/json".into());
        serde_json::to_string_pretty(&obj).expect("sample values always serialize")
    });

    // Generated requests keep no response log; delete `# @no-log` to save responses to `.zed-http/`.
    let mut out = format!("### {name}\n# @name {name}\n# @no-log\n");
    if spec.auth {
        out.push_str("# Requires `token` in .zed/http/http-client.env.json\n");
    }
    for (v, sample) in &vars {
        out.push_str(&format!("@{v} = {sample}\n"));
    }
    out.push_str(&format!("{method} {url}\n{}\n", headers.join("\n")));
    if let Some(b) = body {
        out.push_str(&format!("\n{b}\n"));
    }
    out
}

/// Detects the route on `line` of `source` and renders a full request for it. When the
/// application's configuration gives another address than the environment's `baseUrl` (set by
/// hand), the request says so in a comment.
pub fn generate(catalog: &Catalog, text: &str, source: &Path, line: usize) -> Option<(Endpoint, String, String)> {
    let (ep, spec, _) = catalog.request(text, source, line)?;
    let name = default_name(&ep.method, &ep.url);
    let mut content = render(&ep.method, &ep.url, &name, &spec);
    if ep.url.starts_with("{{baseUrl}}") {
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
    Some((ep, name, content))
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
