#[derive(Debug, Clone, Default)]
pub struct Request {
    pub name: Option<String>,
    pub title: Option<String>,
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Body>,
    pub redirect: Option<(String, bool)>,
    pub start_line: usize,
    pub end_line: usize,
    pub no_redirect: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    Inline(String),
    File(String),
}

#[derive(Debug, Default)]
pub struct HttpFile {
    pub variables: Vec<(String, String)>,
    pub requests: Vec<Request>,
}

const METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE", "CONNECT"];

fn is_separator(line: &str) -> bool {
    line.trim_start().starts_with("###")
}

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with('#') || t.starts_with("//")
}

fn comment_text(line: &str) -> &str {
    let t = line.trim_start();
    let t = t.strip_prefix("//").or_else(|| t.strip_prefix('#')).unwrap_or(t);
    t.trim()
}

/// Parses an `@name = value` declaration.
pub fn parse_var_decl(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    let rest = t.strip_prefix('@')?;
    let (name, value) = rest.split_once('=')?;
    let name = name.trim();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return None;
    }
    Some((name.to_string(), value.trim().to_string()))
}

pub fn parse(src: &str) -> HttpFile {
    let lines: Vec<&str> = src.lines().collect();
    let mut file = HttpFile::default();

    let mut sections: Vec<(usize, usize, Option<String>)> = Vec::new();
    let mut start = 0;
    let mut title = None;
    for (i, line) in lines.iter().enumerate() {
        if is_separator(line) {
            if i > start || title.is_some() {
                sections.push((start, i, title.take()));
            }
            let t = line.trim_start().trim_start_matches('#').trim();
            title = (!t.is_empty()).then(|| t.to_string());
            start = i + 1;
        }
    }
    sections.push((start, lines.len(), title));

    for (s, e, title) in sections {
        if let Some(req) = parse_section(&lines, s, e, title, &mut file.variables) {
            file.requests.push(req);
        }
    }
    file
}

fn parse_section(
    lines: &[&str],
    s: usize,
    e: usize,
    title: Option<String>,
    vars: &mut Vec<(String, String)>,
) -> Option<Request> {
    let mut req = Request { title, start_line: s.max(1), end_line: e, ..Default::default() };

    let mut i = s;
    while i < e {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        if is_comment(line) {
            let c = comment_text(line);
            if let Some(rest) = c.strip_prefix("@name") {
                let n = rest.trim_start_matches(|ch: char| ch == '=' || ch.is_whitespace()).trim();
                if !n.is_empty() {
                    req.name = Some(n.to_string());
                }
            } else if c.starts_with("@no-redirect") {
                req.no_redirect = true;
            }
            i += 1;
            continue;
        }
        if let Some(v) = parse_var_decl(line) {
            vars.push(v);
            i += 1;
            continue;
        }
        break;
    }
    if i >= e {
        return None;
    }

    let first = lines[i].trim();
    let (method, rest) = match first.split_once(char::is_whitespace) {
        Some((m, r)) if METHODS.contains(&m) => (m.to_string(), r.trim().to_string()),
        _ if METHODS.contains(&first) => return None,
        _ => ("GET".to_string(), first.to_string()),
    };
    let mut url = rest;
    i += 1;
    while i < e && (lines[i].starts_with(' ') || lines[i].starts_with('\t')) && !lines[i].trim().is_empty() {
        url.push_str(lines[i].trim());
        i += 1;
    }
    if let Some((u, v)) = url.rsplit_once(' ') {
        if v.starts_with("HTTP/") {
            url = u.trim().to_string();
        }
    }
    req.method = method;
    req.url = url;

    while i < e {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            break;
        }
        if is_comment(line) {
            i += 1;
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            req.headers.push((k.trim().to_string(), v.trim().to_string()));
        }
        i += 1;
    }

    let mut body_lines: Vec<&str> = Vec::new();
    let mut in_script = false;
    while i < e {
        let line = lines[i];
        let t = line.trim();
        if in_script {
            if t.contains("%}") {
                in_script = false;
            }
            i += 1;
            continue;
        }
        if t.starts_with("> {%") || t.starts_with(">{%") {
            in_script = !t.contains("%}");
            i += 1;
            continue;
        }
        if let Some(p) = t.strip_prefix(">>!") {
            req.redirect = Some((p.trim().to_string(), true));
        } else if let Some(p) = t.strip_prefix(">>") {
            req.redirect = Some((p.trim().to_string(), false));
        } else if t.starts_with("> ") {
        } else if body_lines.is_empty() && t.starts_with("< ") {
            req.body = Some(Body::File(t[2..].trim().to_string()));
        } else if !(body_lines.is_empty() && is_comment(line) && comment_text(line).starts_with('@')) {
            body_lines.push(line);
        }
        i += 1;
    }
    while body_lines.last().is_some_and(|l| l.trim().is_empty()) {
        body_lines.pop();
    }
    if req.body.is_none() && !body_lines.is_empty() {
        req.body = Some(Body::Inline(body_lines.join("\n")));
    }
    Some(req)
}
