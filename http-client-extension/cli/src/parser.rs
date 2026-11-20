//! Parsing `.http` files: file variables and requests, with their `# @directive` comments.

#[derive(Debug, Clone, Default)]
pub struct Request {
    pub name: Option<String>,
    pub title: Option<String>,
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Body>,
    pub redirect: Option<Redirect>,
    pub start_line: usize,
    pub end_line: usize,
    pub no_redirect: bool,
    /// `# @no-log`: the response is neither saved to `.zed-http/` nor kept for later runs.
    pub no_log: bool,
    /// The request's own `# @env NAME`; it wins over the file's.
    pub env: Option<EnvDirective>,
    /// Below a `###` of its own, rather than in the file's header (where `# @env` is the file's).
    pub own_block: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    Inline(String),
    File(String),
}

/// A `# @env NAME` line: the environment it names and its line (0-based).
#[derive(Debug, Clone, PartialEq)]
pub struct EnvDirective {
    pub line: usize,
    pub name: String,
}

/// `>> path` or `>>! path` below a request: where its response body is written.
#[derive(Debug, Clone, PartialEq)]
pub struct Redirect {
    pub path: String,
    /// `>>!`: an existing file is overwritten, instead of the response going to a new name.
    pub overwrite: bool,
}

/// An `@name = value` file variable.
#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Default)]
pub struct HttpFile {
    pub variables: Vec<Variable>,
    pub requests: Vec<Request>,
}

impl HttpFile {
    /// The file variables as `(name, value)` pairs, to extend a variable map with.
    pub fn variable_pairs(&self) -> impl Iterator<Item = (String, String)> + '_ {
        self.variables.iter().map(|variable| (variable.name.clone(), variable.value.clone()))
    }
}

/// A `# @directive` comment above a request.
struct Directive {
    /// `@name`, `@no-log`…: a comment whose first word it is is this directive.
    word: &'static str,
    /// Applies the directive to the request; `rest` is the comment after the word, `line` its line.
    apply: fn(request: &mut Request, rest: &str, line: usize),
}

/// The directives a request understands.
const DIRECTIVES: [Directive; 4] = [
    Directive { word: "@name", apply: name_directive },
    Directive { word: "@no-redirect", apply: |request, _, _| request.no_redirect = true },
    Directive { word: "@no-log", apply: |request, _, _| request.no_log = true },
    Directive { word: "@env", apply: env_of_request },
];

/// What follows the directive `word` at the start of `comment` when the comment's first word is
/// that one (`@name login`, `@name=login`, `@no-log`); None for another word (`@namespace`,
/// `@no-logging`).
pub fn directive_argument<'a>(comment: &'a str, word: &str) -> Option<&'a str> {
    let rest = comment.strip_prefix(word)?;
    match rest.chars().next() {
        Some(next) if !next.is_whitespace() && next != '=' => None,
        _ => Some(rest),
    }
}

/// The text of a `#` or `//` comment line, without its marker; None for any other line.
pub fn comment_of(line: &str) -> Option<&str> {
    is_comment(line).then(|| comment_text(line))
}

/// `# @name login`: the request's name, used by `--name` and by response references.
fn name_directive(request: &mut Request, rest: &str, _line: usize) {
    let name = rest.trim_start_matches(|c: char| c == '=' || c.is_whitespace()).trim();
    if !name.is_empty() {
        request.name = Some(name.to_string());
    }
}

/// `# @env dev`: the request's own environment; the first one counts.
fn env_of_request(request: &mut Request, rest: &str, line: usize) {
    if request.env.is_none() {
        request.env = env_name(rest).map(|name| EnvDirective { line, name });
    }
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

/// The environment a whole file runs in: `# @env NAME` (or `// @env NAME`) above its first `###`.
/// The first such line counts. Below a `###`, the same line belongs to that request only (see
/// [`Request::env`]).
pub fn file_env(src: &str) -> Option<EnvDirective> {
    src.lines()
        .enumerate()
        .take_while(|(_, line)| !is_separator(line))
        .find_map(|(line, text)| Some(EnvDirective { line, name: env_directive(text)? }))
}

/// `NAME` of a `# @env NAME` line.
fn env_directive(line: &str) -> Option<String> {
    if !is_comment(line) {
        return None;
    }
    env_name(comment_text(line).strip_prefix("@env")?)
}

/// `NAME` of what follows `@env`: ` NAME`, `=NAME`, ` = NAME`.
fn env_name(rest: &str) -> Option<String> {
    let name = rest.strip_prefix([' ', '\t', '='])?.trim_start_matches(['=', ' ', '\t']).split_whitespace().next()?;
    Some(name.to_string())
}

/// The request whose block holds `line` (1-based, like `--line` and `$ZED_ROW`).
pub fn request_at(file: &HttpFile, line: usize) -> Option<&Request> {
    file.requests.iter().find(|r| line >= r.start_line && line <= r.end_line)
}

/// Parses an `@name = value` declaration.
pub fn parse_var_decl(line: &str) -> Option<Variable> {
    let t = line.trim();
    let rest = t.strip_prefix('@')?;
    let (name, value) = rest.split_once('=')?;
    let name = name.trim();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return None;
    }
    Some(Variable { name: name.to_string(), value: value.trim().to_string() })
}

/// The lines of a file between two `###` separators (or before the first one).
struct Section {
    start: usize,
    end: usize,
    title: Option<String>,
    /// Below a `###`, rather than the file's header before the first one.
    separated: bool,
}

pub fn parse(src: &str) -> HttpFile {
    let lines: Vec<&str> = src.lines().collect();
    let mut file = HttpFile::default();

    // What comes before the first `###` is the file's header.
    let mut sections: Vec<Section> = Vec::new();
    let mut start = 0;
    let mut title = None;
    let mut separated = false;
    for (index, line) in lines.iter().enumerate() {
        if is_separator(line) {
            if index > start || title.is_some() {
                sections.push(Section { start, end: index, title: title.take(), separated });
            }
            let text = line.trim_start().trim_start_matches('#').trim();
            title = (!text.is_empty()).then(|| text.to_string());
            start = index + 1;
            separated = true;
        }
    }
    sections.push(Section { start, end: lines.len(), title, separated });

    for section in sections {
        let separated = section.separated;
        if let Some(mut req) = parse_section(&lines, section, &mut file.variables) {
            req.own_block = separated;
            if !separated {
                // An `# @env` in the header is the whole file's.
                req.env = None;
            }
            file.requests.push(req);
        }
    }
    file
}

fn parse_section(lines: &[&str], section: Section, vars: &mut Vec<Variable>) -> Option<Request> {
    let Section { start: s, end: e, title, .. } = section;
    let mut req = Request { title, start_line: s.max(1), end_line: e, ..Default::default() };

    let mut i = s;
    while i < e {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        if is_comment(line) {
            let comment = comment_text(line);
            let found =
                DIRECTIVES.iter().find_map(|directive| Some((directive, directive_argument(comment, directive.word)?)));
            if let Some((directive, rest)) = found {
                (directive.apply)(&mut req, rest, i);
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
        if let Some(path) = t.strip_prefix(">>!") {
            req.redirect = Some(Redirect { path: path.trim().to_string(), overwrite: true });
        } else if let Some(path) = t.strip_prefix(">>") {
            req.redirect = Some(Redirect { path: path.trim().to_string(), overwrite: false });
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
