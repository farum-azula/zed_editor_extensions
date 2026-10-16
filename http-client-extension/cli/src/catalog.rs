//! The catalog of supported languages and frameworks (`frameworks.json`) and the engine that
//! applies it: finding routes / HTTP calls in source code and inferring their requests.
//!
//! The built-in catalog is embedded at compile time; a project can extend or override it with
//! `.zed/http/frameworks.json` (merged key by key: languages → frameworks → fields).

use crate::endpoint::{self, Endpoint};
use crate::infer::{Spec, Ty};
use crate::pattern::{keys_of, Hit, Matcher};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const BUILTIN: &str = include_str!("../../frameworks.json");
pub const PROJECT_FILE: &str = "frameworks.json";

/// How many lines after a route (or handler definition) belong to it.
const WINDOW: usize = 30;

// ---------------------------------------------------------------------------------------------
// JSON shape
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileDef {
    #[serde(rename = "$schema", default)]
    _schema: Option<String>,
    #[serde(default)]
    types: TypesDef,
    #[serde(default)]
    auth: Vec<String>,
    #[serde(default)]
    urls: UrlsDef,
    languages: serde_json::Map<String, Value>,
}

/// How to read the URL out of a variable, for calls that pass one (`http.post(_endpoint, …)`).
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct UrlsDef {
    /// Patterns binding `{var}` and capturing the `{expr}` assigned to it.
    #[serde(default)]
    assignments: Vec<String>,
    /// Patterns capturing a URL `{path}` in such an expression.
    #[serde(default)]
    literals: Vec<String>,
    /// Patterns capturing the `{name}` of an environment variable read in such an expression.
    #[serde(default)]
    env: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct TypesDef {
    #[serde(default)]
    int: Vec<String>,
    #[serde(default)]
    float: Vec<String>,
    #[serde(default)]
    bool: Vec<String>,
    #[serde(default)]
    string: Vec<String>,
    #[serde(default)]
    list: Vec<String>,
    #[serde(default)]
    object: Vec<String>,
    #[serde(default)]
    wrappers: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LanguageDef {
    name: String,
    #[serde(default)]
    zed: Vec<String>,
    extensions: Vec<String>,
    #[serde(default)]
    decorators: Vec<String>,
    #[serde(default)]
    class_keywords: Vec<String>,
    #[serde(default)]
    definitions: Vec<String>,
    #[serde(default)]
    handler_refs: Vec<String>,
    #[serde(default)]
    variables: Vec<String>,
    #[serde(default)]
    models: Option<ModelsDef>,
    frameworks: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelsDef {
    declarations: Vec<String>,
    #[serde(default)]
    block: Block,
    #[serde(default)]
    modifiers: Vec<String>,
    fields: Vec<String>,
}

#[derive(Deserialize, Default, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Block {
    #[default]
    Braces,
    Indent,
}

#[derive(Deserialize, Default, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    Class,
    File,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameworkDef {
    name: String,
    #[serde(default)]
    docs: Option<String>,
    #[serde(default)]
    imports: Vec<String>,
    #[serde(default)]
    prefix: Vec<String>,
    #[serde(default)]
    prefix_scope: Scope,
    #[serde(default)]
    controller_token: Option<String>,
    routes: Vec<RouteSpec>,
    #[serde(default)]
    path_params: Vec<Style>,
    #[serde(default)]
    query: Vec<String>,
    #[serde(default)]
    headers: Vec<String>,
    #[serde(default)]
    header_case: Option<HeaderCase>,
    #[serde(default)]
    body: Vec<String>,
    #[serde(default)]
    params: Vec<String>,
    #[serde(default)]
    ignore: Vec<String>,
    #[serde(default)]
    auth: Vec<String>,
    #[serde(default)]
    examples: Vec<Example>,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum RouteSpec {
    Pattern(String),
    Full(RouteDef),
}

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(deny_unknown_fields)]
pub struct RouteDef {
    pub pattern: String,
    /// Method when the pattern has no `{method}`; default `GET`.
    #[serde(default)]
    pub method: Option<String>,
    /// URL template from captures, e.g. `https://{host}{path}`; default `{path}` / `{route}`.
    #[serde(default)]
    pub url: Option<String>,
    /// Apply the framework prefix (default true when the framework has one).
    #[serde(default)]
    pub prefixed: Option<bool>,
    /// A low-confidence match: offered as a code action, but no lens.
    #[serde(default)]
    pub weak: bool,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// `{id}`, `{id:int}`, `{id?}`
    Braces,
    /// `:id`
    Colon,
    /// `<id>`, `<int:id>`
    Angle,
    /// `$id`, `${id}`, `${user.id}`
    Dollar,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum HeaderCase {
    /// `x_request_id` → `X-Request-Id`
    Kebab,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Example {
    pub code: String,
    pub expect: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Compiled catalog
// ---------------------------------------------------------------------------------------------

pub struct Catalog {
    types: TypesDef,
    auth: Option<Matcher>,
    urls: Urls,
    pub languages: Vec<Language>,
}

/// Compiled [`UrlsDef`]. Assignment patterns are compiled per lookup, with `{var}` bound.
struct Urls {
    assignments: Vec<String>,
    literals: Option<Matcher>,
    env: Option<Matcher>,
}

#[cfg_attr(not(test), allow(dead_code))]
pub struct Language {
    pub key: String,
    pub name: String,
    pub zed: Vec<String>,
    pub extensions: Vec<String>,
    decorators: Vec<String>,
    class_keywords: Option<Regex>,
    definitions: Option<Matcher>,
    definition_patterns: Vec<String>,
    handler_refs: Option<Matcher>,
    variables: Vec<String>,
    models: Option<Models>,
    pub frameworks: Vec<Framework>,
}

struct Models {
    declarations: Vec<String>,
    block: Block,
    modifiers: Vec<String>,
    fields: Matcher,
}

#[cfg_attr(not(test), allow(dead_code))]
pub struct Framework {
    pub key: String,
    pub name: String,
    pub docs: Option<String>,
    pub imports: Vec<String>,
    prefix: Option<Matcher>,
    prefix_scope: Scope,
    controller_token: Option<String>,
    routes: Matcher,
    route_defs: Vec<RouteDef>,
    path_params: Vec<Style>,
    query: Option<Matcher>,
    headers: Option<Matcher>,
    header_case: Option<HeaderCase>,
    body: Option<Matcher>,
    params: Option<Matcher>,
    ignore: Option<Matcher>,
    auth: Option<Matcher>,
    pub examples: Vec<Example>,
}

/// A route or HTTP call found in a source file.
#[derive(Debug, Clone)]
pub struct Route {
    /// 0-based line of the match start.
    pub line: usize,
    /// 0-based line of the match end (a call may span several lines).
    end_line: usize,
    pub endpoint: Endpoint,
    /// `language/framework` key, e.g. `java/spring`.
    pub framework: String,
    lang: usize,
    fw: usize,
    hit: Hit,
}

fn compile(patterns: &[String], what: &str) -> Result<Option<Matcher>, String> {
    Matcher::new(patterns, &[]).map_err(|e| format!("{what}: {e}"))
}

/// Recursively merges `over` into `base`: objects key by key, everything else replaced.
fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(slot) => merge(slot, v),
                    None => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (b, o) => *b = o,
    }
}

impl Catalog {
    /// The built-in catalog. It is validated by tests, so failing here is a build defect.
    pub fn builtin() -> &'static Catalog {
        static C: OnceLock<Catalog> = OnceLock::new();
        C.get_or_init(|| Catalog::from_sources(&[BUILTIN]).expect("invalid built-in frameworks.json"))
    }

    /// Built-in catalog extended with `<root>/.zed/http/frameworks.json`, if present.
    pub fn for_root(root: &Path) -> Result<Catalog, String> {
        let path = project_file(root);
        let extra = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Catalog::from_sources(&[BUILTIN, &extra]).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn from_sources(sources: &[&str]) -> Result<Catalog, String> {
        let mut merged = Value::Null;
        for (i, src) in sources.iter().enumerate() {
            let v: Value = serde_json::from_str(src).map_err(|e| format!("invalid JSON: {e}"))?;
            if i == 0 {
                merged = v;
            } else {
                merge(&mut merged, v);
            }
        }
        let file: FileDef = serde_json::from_value(merged).map_err(|e| e.to_string())?;
        let mut languages = Vec::new();
        for (key, v) in file.languages {
            let l: LanguageDef = serde_json::from_value(v).map_err(|e| format!("languages.{key}: {e}"))?;
            languages.push(Language::compile(&key, l)?);
        }
        let urls = Urls {
            assignments: file.urls.assignments,
            literals: compile(&file.urls.literals, "urls.literals")?,
            env: compile(&file.urls.env, "urls.env")?,
        };
        Ok(Catalog { auth: compile(&file.auth, "auth")?, types: file.types, urls, languages })
    }

    fn language_for(&self, path: &Path) -> Option<usize> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();
        self.languages.iter().position(|l| l.extensions.iter().any(|e| e == &ext))
    }

    fn any_language(&self) -> Option<usize> {
        self.languages.iter().position(|l| l.extensions.iter().any(|e| e == "*"))
    }

    /// Routes and HTTP calls in `text`, one per line, sorted by line.
    pub fn routes(&self, text: &str, path: &Path) -> Vec<Route> {
        let mut candidates: Vec<(usize, usize)> = Vec::new();
        for l in [self.language_for(path), self.any_language()].into_iter().flatten() {
            for (f, fw) in self.languages[l].frameworks.iter().enumerate() {
                if fw.imports.is_empty() || fw.imports.iter().any(|i| text.contains(i.as_str())) {
                    candidates.push((l, f));
                }
            }
        }
        let starts = line_starts(text);
        let mut by_line: BTreeMap<usize, Route> = BTreeMap::new();
        for (l, f) in candidates {
            let lang = &self.languages[l];
            let fw = &lang.frameworks[f];
            for hit in fw.routes.hits(text) {
                let line = line_of(&starts, hit.start);
                // A class-level prefix (`@RequestMapping("/api")` above a class) is not a route.
                if fw.prefix.as_ref().is_some_and(|p| p.first(&text[hit.start..]).is_some_and(|h| h.start == 0))
                    && fw.prefix_scope == Scope::Class
                    && lang.is_class_level(text, &starts, line)
                {
                    continue;
                }
                let end_line = line_of(&starts, hit.end.saturating_sub(1));
                let def = &fw.route_defs[hit.alt];
                let Some(endpoint) = self.endpoint(lang, fw, def, &hit, text, &starts, line) else { continue };
                let replace = match by_line.get(&line) {
                    None => true,
                    Some(r) => r.endpoint.weak && !endpoint.weak,
                };
                if replace {
                    let framework = format!("{}/{}", lang.key, fw.key);
                    by_line.insert(line, Route { line, end_line, endpoint, framework, lang: l, fw: f, hit });
                }
            }
        }
        // A URL literal inside a recognized multi-line call is the same request, not another one.
        let strong: Vec<(usize, usize)> =
            by_line.values().filter(|r| !r.endpoint.weak).map(|r| (r.line, r.end_line)).collect();
        by_line.retain(|line, r| !r.endpoint.weak || !strong.iter().any(|(s, e)| line >= s && line <= e));
        by_line.into_values().collect()
    }

    /// The endpoint a route matched, or `None` when the match turns out not to be a request:
    /// a `{ref}` pattern whose variable holds no URL (`map.get(key)`).
    #[allow(clippy::too_many_arguments)]
    fn endpoint(
        &self,
        lang: &Language,
        fw: &Framework,
        def: &RouteDef,
        hit: &Hit,
        text: &str,
        starts: &[usize],
        line: usize,
    ) -> Option<Endpoint> {
        let method =
            hit.get("method").map(str::to_uppercase).or_else(|| def.method.clone()).unwrap_or_else(|| "GET".into());
        let mut path = match (&def.url, hit.get("ref")) {
            (Some(t), _) => fill(t, hit),
            (None, Some(r)) => self.url_of(text, r)?,
            (None, None) => hit.get("path").or(hit.get("route")).unwrap_or_default().to_string(),
        };
        let absolute = path.starts_with("http://") || path.starts_with("https://");
        if !absolute && def.prefixed.unwrap_or(true) {
            if let Some(prefix) = self.prefix(lang, fw, text, starts, line) {
                path = join(&prefix, &path);
            }
        }
        let path = convert_params(&path, &fw.path_params);
        Some(Endpoint { method, url: endpoint::to_http_url(&path), weak: def.weak })
    }

    /// The URL a variable holds: a literal or environment variable assigned to it anywhere in
    /// the file, or the variable itself when its name names a URL (`_endpoint` → `{{endpoint}}`,
    /// filled in from the environment file). `None` when neither holds, which is what tells a
    /// `{ref}` match apart from an unrelated call.
    fn url_of(&self, text: &str, reference: &str) -> Option<String> {
        let var = reference.rsplit('.').next().unwrap_or(reference).trim_start_matches(['$', '@']);
        if let Some(expr) = self.assigned(text, var) {
            let mut literals =
                self.urls.literals.iter().flat_map(|m| m.hits(&expr)).filter_map(|h| h.get("path").map(str::to_string));
            let literal = literals.find(|p| looks_like_url(p));
            let key =
                self.urls.env.iter().filter_map(|m| m.first(&expr)).find_map(|h| h.get("name").map(str::to_string));
            if let Some(url) = literal.or_else(|| key.map(|k| format!("{{{{{k}}}}}"))) {
                return Some(url);
            }
        }
        let name = var.trim_start_matches('_');
        endpoint::names_a_url(name).then(|| format!("{{{{{name}}}}}"))
    }

    /// The expression assigned to `var` anywhere in `text`.
    fn assigned(&self, text: &str, var: &str) -> Option<String> {
        let m = Matcher::new(&self.urls.assignments, &[("var", var)]).ok()??;
        let found = m.hits(text).find_map(|h| h.get("expr").map(str::to_string));
        found
    }

    fn prefix(&self, lang: &Language, fw: &Framework, text: &str, starts: &[usize], line: usize) -> Option<String> {
        let m = fw.prefix.as_ref()?;
        let found = match fw.prefix_scope {
            Scope::File => m.first(text),
            Scope::Class => m
                .hits(text)
                .filter(|h| {
                    let l = line_of(starts, h.start);
                    l < line && lang.is_class_level(text, starts, l)
                })
                .last(),
        }?;
        let mut prefix = found.get("path").unwrap_or_default().to_string();
        if let Some(token) = fw.controller_token.as_deref().filter(|t| prefix.contains(*t)) {
            let class = Regex::new(r"class\s+(\w+?)(?:Controller)?\b").unwrap();
            if let Some(c) = class.captures(&text[found.end..]) {
                prefix = prefix.replace(token, &c[1]);
            }
        }
        Some(prefix)
    }

    /// The route on `line` and the request inferred for it.
    pub fn request(&self, text: &str, path: &Path, line: usize) -> Option<(Endpoint, Spec, String)> {
        let routes = self.routes(text, path);
        let route = routes.iter().find(|r| r.line == line)?;
        let route_lines: BTreeSet<usize> = routes.iter().filter(|r| !r.endpoint.weak).map(|r| r.line).collect();
        let spec = self.infer(text, route, &route_lines);
        Some((route.endpoint.clone(), spec, route.framework.clone()))
    }

    fn infer(&self, text: &str, route: &Route, route_lines: &BTreeSet<usize>) -> Spec {
        let lang = &self.languages[route.lang];
        let fw = &lang.frameworks[route.fw];
        let lines: Vec<&str> = text.lines().collect();
        let mut window = lang.window(&lines, route.line, route_lines);
        let handler = route.hit.get("handler").map(str::to_string).or_else(|| {
            let line = lines.get(route.line)?;
            lang.handler_refs.as_ref()?.first(line)?.get("handler").map(str::to_string)
        });
        if let Some(def) = handler.and_then(|h| lang.find_definition(&lines, &h)) {
            window.push('\n');
            window.push_str(&lang.window(&lines, def, route_lines));
        }
        let w = window.as_str();
        let path_vars = endpoint::url_vars(&route.endpoint.url);
        let mut spec = Spec::default();
        let mut body: Vec<(String, Ty)> = Vec::new();
        let add = |v: &mut Vec<(String, Ty)>, name: &str, ty: Ty| {
            if !name.is_empty() && !v.iter().any(|(n, _)| n == name) {
                v.push((name.to_string(), ty));
            }
        };

        // Headers first: their parameter names must not be reported as query parameters.
        let mut taken: Vec<String> = Vec::new();
        for hit in fw.headers.iter().flat_map(|m| m.hits(w)) {
            let names = match hit.get("keys") {
                Some(k) => keys_of(k),
                None => hit.get("name").map(|n| vec![n.to_string()]).unwrap_or_default(),
            };
            for n in names {
                taken.push(n.clone());
                let header = match fw.header_case {
                    Some(HeaderCase::Kebab) => kebab_header(&n),
                    None => n,
                };
                if !spec.headers.iter().any(|h| h.eq_ignore_ascii_case(&header)) {
                    spec.headers.push(header);
                }
            }
        }
        taken.extend(fw.ignore.iter().flat_map(|m| m.hits(w)).filter_map(|h| h.get("name").map(str::to_string)));

        for (matcher, target) in [(&fw.query, 0), (&fw.body, 1)] {
            for hit in matcher.iter().flat_map(|m| m.hits(w)) {
                for (n, t) in self.fields_from(lang, text, w, &hit) {
                    add(if target == 0 { &mut spec.query } else { &mut body }, &n, t);
                }
            }
        }
        // Signature parameters (FastAPI-style): models go to the body, scalars to the query.
        for hit in fw.params.iter().flat_map(|m| m.hits(w)) {
            let Some(name) = hit.get("name") else { continue };
            if taken.iter().any(|t| t == name) || matches!(name, "self" | "cls") || path_vars.iter().any(|p| p == name)
            {
                continue;
            }
            match self.ty_of(lang, text, hit.get("type").unwrap_or("str"), 0) {
                Ty::Obj(fields) if !fields.is_empty() => fields.into_iter().for_each(|(n, t)| add(&mut body, &n, t)),
                ty => add(&mut spec.query, name, ty),
            }
        }

        spec.auth = [&self.auth, &fw.auth].iter().any(|m| m.as_ref().is_some_and(|m| m.is_match(w)))
            || spec.headers.iter().any(|h| h.eq_ignore_ascii_case("authorization"));
        // Authorization and Content-Type are rendered from the auth flag and the body instead.
        spec.headers.retain(|h| !["authorization", "content-type"].iter().any(|k| h.eq_ignore_ascii_case(k)));
        spec.query.retain(|(n, _)| !path_vars.contains(n));
        let method = route.endpoint.method.as_str();
        if matches!(method, "GET" | "HEAD") {
            // No body for GET: whatever was read from the request becomes query parameters.
            for (n, t) in body.drain(..) {
                add(&mut spec.query, &n, t);
            }
        } else if !body.is_empty() || matches!(method, "POST" | "PUT" | "PATCH") {
            spec.body = Some(body);
        }
        spec
    }

    /// Fields named by one query/body hit: `{keys}`, `{var}` (via its type), `{name}` or `{type}`.
    fn fields_from(&self, lang: &Language, text: &str, window: &str, hit: &Hit) -> Vec<(String, Ty)> {
        if let Some(k) = hit.get("keys") {
            return keys_of(k).into_iter().map(|k| (k, Ty::Str)).collect();
        }
        if let Some(var) = hit.get("var") {
            return match lang.var_type(window, var).or_else(|| lang.var_type(text, var)) {
                Some(t) => self.model_fields(lang, text, &t, 0),
                None => vec![],
            };
        }
        if let Some(name) = hit.get("name") {
            let ty = hit.get("type").map_or(Ty::Str, |t| self.ty_of(lang, text, t, 1));
            return vec![(name.to_string(), ty)];
        }
        if let Some(t) = hit.get("type") {
            return self.model_fields(lang, text, t, 0);
        }
        vec![]
    }

    /// Strips wrapper types (`Optional<T>`, `Json<T>`…) down to the type they carry.
    pub fn unwrap_type<'a>(&self, t: &'a str) -> &'a str {
        let mut t = t.trim().trim_start_matches(['&', '*']).trim_end_matches('?');
        t = t.strip_suffix(" | None").unwrap_or(t).trim();
        loop {
            let base = t.split(['<', '[']).next().unwrap_or(t);
            let is_wrapper = self.types.wrappers.iter().any(|w| w.eq_ignore_ascii_case(base));
            match (is_wrapper, t.find(['<', '[']), t.rfind(['>', ']'])) {
                (true, Some(a), Some(b)) if a < b => {
                    t = t[a + 1..b].split(',').next().unwrap_or("").trim();
                }
                _ => return t,
            }
        }
    }

    /// Maps a source type to the JSON shape used for sample values.
    pub fn ty_of(&self, lang: &Language, text: &str, t: &str, depth: usize) -> Ty {
        let t = self.unwrap_type(t);
        if t.starts_with("[]") || t.ends_with("[]") {
            return Ty::List;
        }
        let base = t.split(['<', '[']).next().unwrap_or(t);
        let short = base.rsplit(['.', ':']).next().unwrap_or(base).to_lowercase();
        let is = |list: &[String]| list.iter().any(|x| x.eq_ignore_ascii_case(&short));
        if is(&self.types.list) {
            Ty::List
        } else if is(&self.types.int) {
            Ty::Int
        } else if is(&self.types.float) {
            Ty::Float
        } else if is(&self.types.bool) {
            Ty::Bool
        } else if is(&self.types.object) {
            Ty::Obj(vec![])
        } else if is(&self.types.string) {
            Ty::Str
        } else if depth < 3 && base.chars().next().is_some_and(char::is_uppercase) {
            let fields = self.model_fields(lang, text, base, depth);
            if fields.is_empty() {
                Ty::Str
            } else {
                Ty::Obj(fields)
            }
        } else {
            Ty::Str
        }
    }

    /// Fields of the model (class / struct / record / interface…) `name` declared in `text`.
    fn model_fields(&self, lang: &Language, text: &str, name: &str, depth: usize) -> Vec<(String, Ty)> {
        let name = self.unwrap_type(name);
        let name = name.split(['<', '[']).next().unwrap_or(name);
        let name = name.rsplit(['.', ':']).next().unwrap_or(name);
        let Some(models) = &lang.models else { return vec![] };
        if name.is_empty() || !name.chars().next().is_some_and(char::is_uppercase) {
            return vec![];
        }
        let Ok(Some(decl)) = Matcher::new(&models.declarations, &[("type", name)]) else { return vec![] };
        let Some(hit) =
            decl.hits(text).find(|h| text[h.end..].chars().next().is_none_or(|c| !c.is_alphanumeric() && c != '_'))
        else {
            return vec![];
        };
        let rest = &text[hit.end..];
        let parens = models.block == Block::Braces && rest.trim_start_matches([' ', '\t']).starts_with(['(', '<']) && {
            let after = rest.trim_start().trim_start_matches(|c| c != '(' && c != '{');
            after.starts_with('(')
        };
        let block = if parens {
            enclosed(rest, '(', ')').unwrap_or_default().replace(',', "\n")
        } else if models.block == Block::Indent {
            rest.lines()
                .skip(1)
                .take_while(|l| l.trim().is_empty() || l.starts_with([' ', '\t']))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            let Some(open) = rest.find('{') else { return vec![] };
            if rest[..open].contains(';') {
                return vec![];
            }
            top_level(&enclosed(&rest[open..], '{', '}').unwrap_or_default())
        };

        let annotation = Regex::new(r"^(?:@[\w.]+(?:\([^)]*\))?|\[[^\]]*\])\s*").unwrap();
        let mut fields = Vec::new();
        for raw in block.lines() {
            let mut line = raw.trim();
            while let Some(m) = annotation.find(line) {
                line = line[m.end()..].trim_start();
            }
            'strip: loop {
                for m in &models.modifiers {
                    if let Some(rest) = line.strip_prefix(m.as_str()).filter(|r| r.starts_with(char::is_whitespace)) {
                        line = rest.trim_start();
                        continue 'strip;
                    }
                }
                break;
            }
            if line.is_empty()
                || line.starts_with(['#', '/', '*', '@'])
                || ["def ", "fn ", "func ", "function ", "return", "class ", "static "]
                    .iter()
                    .any(|k| line.starts_with(k))
                || (!parens && line.contains('('))
            {
                continue;
            }
            let Some(h) = models.fields.first(line).filter(|h| h.start == 0) else { continue };
            let (Some(n), Some(t)) = (h.get("name"), h.get("type")) else { continue };
            if !fields.iter().any(|(f, _): &(String, Ty)| f == n) {
                fields.push((n.to_string(), self.ty_of(lang, text, t, depth + 1)));
            }
        }
        fields
    }

    /// Human-readable tree of the catalog.
    pub fn tree(&self) -> String {
        let mut out = String::new();
        for l in &self.languages {
            let exts = l.extensions.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(" ");
            out.push_str(&format!("{} ({exts})\n", l.name));
            for (i, f) in l.frameworks.iter().enumerate() {
                let branch = if i + 1 == l.frameworks.len() { "└──" } else { "├──" };
                let when = if f.imports.is_empty() {
                    "always".to_string()
                } else {
                    format!(
                        "when the file contains {}",
                        f.imports.iter().map(|i| format!("`{i}`")).collect::<Vec<_>>().join(" or ")
                    )
                };
                let n = f.route_defs.len();
                let plural = if n == 1 { "" } else { "s" };
                out.push_str(&format!("  {branch} {} [{}] — {n} route pattern{plural}, {when}\n", f.name, f.key));
                if let Some(d) = &f.docs {
                    let pad = if i + 1 == l.frameworks.len() { " " } else { "│" };
                    out.push_str(&format!("  {pad}     {d}\n"));
                }
            }
        }
        out
    }
}

impl Language {
    fn compile(key: &str, l: LanguageDef) -> Result<Language, String> {
        let ctx = |what: &str| format!("languages.{key}.{what}");
        let mut frameworks = Vec::new();
        for (fkey, v) in l.frameworks {
            let f: FrameworkDef =
                serde_json::from_value(v).map_err(|e| format!("{}: {e}", ctx(&format!("frameworks.{fkey}"))))?;
            frameworks.push(Framework::compile(&ctx(&format!("frameworks.{fkey}")), &fkey, f)?);
        }
        let models = match l.models {
            Some(m) => Some(Models {
                fields: compile(&m.fields, &ctx("models.fields"))?
                    .ok_or_else(|| format!("{}: empty", ctx("models.fields")))?,
                declarations: m.declarations,
                block: m.block,
                modifiers: m.modifiers,
            }),
            None => None,
        };
        let class_keywords = if l.class_keywords.is_empty() {
            None
        } else {
            let alts = l.class_keywords.iter().map(|k| regex::escape(k)).collect::<Vec<_>>().join("|");
            // The keywords are escaped, so the pattern is always a valid regex.
            Some(Regex::new(&format!(r"\b(?:{alts})\s+\w")).expect("escaped keywords"))
        };
        Ok(Language {
            key: key.to_string(),
            name: l.name,
            zed: l.zed,
            extensions: l.extensions.iter().map(|e| e.to_lowercase()).collect(),
            decorators: l.decorators,
            class_keywords,
            definitions: compile(&l.definitions, &ctx("definitions"))?,
            definition_patterns: l.definitions,
            handler_refs: compile(&l.handler_refs, &ctx("handler_refs"))?,
            variables: l.variables,
            models,
            frameworks,
        })
    }

    /// An annotation on `line` belongs to a class if the first line after the annotation block
    /// (other decorators, blank lines) declares a class.
    fn is_class_level(&self, text: &str, starts: &[usize], line: usize) -> bool {
        let Some(re) = &self.class_keywords else { return false };
        let from = starts.get(line).copied().unwrap_or(text.len());
        text[from..]
            .lines()
            .skip(1)
            .find(|l| !l.trim().is_empty() && !self.starts_decorated(l))
            .is_some_and(|l| re.is_match(l))
    }

    fn starts_decorated(&self, line: &str) -> bool {
        let t = line.trim_start();
        self.decorators.iter().any(|d| t.starts_with(d.as_str()))
    }

    fn is_definition(&self, line: &str) -> bool {
        self.definitions.as_ref().is_some_and(|m| m.is_match(line))
    }

    fn find_definition(&self, lines: &[&str], handler: &str) -> Option<usize> {
        let with_handler: Vec<String> =
            self.definition_patterns.iter().filter(|p| p.contains("{handler}")).cloned().collect();
        let m = Matcher::new(&with_handler, &[("handler", handler)]).ok()??;
        lines.iter().position(|l| m.is_match(l))
    }

    /// Lines belonging to the route / handler at `start`: up to 3 lines before it (a signature
    /// above a client call, stacked decorators), then forward until the next route or definition.
    fn window(&self, lines: &[&str], start: usize, route_lines: &BTreeSet<usize>) -> String {
        let start = start.min(lines.len());
        let mut from = start;
        while from > 0 && start - from < 3 {
            let prev = lines[from - 1].trim_end();
            let decorated = self.starts_decorated(prev);
            if prev.trim().is_empty()
                || route_lines.contains(&(from - 1))
                || (!decorated && prev.ends_with(['}', ';', ')']))
            {
                break;
            }
            from -= 1;
        }
        // After a decorator (`@app.post`, `#[get]`) the first definition is the handler itself.
        let mut seen_def = !lines.get(start).is_some_and(|l| self.starts_decorated(l));
        let end = (start + 1..lines.len().min(start + WINDOW))
            .find(|&i| {
                if route_lines.contains(&i) {
                    return true;
                }
                let def = self.is_definition(lines[i]);
                let stop = def && seen_def;
                seen_def |= def;
                stop
            })
            .unwrap_or_else(|| lines.len().min(start + WINDOW));
        lines[from..end].join("\n")
    }

    /// Declared type of variable `var` in `text`.
    fn var_type(&self, text: &str, var: &str) -> Option<String> {
        let m = Matcher::new(&self.variables, &[("var", var)]).ok()??;
        let found = m.hits(text).find_map(|h| h.get("type").map(str::to_string));
        found
    }
}

impl Framework {
    fn compile(ctx: &str, key: &str, f: FrameworkDef) -> Result<Framework, String> {
        let route_defs: Vec<RouteDef> = f
            .routes
            .iter()
            .map(|r| match r {
                RouteSpec::Pattern(p) => RouteDef { pattern: p.clone(), ..Default::default() },
                RouteSpec::Full(d) => d.clone(),
            })
            .collect();
        let patterns: Vec<String> = route_defs.iter().map(|r| r.pattern.clone()).collect();
        let c = |p: &[String], what: &str| compile(p, &format!("{ctx}.{what}"));
        let path_params = if f.path_params.is_empty() {
            vec![Style::Dollar, Style::Braces, Style::Angle, Style::Colon]
        } else {
            f.path_params
        };
        Ok(Framework {
            key: key.to_string(),
            name: f.name,
            docs: f.docs,
            imports: f.imports,
            prefix: c(&f.prefix, "prefix")?,
            prefix_scope: f.prefix_scope,
            controller_token: f.controller_token,
            routes: c(&patterns, "routes")?.ok_or_else(|| format!("{ctx}.routes: empty"))?,
            route_defs,
            path_params,
            query: c(&f.query, "query")?,
            headers: c(&f.headers, "headers")?,
            header_case: f.header_case,
            body: c(&f.body, "body")?,
            params: c(&f.params, "params")?,
            ignore: c(&f.ignore, "ignore")?,
            auth: c(&f.auth, "auth")?,
            examples: f.examples,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

pub fn project_file(root: &Path) -> PathBuf {
    root.join(crate::vars::ZED_HTTP_DIR).join(PROJECT_FILE)
}

/// The catalog for a source file: built-in plus the nearest `.zed/http/frameworks.json` above it.
pub fn for_source(source: &Path) -> Result<Catalog, String> {
    let mut dir = source.parent();
    while let Some(d) = dir {
        if project_file(d).exists() {
            return Catalog::for_root(d);
        }
        dir = d.parent();
    }
    Catalog::from_sources(&[BUILTIN])
}

fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect()
}

fn line_of(starts: &[usize], offset: usize) -> usize {
    starts.partition_point(|&s| s <= offset).saturating_sub(1)
}

/// `https://{host}{path}` with captures filled in.
fn fill(template: &str, hit: &Hit) -> String {
    let mut out = template.to_string();
    for (k, v) in &hit.caps {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// Is this string literal a URL or a path, rather than any other text a variable may hold?
/// Interpolation counts: `$baseUrl/items`, `{base}/users` and `${API}/users` are all URLs.
fn looks_like_url(s: &str) -> bool {
    s.contains("://") || s.starts_with('/') || s.starts_with('$') || s.starts_with('{')
}

fn join(prefix: &str, path: &str) -> String {
    let (a, b) = (prefix.trim_matches('/'), path.trim_start_matches('/'));
    match (a.is_empty(), b.is_empty()) {
        (true, true) => "/".into(),
        (true, false) => format!("/{b}"),
        (false, true) => format!("/{a}"),
        (false, false) => format!("/{a}/{b}"),
    }
}

/// Converts framework path parameters to `{{name}}`, in one pass: a `{{name}}` already in the
/// path (a URL read out of a variable) is left as it is instead of being converted again.
pub fn convert_params(path: &str, styles: &[Style]) -> String {
    let mut alts = vec![r"(?P<keep>\{\{\s*\w+\s*\}\})"];
    for style in [Style::Dollar, Style::Braces, Style::Angle, Style::Colon] {
        if styles.contains(&style) {
            alts.push(match style {
                Style::Dollar => r"\$\{\s*(?:[\w.]+\.)?(?P<d1>\w+)\s*\}|\$(?P<d2>[A-Za-z_]\w*)",
                Style::Braces => r"\{(?P<b>\w+)(?::[^}]*)?\??\}",
                Style::Angle => r"<(?:\w+:)?(?P<a>\w+)>",
                Style::Colon => r"(?P<lead>^|/):(?P<c>\w+)",
            });
        }
    }
    // `path_params` always holds at least one style (see `Framework::compile`).
    let re = Regex::new(&alts.join("|")).unwrap();
    re.replace_all(path, |c: &regex::Captures| {
        if let Some(kept) = c.name("keep") {
            return kept.as_str().to_string();
        }
        let lead = c.name("lead").map_or("", |m| m.as_str());
        let name = ["d1", "d2", "b", "a", "c"].iter().find_map(|n| c.name(n)).map_or("", |m| m.as_str());
        format!("{lead}{{{{{name}}}}}")
    })
    .into_owned()
}

fn kebab_header(param: &str) -> String {
    param
        .split('_')
        .map(|p| {
            let mut cs = p.chars();
            cs.next().map_or(String::new(), |c| c.to_uppercase().chain(cs).collect())
        })
        .collect::<Vec<_>>()
        .join("-")
}

fn enclosed(s: &str, open: char, close: char) -> Option<String> {
    let start = s.find(open)?;
    let mut depth = 0;
    for (i, c) in s[start..].char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(s[start + 1..start + i].to_string());
            }
        }
    }
    None
}

/// Top-level statements of a block body (nested bodies removed), one per line.
fn top_level(body: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for c in body.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
        if c == '\n' && depth > 0 {
            out.push('\n');
        }
    }
    out.replace(';', ";\n")
}
