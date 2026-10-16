//! The pattern language used in `frameworks.json`.
//!
//! A pattern is source code with typed placeholders, e.g. `@{method}Mapping("{path}")`:
//!
//! | Token        | Matches                                                                |
//! |--------------|------------------------------------------------------------------------|
//! | `{method}`   | an HTTP method word, any case (`get`, `Post`, `DELETE`)                |
//! | `{path}`     | the contents of a string literal                                       |
//! | `{route}`    | string contents starting with `/` or `http(s)://`                      |
//! | `{host}`     | string contents without `/` (a host name)                              |
//! | `{name}`     | an identifier or header name (`page`, `X-Request-Id`)                  |
//! | `{type}`     | a type expression (`int`, `List<String>`, `list[str]`, `Dto?`)         |
//! | `{var}`      | a variable name (its declared type is looked up)                       |
//! | `{handler}`  | a function name (its definition is included in the search window)      |
//! | `{keys}`     | a `{...}` / `[...]` literal; its keys or destructured names are taken  |
//! | `{generic}`  | an optional `<...>` type argument list                                 |
//! | `{_}`        | any identifier, ignored                                                |
//! | `{x?}`       | any placeholder made optional                                          |
//! | `...`        | anything on the same line, except parentheses                          |
//! | `"`          | any quote: `"`, `'` or `` ` ``; an opening one also allows a string      |
//! |              | prefix (`$"…"`, `f"…"`, `r"…"`)                                        |
//! | whitespace   | optional whitespace (required between two words), may span lines      |
//! |              | whitespace is also allowed between any two tokens outside strings,     |
//! |              | so `("{path}")` matches `( "/x" )`                                     |
//! | `?`          | an optional `?`                                                        |
//! | `^` (first)  | start of a line                                                        |
//! | `{{` / `}}`  | literal `{` / `}`                                                      |

use regex::Regex;
use std::collections::HashMap;

/// The regex each placeholder expands to, or `None` if the name is not a placeholder.
fn class_of(name: &str) -> Option<&'static str> {
    Some(match name {
        "method" => r"(?i:get|post|put|delete|patch|head|options)",
        "path" => r#"[^"'`\n]*"#,
        "route" => r#"(?:/|https?://)[^"'`\n]*"#,
        "host" => r#"[^"'`\n/]+"#,
        "name" => r"[A-Za-z_$][\w$-]*",
        "type" => r"(?:\[\]|\*|&|map\[[^\]\n]*\])*[\w.$]+(?:<[^()=;{}\n]*>|\[[^\]\n]*\])?(?:\[\])*\??",
        "var" | "handler" | "_" => r"[A-Za-z_$][\w$]*",
        "keys" => r"(?:<[^>\n]*>\s*)?(?:\{[^{}]*\}|\[[^\[\]]*\])",
        "generic" => r"(?:<[^()\n]*>)?",
        _ => return None,
    })
}

/// Placeholder names, for error messages.
pub const PLACEHOLDERS: &[&str] =
    &["method", "path", "route", "host", "name", "type", "var", "handler", "keys", "generic", "_"];

/// Several patterns compiled into one matcher. At any position the first listed pattern wins.
#[derive(Debug)]
pub struct Matcher {
    re: Regex,
    /// Captured placeholder names per alternative.
    alts: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub alt: usize,
    pub start: usize,
    pub end: usize,
    pub caps: HashMap<String, String>,
}

impl Hit {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.caps.get(name).map(String::as_str)
    }
}

impl Matcher {
    /// Compiles `patterns`; `bind` replaces placeholders with literal text (e.g. `{var}` → `item`).
    pub fn new(patterns: &[String], bind: &[(&str, &str)]) -> Result<Option<Self>, String> {
        if patterns.is_empty() {
            return Ok(None);
        }
        let mut src = Vec::new();
        let mut alts = Vec::new();
        for (i, p) in patterns.iter().enumerate() {
            let (re, names) = compile(p, i, bind).map_err(|e| format!("pattern `{p}`: {e}"))?;
            src.push(format!("(?P<a{i}>){re}"));
            alts.push(names);
        }
        let joined = src.iter().map(|s| format!("(?:{s})")).collect::<Vec<_>>().join("|");
        let re = Regex::new(&joined).map_err(|e| format!("patterns {patterns:?}: {e}"))?;
        Ok(Some(Self { re, alts }))
    }

    /// Compiles a single pattern.
    pub fn one(pattern: &str, bind: &[(&str, &str)]) -> Result<Option<Self>, String> {
        Self::new(&[pattern.to_string()], bind)
    }

    pub fn hits<'a>(&'a self, text: &'a str) -> impl Iterator<Item = Hit> + 'a {
        self.re.captures_iter(text).filter_map(move |c| {
            let alt = (0..self.alts.len()).find(|i| c.name(&format!("a{i}")).is_some())?;
            let whole = c.get(0)?;
            let caps = self.alts[alt]
                .iter()
                .filter_map(|n| c.name(&format!("a{alt}_{n}")).map(|m| (n.clone(), m.as_str().to_string())))
                .collect();
            Some(Hit { alt, start: whole.start(), end: whole.end(), caps })
        })
    }

    pub fn first(&self, text: &str) -> Option<Hit> {
        self.hits(text).next()
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.re.is_match(text)
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Placeholders that match word characters at their edges.
fn wordy(name: &str) -> bool {
    matches!(name, "method" | "name" | "type" | "var" | "handler" | "_")
}

enum Tok {
    Lit(char),
    Space,
    Any,
    Ph(String, bool),
}

fn tokenize(p: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = p.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '{' && chars.get(i + 1) == Some(&'{') {
            out.push(Tok::Lit('{'));
            i += 2;
        } else if c == '}' && chars.get(i + 1) == Some(&'}') {
            out.push(Tok::Lit('}'));
            i += 2;
        } else if c == '{' {
            let end = chars[i..].iter().position(|&c| c == '}').ok_or("unclosed `{`")? + i;
            let inner: String = chars[i + 1..end].iter().collect();
            let (name, optional) = match inner.strip_suffix('?') {
                Some(n) => (n.to_string(), true),
                None => (inner, false),
            };
            if class_of(&name).is_none() {
                return Err(format!("unknown placeholder `{{{name}}}`, expected one of {PLACEHOLDERS:?}"));
            }
            out.push(Tok::Ph(name, optional));
            i = end + 1;
        } else if c.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            out.push(Tok::Space);
        } else if chars[i..].starts_with(&['.', '.', '.']) {
            out.push(Tok::Any);
            i += 3;
        } else {
            out.push(Tok::Lit(c));
            i += 1;
        }
    }
    Ok(out)
}

/// Does the token start / end with a word character?
fn starts_word(t: &Tok) -> bool {
    match t {
        Tok::Lit(c) => is_word(*c),
        Tok::Ph(n, _) => wordy(n),
        _ => false,
    }
}

fn compile(p: &str, alt: usize, bind: &[(&str, &str)]) -> Result<(String, Vec<String>), String> {
    let toks = tokenize(p)?;
    let mut re = String::new();
    let mut names: Vec<String> = Vec::new();
    let mut in_str = false;
    for (i, t) in toks.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| &toks[j]);
        let next = toks.get(i + 1);
        // Implicit optional whitespace between tokens, except inside string literals and between words.
        if let Some(p) = prev {
            let spaced = matches!(p, Tok::Space) || matches!(t, Tok::Space);
            let words = starts_word(p) && starts_word(t);
            let line_start = i == 1 && matches!(p, Tok::Lit('^'));
            if !in_str && !spaced && !words && !line_start {
                re.push_str(r"\s*");
            }
        }
        match t {
            Tok::Lit('^') if i == 0 => re.push_str(r"(?m:^)[ \t]*"),
            // An opening quote may carry an interpolation/raw prefix: $"…" (C#), f"…" (Python), r"…".
            Tok::Lit('"') if !in_str => re.push_str(r#"[$fFrRbBuU]{0,2}["'`]"#),
            Tok::Lit('"') => re.push_str(r#"["'`]"#),
            Tok::Lit('?') => re.push_str(r"\??"),
            Tok::Lit(c) => re.push_str(&regex::escape(&c.to_string())),
            Tok::Any => re.push_str(r"[^()\n]*?"),
            Tok::Space => {
                let between_words = prev.is_some_and(starts_word) && next.is_some_and(starts_word);
                re.push_str(if between_words { r"\s+" } else { r"\s*" });
            }
            Tok::Ph(name, optional) => {
                let body = if let Some((_, v)) = bind.iter().find(|(k, _)| k == name) {
                    regex::escape(v)
                } else {
                    let class = class_of(name).expect("validated by tokenize");
                    if name == "_" || name == "generic" || names.contains(name) {
                        format!("(?:{class})")
                    } else {
                        names.push(name.clone());
                        format!("(?P<a{alt}_{name}>{class})")
                    }
                };
                // `{type}` can end in `>`/`]`/`?`, so it gets no trailing boundary.
                let w = wordy(name) && name != "type";
                let before = w && !prev.is_some_and(starts_word) && !matches!(prev, Some(Tok::Space));
                let after = w && !next.is_some_and(starts_word);
                let mut piece = String::new();
                if before {
                    piece.push_str(r"\b");
                }
                piece.push_str(&body);
                if after {
                    piece.push_str(r"\b");
                }
                if *optional {
                    re.push_str(&format!("(?:{piece})?"));
                } else {
                    re.push_str(&piece);
                }
            }
        }
        if matches!(t, Tok::Lit('"')) {
            in_str = !in_str;
        }
    }
    Ok((re, names))
}

/// Keys of a `{keys}` capture: quoted keys (`'a': 1`, `"a" => 1`) or destructured names (`{ a, b: c }`).
pub fn keys_of(literal: &str) -> Vec<String> {
    let inner = literal
        .trim()
        .trim_start_matches(|c| c != '{' && c != '[')
        .trim_start_matches(['{', '['])
        .trim_end_matches(['}', ']']);
    let quoted = Regex::new(r#"["']([\w-]+)["']\s*(?::|=>)"#).unwrap();
    let keys: Vec<String> = quoted.captures_iter(inner).map(|c| c[1].to_string()).collect();
    if !keys.is_empty() {
        return keys;
    }
    inner
        .split(',')
        .map(|p| p.split([':', '=']).next().unwrap_or("").trim().trim_start_matches("...").to_string())
        .filter(|k| !k.is_empty() && k.chars().all(|c| is_word(c) || c == '$'))
        .collect()
}
