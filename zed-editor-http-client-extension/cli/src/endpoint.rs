//! Detecting HTTP endpoints in source code and turning them into `.http` templates.

use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, PartialEq)]
pub struct Endpoint {
    pub method: String,
    /// `{{baseUrl}}/users/{{id}}` or an absolute URL.
    pub url: String,
}

const METHODS: &str = "get|post|put|delete|patch|head|options";

struct Patterns {
    /// Spring `@GetMapping("/x")`, NestJS `@Get('x')`, also with `value =` / `path =`.
    annotation: Regex,
    /// Spring `@RequestMapping(value = "/x", method = RequestMethod.POST)`.
    request_mapping: Regex,
    /// Rust actix/rocket `#[get("/x")]`.
    attribute: Regex,
    /// Express/Fastify/Gin/Echo/Chi/FastAPI/Flask/Laravel/HTTP clients: `x.get("/path"` / `Route::get('/path'`.
    call: Regex,
    /// Flask `@app.route("/x", methods=["POST"])`, axum `.route("/x", post(handler))`.
    route: Regex,
    /// Dart `http.post(Uri.parse('$baseUrl/x'))`, `client.get(Uri.https('host', '/x'))`.
    dart_uri: Regex,
    /// Go `http.HandleFunc("POST /x", ...)`.
    handle_func: Regex,
    /// `fetch("https://..", { method: "POST" })`.
    fetch: Regex,
    /// Any absolute URL literal as a last resort.
    url_literal: Regex,
    method_kw: Regex,
    /// Class-level `@RequestMapping("/api")` / `@Controller('api')` / `@Path("/api")`.
    controller: Regex,
    placeholder: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| {
        let q = r#"["'`]"#;
        Patterns {
            annotation: Regex::new(&format!(
                r#"@(?i:({METHODS}))(?:Mapping)?\b\s*(?:\(\s*(?:(?:value|path)\s*=\s*)?\{{?\s*(?:{q}([^"'`]*){q})?)?"#
            ))
            .unwrap(),
            request_mapping: Regex::new(&format!(r#"@RequestMapping\s*\(.*?{q}([^"'`]*){q}"#)).unwrap(),
            attribute: Regex::new(&format!(r#"#\[\s*(?i:({METHODS}))\s*\(\s*"([^"]*)""#)).unwrap(),
            call: Regex::new(&format!(r#"(?:\.|::)(?i:({METHODS}))\s*(?:<[^()]*>)?\s*\(\s*{q}((?:/|https?://)[^"'`]*){q}"#)).unwrap(),
            route: Regex::new(&format!(r#"\.route\s*\(\s*{q}([^"'`]*){q}(.*)"#)).unwrap(),
            dart_uri: Regex::new(&format!(
                r#"\.({METHODS})\s*(?:<[^()]*>)?\s*\(\s*Uri\.(?:parse\s*\(\s*{q}([^"'`]+){q}|(https?)\s*\(\s*{q}([^"'`]+){q}\s*,\s*{q}([^"'`]*){q})"#
            ))
            .unwrap(),
            handle_func: Regex::new(&format!(r#"Handle(?:Func)?\s*\(\s*"(?:([A-Z]+)\s+)?(/[^"]*)""#)).unwrap(),
            fetch: Regex::new(&format!(r#"fetch\s*\(\s*{q}([^"'`]+){q}(.*)"#)).unwrap(),
            url_literal: Regex::new(&format!(r#"{q}(https?://[^"'`\s]+){q}"#)).unwrap(),
            method_kw: Regex::new(&format!(r#"(?i)\b({METHODS})\b"#)).unwrap(),
            controller: Regex::new(r#"@(?:RequestMapping|Controller|Path)\s*\(\s*(?:(?:value|path)\s*=\s*)?["']([^"']*)["']"#).unwrap(),
            placeholder: Regex::new(r"\$\{\s*([\w.]+)\s*\}|\$(\w+)|\{\s*(\w+)(?::[^}]*)?\s*\}|<(?:\w+:)?(\w+)>|(?:^|/):(\w+)").unwrap(),
        }
    })
}

/// Finds an endpoint declared or called on `line`. `text` is the whole file (for class-level prefixes).
pub fn detect(text: &str, line: usize) -> Option<Endpoint> {
    let p = patterns();
    let src = text.lines().nth(line)?;
    let up = |m: &str| m.to_uppercase();
    let method_in = |rest: &str| p.method_kw.captures(rest).map(|c| up(&c[1]));

    let (method, path, prefixed) = if let Some(c) = p.attribute.captures(src) {
        (up(&c[1]), c[2].to_string(), false)
    } else if let Some(c) = p.request_mapping.captures(src).filter(|_| !is_class_level(text, line)) {
        let method = src.split_once("RequestMethod.").and_then(|(_, r)| method_in(r)).unwrap_or("GET".into());
        (method, c[1].to_string(), true)
    } else if let Some(c) = p.annotation.captures(src) {
        (up(&c[1]), c.get(2).map_or("", |m| m.as_str()).to_string(), true)
    } else if let Some(c) = p.dart_uri.captures(src) {
        let url = match c.get(2) {
            Some(u) => u.as_str().to_string(),
            None => format!("{}://{}{}", &c[3], &c[4], &c[5]),
        };
        (up(&c[1]), url, false)
    } else if let Some(c) = p.handle_func.captures(src) {
        (c.get(1).map_or("GET".into(), |m| m.as_str().to_string()), c[2].to_string(), false)
    } else if let Some(c) = p.route.captures(src) {
        let rest = &c[2];
        let method = rest
            .split_once("methods")
            .map_or(rest, |(_, r)| r)
            .split(|ch: char| !ch.is_alphanumeric())
            .find_map(method_in)
            .unwrap_or("GET".into());
        (method, c[1].to_string(), false)
    } else if let Some(c) = p.call.captures(src) {
        (up(&c[1]), c[2].to_string(), false)
    } else if let Some(c) = p.fetch.captures(src) {
        let method = c[2].split_once("method").and_then(|(_, r)| method_in(r)).unwrap_or("GET".into());
        (method, c[1].to_string(), false)
    } else if let Some(c) = p.url_literal.captures(src) {
        ("GET".into(), c[1].to_string(), false)
    } else {
        return None;
    };

    let path = if prefixed { join(&class_prefix(text, line), &path) } else { path };
    Some(Endpoint { method, url: to_http_url(&path) })
}

/// `@RequestMapping("/api")` / `@Controller('api')` above the class containing `line`.
fn class_prefix(text: &str, line: usize) -> String {
    let lines: Vec<&str> = text.lines().take(line).collect();
    for (i, l) in lines.iter().enumerate().rev() {
        if let Some(c) = patterns().controller.captures(l) {
            if is_class_level(text, i) {
                return c[1].to_string();
            }
        }
    }
    String::new()
}

/// True if an annotation on `line` is followed (within a few lines) by a class declaration.
fn is_class_level(text: &str, line: usize) -> bool {
    text.lines()
        .skip(line)
        .take(6)
        .any(|l| l.contains("class ") || l.contains("interface "))
}

fn join(prefix: &str, path: &str) -> String {
    let (a, b) = (prefix.trim_matches('/'), path.trim_matches('/'));
    match (a.is_empty(), b.is_empty()) {
        (true, true) => "/".into(),
        (true, false) => format!("/{b}"),
        (false, true) => format!("/{a}"),
        (false, false) => format!("/{a}/{b}"),
    }
}

/// Converts `{id}`, `{id:int}`, `:id`, `<id>`, `<int:id>`, `${id}` into `{{id}}` and prefixes `{{baseUrl}}`.
pub fn to_http_url(path: &str) -> String {
    let p = patterns();
    let converted = p.placeholder.replace_all(path, |c: &regex::Captures| {
        let lead = if c[0].starts_with('/') { "/" } else { "" };
        let name = (1..=5).find_map(|i| c.get(i)).map_or("", |m| m.as_str());
        let name = name.rsplit('.').next().unwrap_or(name);
        format!("{lead}{{{{{name}}}}}")
    });
    if converted.starts_with("http://") || converted.starts_with("https://") || converted.starts_with("{{") {
        converted.into_owned()
    } else {
        format!("{{{{baseUrl}}}}/{}", converted.trim_start_matches('/'))
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
    let body = if matches!(method, "POST" | "PUT" | "PATCH") {
        "Content-Type: application/json\n\n{\n  \n}\n"
    } else {
        "\n"
    };
    format!("### {name}\n# @name {name}\n{method} {url}\nAccept: application/json\n{body}")
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

pub const DEFAULT_ENV: &str = "{\n  \"dev\": {\n    \"baseUrl\": \"http://localhost:8080\"\n  }\n}\n";

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str, line: usize) -> Option<(String, String)> {
        detect(text, line).map(|e| (e.method, e.url))
    }
    fn e(m: &str, u: &str) -> Option<(String, String)> {
        Some((m.into(), u.into()))
    }

    #[test]
    fn frameworks() {
        assert_eq!(d(r#"app.get('/users/:id', (req, res) => {"#, 0), e("GET", "{{baseUrl}}/users/{{id}}"));
        assert_eq!(d(r#"router.POST("/items", h)"#, 0), e("POST", "{{baseUrl}}/items"));
        assert_eq!(d(r#"@app.post("/items/{item_id}")"#, 0), e("POST", "{{baseUrl}}/items/{{item_id}}"));
        assert_eq!(d(r#"@app.route("/a/<int:id>", methods=["PUT"])"#, 0), e("PUT", "{{baseUrl}}/a/{{id}}"));
        assert_eq!(d(r#"#[get("/hello/{name}")]"#, 0), e("GET", "{{baseUrl}}/hello/{{name}}"));
        assert_eq!(d(r#".route("/users", post(create_user))"#, 0), e("POST", "{{baseUrl}}/users"));
        assert_eq!(d(r#"http.HandleFunc("DELETE /x/{id}", h)"#, 0), e("DELETE", "{{baseUrl}}/x/{{id}}"));
        assert_eq!(d(r#"Route::patch('/p/{id}', [C::class, 'u']);"#, 0), e("PATCH", "{{baseUrl}}/p/{{id}}"));
        assert_eq!(
            d(r#"await fetch(`https://api.x.io/u/${user.id}`, { method: "DELETE" })"#, 0),
            e("DELETE", "https://api.x.io/u/{{id}}")
        );
        assert_eq!(d(r#"requests.get("https://e.com/a")"#, 0), e("GET", "https://e.com/a"));
        assert_eq!(d(r#"map.get("key")"#, 0), None);
    }

    #[test]
    fn dart_and_flutter() {
        assert_eq!(d(r#"final r = await dio.get('/users/$id');"#, 0), e("GET", "{{baseUrl}}/users/{{id}}"));
        assert_eq!(d(r#"await _dio.post<Map<String, dynamic>>('/auth/login', data: body);"#, 0), e("POST", "{{baseUrl}}/auth/login"));
        assert_eq!(d(r#"await http.put(Uri.parse('$baseUrl/items/${item.id}'), body: b);"#, 0), e("PUT", "{{baseUrl}}/items/{{id}}"));
        assert_eq!(d(r#"client.delete(Uri.https('api.x.io', '/v1/x'))"#, 0), e("DELETE", "https://api.x.io/v1/x"));
        assert_eq!(d(r#"  @GET('/tasks/{id}')"#, 0), e("GET", "{{baseUrl}}/tasks/{{id}}"));
        assert_eq!(d(r#"router.get('/hello/<name>', handler);"#, 0), e("GET", "{{baseUrl}}/hello/{{name}}"));
    }

    #[test]
    fn spring_and_nest_prefixes() {
        let java = "@RestController\n@RequestMapping(\"/api/users\")\npublic class C {\n  @GetMapping(\"/{id}\")\n  User g() {}\n  @PostMapping\n  void c() {}\n  @RequestMapping(value = \"/x\", method = RequestMethod.PUT)\n}";
        assert_eq!(d(java, 3), e("GET", "{{baseUrl}}/api/users/{{id}}"));
        assert_eq!(d(java, 5), e("POST", "{{baseUrl}}/api/users"));
        assert_eq!(d(java, 7), e("PUT", "{{baseUrl}}/api/users/x"));
        assert_eq!(d(java, 1), None);
        let ts = "@Controller('cats')\nexport class CatsController {\n  @Get(':id')\n  findOne() {}\n}";
        assert_eq!(d(ts, 2), e("GET", "{{baseUrl}}/cats/{{id}}"));
    }

    #[test]
    fn names() {
        assert_eq!(default_name("GET", "{{baseUrl}}/users/{{id}}"), "get-users-by-id");
        assert_eq!(default_name("GET", "https://e.com/a/b"), "get-a-b");
    }
}
