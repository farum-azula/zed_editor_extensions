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
pub fn to_http_url(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("{{") {
        path.to_string()
    } else {
        format!("{{{{baseUrl}}}}/{}", path.trim_start_matches('/'))
    }
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
    let mut vars: Vec<(String, &str)> = url_vars(url).into_iter().map(|v| (v, "1")).collect();
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
        serde_json::to_string_pretty(&obj).unwrap_or_else(|_| "{}".into())
    });

    let mut out = format!("### {name}\n# @name {name}\n");
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

/// Detects the route on `line` of `source` and renders a full request for it.
pub fn generate(catalog: &Catalog, text: &str, source: &Path, line: usize) -> Option<(Endpoint, String, String)> {
    let (ep, spec, _) = catalog.request(text, source, line)?;
    let name = default_name(&ep.method, &ep.url);
    let content = render(&ep.method, &ep.url, &name, &spec);
    Some((ep, name, content))
}

/// `.http` file collecting all requests generated from `source`: `<dir>/<source file name>.http`
/// (`server.ts` → `server.ts.http`).
pub fn file_for_source(dir: &Path, source: &Path) -> PathBuf {
    let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "endpoints".into());
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

pub const DEFAULT_ENV: &str = "{\n  \"dev\": {\n    \"baseUrl\": \"http://localhost:8080\",\n    \"token\": \"\"\n  }\n}\n";

#[cfg(test)]
mod file_tests {
    use super::*;

    #[test]
    fn per_source_file() {
        assert_eq!(file_for_source(Path::new("/r"), Path::new("/p/server.ts")), PathBuf::from("/r/server.ts.http"));
        let text = "### a\n# @name a\nGET /a\n\n### b\n# @name b\n@id = 1\nGET /b/{{id}}\n";
        assert_eq!(find_request(text, "b"), Some(4));
        assert_eq!(find_request(text, "c"), None);
        assert_eq!(append_text("x\n", "y"), "\ny");
        assert_eq!(append_text("", "y"), "y");
    }
}
