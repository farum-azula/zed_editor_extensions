use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Context {
    pub vars: HashMap<String, String>,
    pub responses: serde_json::Map<String, Value>,
    /// Responses of `# @no-log` requests, left out when the store is saved.
    pub unlogged: std::collections::HashSet<String>,
}

impl Context {
    pub fn resolve(&self, input: &str) -> Result<String, String> {
        self.resolve_depth(input, 0)
    }

    fn resolve_depth(&self, input: &str, depth: usize) -> Result<String, String> {
        if depth > 16 {
            return Err(format!("variable recursion too deep in `{input}`"));
        }
        let mut out = String::with_capacity(input.len());
        let mut rest = input;
        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            let Some(end) = after.find("}}") else {
                out.push_str(&rest[start..]);
                return Ok(out);
            };
            let name = after[..end].trim();
            let value = self.lookup(name)?;
            out.push_str(&self.resolve_depth(&value, depth + 1)?);
            rest = &after[end + 2..];
        }
        out.push_str(rest);
        Ok(out)
    }

    fn lookup(&self, name: &str) -> Result<String, String> {
        if let Some(dynamic) = name.strip_prefix('$') {
            return dynamic_var(dynamic);
        }
        if let Some(v) = self.vars.get(name) {
            return Ok(v.clone());
        }
        if let Some((req, path)) = name.split_once(".response.") {
            let resp = self
                .responses
                .get(req)
                .ok_or_else(|| format!("no stored response for request `{req}` — run it first"))?;
            return response_value(resp, path).ok_or_else(|| format!("`{name}` not found in stored response"));
        }
        Err(format!("unknown variable `{{{{{name}}}}}`"))
    }
}

fn dynamic_var(name: &str) -> Result<String, String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let (func, arg) = match name.split_once(' ') {
        Some((f, a)) => (f, a.trim()),
        None => (name, ""),
    };
    Ok(match func {
        "uuid" | "random.uuid" => {
            let b: [u8; 16] = rng.gen();
            let mut b = b;
            b[6] = (b[6] & 0x0f) | 0x40;
            b[8] = (b[8] & 0x3f) | 0x80;
            let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
            format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
        }
        "timestamp" => chrono::Utc::now().timestamp().to_string(),
        "isoTimestamp" => chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "randomInt" | "random.integer" => {
            let nums: Vec<i64> =
                arg.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|s| s.trim().parse().ok()).collect();
            let (lo, hi) = match nums.as_slice() {
                [a, b] if a < b => (*a, *b),
                _ => (0, 1000),
            };
            rng.gen_range(lo..hi).to_string()
        }
        "random.float" => rng.gen::<f64>().to_string(),
        "random.alphabetic" | "random.alphanumeric" | "random.hexadecimal" => {
            let len: usize = arg.parse().unwrap_or(10);
            let charset: &[u8] = match func {
                "random.alphabetic" => b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
                "random.hexadecimal" => b"0123456789abcdef",
                _ => b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
            };
            (0..len).map(|_| charset[rng.gen_range(0..charset.len())] as char).collect()
        }
        f if f.starts_with("processEnv.") || f.starts_with("env.") => {
            let key = f.split_once('.').unwrap().1;
            std::env::var(key).map_err(|_| format!("environment variable `{key}` is not set"))?
        }
        _ => return Err(format!("unknown dynamic variable `{{{{${name}}}}}`")),
    })
}

fn response_value(resp: &Value, path: &str) -> Option<String> {
    if let Some(h) = path.strip_prefix("headers.") {
        let headers = resp.get("headers")?.as_object()?;
        return headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(h))
            .map(|(_, v)| v.as_str().unwrap_or_default().to_string());
    }
    if path == "status" {
        return resp.get("status").map(|v| v.to_string());
    }
    let body_path = path.strip_prefix("body")?;
    let body_path = body_path.trim_start_matches('.').trim_start_matches('$').trim_start_matches('.');
    let mut cur = resp.get("body")?;
    if !body_path.is_empty() {
        for seg in body_path.split('.') {
            let (key, indices) = match seg.find('[') {
                Some(p) => (&seg[..p], &seg[p..]),
                None => (seg, ""),
            };
            if !key.is_empty() {
                cur = cur.get(key)?;
            }
            for idx in indices.split('[').filter(|s| !s.is_empty()) {
                cur = cur.get(idx.trim_end_matches(']').parse::<usize>().ok()?)?;
            }
        }
    }
    Some(match cur {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

/// The environment asked for: an explicit choice (`--env`), then the `.http` file's own
/// `# @env NAME`, then `HTTP_CLIENT_ENV`. `None` leaves it to [`Environments::select`]'s defaults.
pub fn requested_env(explicit: Option<String>, http_file: &str) -> Option<String> {
    explicit
        .or_else(|| crate::parser::file_env(http_file).map(|(_, name)| name))
        .or_else(|| std::env::var("HTTP_CLIENT_ENV").ok().filter(|s| !s.is_empty()))
}

/// Project-local config dir: `<root>/.zed/http/`.
pub const ZED_HTTP_DIR: &str = ".zed/http";

/// Walks up from `start`; at each level checks the dir itself, then its `.zed/http/`.
pub fn find_env_dir(start: &Path) -> Option<PathBuf> {
    let has_env = |d: &Path| d.join("http-client.env.json").exists() || d.join("http-client.private.env.json").exists();
    let mut dir = Some(start);
    while let Some(d) = dir {
        for c in [d.to_path_buf(), d.join(ZED_HTTP_DIR)] {
            if has_env(&c) {
                return Some(c);
            }
        }
        dir = d.parent();
    }
    None
}

fn read_json(path: &Path) -> Result<serde_json::Map<String, Value>, String> {
    if !path.exists() {
        return Ok(Default::default());
    }
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    match serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))? {
        Value::Object(m) => Ok(m),
        _ => Err(format!("{}: expected a JSON object", path.display())),
    }
}

#[derive(Default)]
pub struct Environments {
    pub names: Vec<String>,
    merged: serde_json::Map<String, Value>,
}

impl Environments {
    pub fn load(http_file_dir: &Path) -> Result<Self, String> {
        let Some(dir) = find_env_dir(http_file_dir) else {
            return Ok(Self { names: vec![], merged: Default::default() });
        };
        let mut merged = read_json(&dir.join("http-client.env.json"))?;
        for (env, vals) in read_json(&dir.join("http-client.private.env.json"))? {
            let entry = merged.entry(env).or_insert_with(|| Value::Object(Default::default()));
            if let (Some(dst), Value::Object(src)) = (entry.as_object_mut(), vals) {
                dst.extend(src);
            }
        }
        let names = merged.keys().filter(|k| !k.starts_with('$')).cloned().collect();
        Ok(Self { names, merged })
    }

    pub fn select(&self, requested: Option<&str>) -> Result<Option<String>, String> {
        if let Some(r) = requested {
            if self.names.iter().any(|n| n == r) {
                return Ok(Some(r.to_string()));
            }
            return Err(format!("environment `{r}` not found (available: {})", self.names.join(", ")));
        }
        for d in ["dev", "local", "development"] {
            if self.names.iter().any(|n| n == d) {
                return Ok(Some(d.to_string()));
            }
        }
        Ok(self.names.first().cloned())
    }

    pub fn vars(&self, env: Option<&str>) -> HashMap<String, String> {
        let mut out = HashMap::new();
        for key in ["$shared"].into_iter().chain(env) {
            if let Some(Value::Object(m)) = self.merged.get(key) {
                for (k, v) in m {
                    let s = match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    out.insert(k.clone(), s);
                }
            }
        }
        out
    }
}
