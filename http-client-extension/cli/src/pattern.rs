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
//! | `{ref}`      | a variable holding the URL (its value is looked up; `this.url`)        |
//! | `{expr}`     | the rest of an expression, up to `;` or the end of the line            |
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
use std::sync::LazyLock;

/// A placeholder of the pattern language and the regex it expands to.
pub struct Placeholder {
    pub name: &'static str,
    regex: &'static str,
}

/// Every placeholder; a name not listed here is an error in the pattern.
pub const PLACEHOLDERS: [Placeholder; 13] = [
    Placeholder { name: "method", regex: r"(?i:get|post|put|delete|patch|head|options)" },
    Placeholder { name: "path", regex: r#"[^"'`\n]*"# },
    Placeholder { name: "route", regex: r#"(?:/|https?://)[^"'`\n]*"# },
    Placeholder { name: "host", regex: r#"[^"'`\n/]+"# },
    Placeholder { name: "name", regex: r"[A-Za-z_$][\w$-]*" },
    Placeholder {
        name: "type",
        regex: r"(?:\[\]|\*|&|map\[[^\]\n]*\])*[\w.$]+(?:<[^()=;{}\n]*>|\[[^\]\n]*\])?(?:\[\])*\??",
    },
    Placeholder { name: "var", regex: IDENTIFIER },
    Placeholder { name: "ref", regex: r"[A-Za-z_$@][\w$]*(?:\.[A-Za-z_$][\w$]*)*" },
    Placeholder { name: "expr", regex: r"[^\n;]*" },
    Placeholder { name: "handler", regex: IDENTIFIER },
    Placeholder {
        name: "keys",
        regex: r"(?:<[^>\n]*>\s*)?(?:\{(?:[^{}]|\{[^{}]*\})*\}|\[(?:[^\[\]]|\[[^\[\]]*\])*\])",
    },
    Placeholder { name: "generic", regex: r"(?:<[^()\n]*>)?" },
    Placeholder { name: "_", regex: IDENTIFIER },
];

/// What `{var}`, `{handler}` and `{_}` match.
const IDENTIFIER: &str = r"[A-Za-z_$][\w$]*";

/// A quoted key of an object literal: `'a': 1`, `"a" => 1`.
static QUOTED_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^["']([\w-]+)["']\s*(?::|=>)"#).expect("the quoted key pattern is a valid regex"));

/// The regex each placeholder expands to, or `None` if the name is not a placeholder.
fn class_of(name: &str) -> Option<&'static str> {
    PLACEHOLDERS.iter().find(|placeholder| placeholder.name == name).map(|placeholder| placeholder.regex)
}

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
        for (index, pattern) in patterns.iter().enumerate() {
            let compiled = compile(pattern, index, bind).map_err(|error| format!("pattern `{pattern}`: {error}"))?;
            src.push(format!("(?P<a{index}>){}", compiled.regex));
            alts.push(compiled.captures);
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

enum Token {
    Literal(char),
    Space,
    Any,
    Placeholder { name: String, optional: bool },
}

fn tokenize(pattern: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '{' && chars.get(i + 1) == Some(&'{') {
            out.push(Token::Literal('{'));
            i += 2;
        } else if c == '}' && chars.get(i + 1) == Some(&'}') {
            out.push(Token::Literal('}'));
            i += 2;
        } else if c == '{' {
            let end = chars[i..].iter().position(|&c| c == '}').ok_or("unclosed `{`")? + i;
            let inner: String = chars[i + 1..end].iter().collect();
            let (name, optional) = match inner.strip_suffix('?') {
                Some(name) => (name.to_string(), true),
                None => (inner, false),
            };
            if class_of(&name).is_none() {
                let names: Vec<&str> = PLACEHOLDERS.iter().map(|placeholder| placeholder.name).collect();
                return Err(format!("unknown placeholder `{{{name}}}`, expected one of {names:?}"));
            }
            out.push(Token::Placeholder { name, optional });
            i = end + 1;
        } else if c.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            out.push(Token::Space);
        } else if chars[i..].starts_with(&['.', '.', '.']) {
            out.push(Token::Any);
            i += 3;
        } else {
            out.push(Token::Literal(c));
            i += 1;
        }
    }
    Ok(out)
}

/// Does the token start / end with a word character?
fn starts_word(token: &Token) -> bool {
    match token {
        Token::Literal(c) => is_word(*c),
        Token::Placeholder { name, .. } => wordy(name),
        _ => false,
    }
}

/// One pattern as a regex alternative, and the placeholders it captures.
struct CompiledPattern {
    regex: String,
    captures: Vec<String>,
}

fn compile(pattern: &str, alt: usize, bind: &[(&str, &str)]) -> Result<CompiledPattern, String> {
    let tokens = tokenize(pattern)?;
    let mut re = String::new();
    let mut names: Vec<String> = Vec::new();
    let mut in_str = false;
    for (index, token) in tokens.iter().enumerate() {
        let prev = index.checked_sub(1).map(|before| &tokens[before]);
        let next = tokens.get(index + 1);
        // Implicit optional whitespace between tokens, except inside string literals and between words.
        if let Some(previous) = prev {
            let spaced = matches!(previous, Token::Space) || matches!(token, Token::Space);
            let words = starts_word(previous) && starts_word(token);
            let line_start = index == 1 && matches!(previous, Token::Literal('^'));
            if !in_str && !spaced && !words && !line_start {
                re.push_str(r"\s*");
            }
        }
        match token {
            Token::Literal('^') if index == 0 => re.push_str(r"(?m:^)[ \t]*"),
            // An opening quote may carry an interpolation/raw prefix: $"…" (C#), f"…" (Python), r"…".
            Token::Literal('"') if !in_str => re.push_str(r#"[$fFrRbBuU]{0,2}["'`]"#),
            Token::Literal('"') => re.push_str(r#"["'`]"#),
            Token::Literal('?') => re.push_str(r"\??"),
            Token::Literal(c) => re.push_str(&regex::escape(&c.to_string())),
            Token::Any => re.push_str(r"[^()\n]*?"),
            Token::Space => {
                let between_words = prev.is_some_and(starts_word) && next.is_some_and(starts_word);
                re.push_str(if between_words { r"\s+" } else { r"\s*" });
            }
            Token::Placeholder { name, optional } => {
                let body = if let Some((_, value)) = bind.iter().find(|(bound, _)| bound == name) {
                    regex::escape(value)
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
                let bounded = wordy(name) && name != "type";
                let before = bounded && !prev.is_some_and(starts_word) && !matches!(prev, Some(Token::Space));
                let after = bounded && !next.is_some_and(starts_word);
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
        if matches!(token, Token::Literal('"')) {
            in_str = !in_str;
        }
    }
    Ok(CompiledPattern { regex: re, captures: names })
}

/// Keys of a `{keys}` capture: quoted keys (`'a': 1`, `"a" => 1`) or destructured names (`{ a, b: c }`).
/// Only the top level, so the keys of a nested object are not read as keys of this one.
pub fn keys_of(literal: &str) -> Vec<String> {
    let inner = literal
        .trim()
        .trim_start_matches(|c| c != '{' && c != '[')
        .trim_start_matches(['{', '['])
        .trim_end_matches(['}', ']']);
    let parts = fields(inner);
    let keys: Vec<String> =
        parts.iter().filter_map(|part| QUOTED_KEY.captures(part.trim())).map(|key| key[1].to_string()).collect();
    if !keys.is_empty() {
        return keys;
    }
    parts
        .iter()
        .map(|p| p.split([':', '=']).next().unwrap_or("").trim().trim_start_matches("...").to_string())
        .filter(|k| !k.is_empty() && k.chars().all(|c| is_word(c) || c == '$'))
        .collect()
}

/// Splits on the commas of the outermost level only.
fn fields(inner: &str) -> Vec<&str> {
    let (mut depth, mut start, mut out) = (0i32, 0, Vec::new());
    for (i, c) in inner.char_indices() {
        match c {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            ',' if depth <= 0 => {
                out.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&inner[start..]);
    out
}
