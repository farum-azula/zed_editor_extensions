//! Finding routes and HTTP calls in a source file, and the endpoint each one stands for:
//! method, prefix, base URL and path parameters.

use super::compile::{Framework, Language, ALL_STYLES};
use super::model::{Scope, Style};
use super::registrations::RegistrationLookup;
use super::source::LineIndex;
use super::urls::Urls;
use super::Catalog;
use crate::endpoint::{self, Endpoint};
use crate::pattern::Hit;
use crate::server;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;

/// The class a controller-relative prefix (`api/[controller]`) takes its name from.
static CONTROLLER_CLASS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"class\s+(\w+?)(?:Controller)?\b").expect("the controller class pattern is a valid regex")
});

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
    pub(super) language_index: usize,
    pub(super) framework_index: usize,
    pub(super) hit: Hit,
    /// The headers its class's annotation gives every route of the class (`Name`, value), the
    /// project's placeholders resolved.
    pub(super) class_headers: Vec<(String, String)>,
}

/// A framework that may appear in a file, by its position in the catalog.
struct Candidate {
    language: usize,
    framework: usize,
}

impl Catalog {
    /// Routes and HTTP calls in `text`, one per line, sorted by line.
    pub fn routes(&self, text: &str, path: &Path) -> Vec<Route> {
        let lines = LineIndex::new(text);
        let mut by_line: BTreeMap<usize, Route> = BTreeMap::new();
        for candidate in self.candidates(text, path) {
            let language = &self.languages[candidate.language];
            let framework = &language.frameworks[candidate.framework];
            let registrations = framework
                .registrations
                .as_ref()
                .map(|registrations| RegistrationLookup::new(registrations, text, path, &language.extensions));
            let context = RouteContext {
                urls: &self.urls,
                language,
                framework,
                text,
                lines: &lines,
                source: path,
                registrations,
            };
            for hit in framework.routes.hits(text) {
                let line = lines.line_of(hit.start);
                if context.is_class_prefix(&hit, line) {
                    continue;
                }
                let Some(endpoint) = context.endpoint(&hit, line) else { continue };
                let replaces = by_line.get(&line).is_none_or(|found| found.endpoint.weak && !endpoint.weak);
                if replaces {
                    let class_headers = context.class_defaults(line).headers;
                    let route = Route {
                        line,
                        end_line: lines.line_of(hit.end.saturating_sub(1)),
                        endpoint,
                        framework: format!("{}/{}", language.key, framework.key),
                        language_index: candidate.language,
                        framework_index: candidate.framework,
                        hit,
                        class_headers,
                    };
                    by_line.insert(line, route);
                }
            }
        }
        // A URL literal inside a recognized multi-line call is the same request, not another one.
        let calls: Vec<(usize, usize)> =
            by_line.values().filter(|route| !route.endpoint.weak).map(|route| (route.line, route.end_line)).collect();
        by_line.retain(|line, route| {
            !route.endpoint.weak || !calls.iter().any(|(start, end)| line >= start && line <= end)
        });
        by_line.into_values().collect()
    }

    /// The frameworks of the file's language and of every language (`*`) whose imports, if they
    /// have any, the file mentions.
    fn candidates(&self, text: &str, path: &Path) -> Vec<Candidate> {
        let mut candidates = Vec::new();
        for language in [self.language_for(path), self.any_language()].into_iter().flatten() {
            for (framework, definition) in self.languages[language].frameworks.iter().enumerate() {
                let imported = definition.imports.is_empty()
                    || definition.imports.iter().any(|import| text.contains(import.as_str()));
                if imported {
                    candidates.push(Candidate { language, framework });
                }
            }
        }
        candidates
    }
}

/// What finding the endpoint of a route needs to know about the file it is in.
struct RouteContext<'a> {
    urls: &'a Urls,
    language: &'a Language,
    framework: &'a Framework,
    text: &'a str,
    lines: &'a LineIndex,
    source: &'a Path,
    /// Where the file's client classes are registered, if the framework registers them.
    registrations: Option<RegistrationLookup<'a>>,
}

impl RouteContext<'_> {
    /// A class-level prefix (`@RequestMapping("/api")` above a class) is not a route.
    fn is_class_prefix(&self, hit: &Hit, line: usize) -> bool {
        let starts_a_prefix = |prefix: &crate::pattern::Matcher| {
            prefix.first(&self.text[hit.start..]).is_some_and(|found| found.start == 0)
        };
        self.framework.prefix.as_ref().is_some_and(starts_a_prefix)
            && self.framework.prefix_scope == Scope::Class
            && self.language.is_class_level(self.text, self.lines, line)
    }

    /// The endpoint a route matched, or `None` when the match turns out not to be a request:
    /// a `{ref}` pattern whose variable holds no URL (`map.get(key)`).
    fn endpoint(&self, hit: &Hit, line: usize) -> Option<Endpoint> {
        let definition = &self.framework.route_definitions[hit.alt];
        let method = hit
            .get("method")
            .map(str::to_uppercase)
            .or_else(|| self.class_defaults(line).method)
            .or_else(|| definition.method.clone())
            .unwrap_or_else(|| "GET".into());
        let mut path = match (&definition.url, hit.get("ref")) {
            (Some(template), _) => fill(template, hit),
            (None, Some(reference)) => self.urls.held_by(self.text, reference)?,
            (None, None) => hit.get("path").or(hit.get("route")).unwrap_or_default().to_string(),
        };
        if !is_absolute(&path) && definition.prefixed.unwrap_or(true) {
            if let Some(prefix) = self.prefix(line) {
                path = join(&prefix, &path);
            }
        }
        let mut path = self.framework.path_params.convert(&path);
        // A prefix may be a whole URL too (`@HttpExchange("https://api.github.com")`).
        if !is_absolute(&path) {
            if let Some(base) = self.base_url(line) {
                path = format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'));
                path = path.trim_end_matches('/').to_string();
            }
        }
        Some(Endpoint { method, url: endpoint::to_http_url(&path), weak: definition.weak })
    }

    /// What the class of the route on `line` sets for its routes (`@HttpExchange.of(method:
    /// HttpMethod.post, headers: ['X-Api=1'])` on the interface): a method for those that name
    /// none, and headers for all of them.
    fn class_defaults(&self, line: usize) -> ClassDefaults {
        let mut defaults = ClassDefaults::default();
        let Some(owner) =
            self.language.enclosing_class(self.text, line).filter(|_| !self.framework.defaults.is_empty())
        else {
            return defaults;
        };
        let hits = self.framework.defaults.iter().flat_map(|matcher| matcher.hits(self.text));
        for hit in hits.filter(|hit| self.annotates(hit, line) == Some(owner)) {
            if let Some(method) = hit.get("method") {
                defaults.method = Some(method.to_uppercase());
            }
            if let Some(list) = hit.get("keys").or(hit.get("path")) {
                for (name, value) in header_values(list) {
                    defaults.headers.push((name, self.resolved(&value)));
                }
            }
        }
        defaults
    }

    /// `text` with the project's `${...}` placeholders filled in, `{{variable}}` for those it
    /// does not set.
    fn resolved(&self, text: &str) -> String {
        resolve_placeholders(text, self.source)
    }

    /// The prefix that applies to the route on `line`: the file's, or that of its class.
    fn prefix(&self, line: usize) -> Option<String> {
        let matcher = self.framework.prefix.as_ref()?;
        let found = match self.framework.prefix_scope {
            Scope::File => matcher.first(self.text),
            Scope::Class => {
                let owner = self.language.enclosing_class(self.text, line);
                matcher.hits(self.text).filter(|hit| owner.is_some() && self.annotates(hit, line) == owner).last()
            }
        }?;
        let mut prefix = found.get("path").unwrap_or_default().to_string();
        if let Some(token) = self.framework.controller_token.as_deref().filter(|token| prefix.contains(*token)) {
            if let Some(class) = CONTROLLER_CLASS.captures(&self.text[found.end..]) {
                prefix = prefix.replace(token, &class[1]);
            }
        }
        Some(prefix)
    }

    /// The class declaration that `hit`, above `line`, annotates.
    fn annotates(&self, hit: &Hit, line: usize) -> Option<usize> {
        let hit_line = self.lines.line_of(hit.start);
        if hit_line >= line {
            return None;
        }
        self.language.class_after(self.text, self.lines, hit_line)
    }

    /// The base URL of the client class the route on `line` belongs to: the one its annotation
    /// gives (`@FeignClient(url = ...)`), else that of the name it is registered under
    /// (`@ImportHttpServices(group: 'catalog', ...)` → `spring.http.serviceclient.catalog.base-url`).
    /// An absolute URL once the project's configuration resolves its placeholders, else
    /// `{{variable}}` for what is missing (`${catalog.url}` → `{{catalogUrl}}`).
    fn base_url(&self, line: usize) -> Option<String> {
        let raw = self.declared_base(line).or_else(|| self.registered_base(line))?;
        if !raw.contains("${") {
            return Some(raw);
        }
        let mut resolved = self.resolved(&raw);
        if !resolved.contains("://") && !resolved.starts_with("{{") {
            resolved = format!("http://{resolved}");
        }
        Some(resolved)
    }

    /// The base a `base` pattern on the class of the route on `line` gives: its URL, or the
    /// property of its name. `None` for an empty URL.
    fn declared_base(&self, line: usize) -> Option<String> {
        let matcher = self.framework.base.as_ref()?;
        let owner = self.language.enclosing_class(self.text, line)?;
        let hit = matcher.hits(self.text).filter(|hit| self.annotates(hit, line) == Some(owner)).last()?;
        match (hit.get("path"), hit.get("name")) {
            (Some(url), _) if !url.trim().is_empty() => Some(url.trim().to_string()),
            (_, Some(name)) => self.named_base(name),
            _ => None,
        }
    }

    /// The base of the name the class of the route on `line` is registered under.
    fn registered_base(&self, line: usize) -> Option<String> {
        let registrations = self.registrations.as_ref()?;
        let owner = self.language.enclosing_class(self.text, line)?;
        let name = registrations.name_of(&self.language.class_name(self.text, owner)?)?;
        self.named_base(&name)
    }

    /// `${<base property>}` of the client named `name`.
    fn named_base(&self, name: &str) -> Option<String> {
        let property = self.framework.base_property.as_deref()?;
        Some(format!("${{{}}}", property.replace("{name}", name)))
    }
}

/// What a client class's annotation sets for the routes of the class.
#[derive(Default)]
struct ClassDefaults {
    method: Option<String>,
    headers: Vec<(String, String)>,
}

/// `text` with the `${...}` placeholders the configuration of the project of `source` sets filled
/// in, and `{{variable}}` for the others (`${catalog.url}` → `{{catalogUrl}}`).
pub(super) fn resolve_placeholders(text: &str, source: &Path) -> String {
    if !text.contains("${") {
        return text.to_string();
    }
    let properties = server::project_properties(source).unwrap_or_default();
    let resolution = server::resolve(text, &properties);
    let mut resolved = resolution.text;
    for key in resolution.missing {
        resolved = resolved.replace(&format!("${{{key}}}"), &format!("{{{{{}}}}}", server::variable_for(&key)));
    }
    resolved
}

/// The `Name=value` headers of a list literal (`['X-Api=1', r'X-Shop=${shop.name}']`,
/// `{"X-Api=1"}`) or of a single string's contents: each name with its value, in order. An item
/// without `=` names no value and is left out.
pub(super) fn header_values(list: &str) -> Vec<(String, String)> {
    let items: Vec<String> = match list.trim().strip_prefix(['[', '{']) {
        Some(inner) => {
            let inner = inner.trim_end().trim_end_matches([']', '}']);
            string_contents(inner)
        }
        None => vec![list.to_string()],
    };
    items
        .iter()
        .filter_map(|item| item.split_once('='))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .filter(|(name, _)| !name.is_empty())
        .collect()
}

/// The contents of the string literals in `text`, in order (`'a', r"b"` → `a`, `b`).
fn string_contents(text: &str) -> Vec<String> {
    static STRING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#""([^"]*)"|'([^']*)'"#).expect("the string literal pattern is a valid regex"));
    STRING
        .captures_iter(text)
        .filter_map(|found| found.get(1).or(found.get(2)).map(|content| content.as_str().to_string()))
        .collect()
}

/// Is `path` a whole URL, not a path?
fn is_absolute(path: &str) -> bool {
    path.starts_with("http://") || path.starts_with("https://")
}

/// `https://{host}{path}` with captures filled in.
fn fill(template: &str, hit: &Hit) -> String {
    let mut out = template.to_string();
    for (name, value) in &hit.caps {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// A prefix and a path joined by exactly one `/`, starting with one unless the prefix is a
/// whole URL.
fn join(prefix: &str, path: &str) -> String {
    let (prefix, path) = (prefix.trim_matches('/'), path.trim_start_matches('/'));
    let root = if is_absolute(prefix) { "" } else { "/" };
    match (prefix.is_empty(), path.is_empty()) {
        (true, true) => "/".into(),
        (true, false) => format!("/{path}"),
        (false, true) => format!("{root}{prefix}"),
        (false, false) => format!("{root}{prefix}/{path}"),
    }
}

/// Converts a framework's path parameters (`{id}`, `:id`, `<id>`, `$id`) to `{{id}}`.
pub(super) struct PathParams {
    regex: Regex,
}

/// Regex groups holding the name of a parameter, over every style.
const NAME_GROUPS: [&str; 5] = ["dollar_braced", "dollar", "braces", "angle", "colon"];

impl Style {
    /// The regex alternative matching a parameter written in this style.
    fn alternative(self) -> &'static str {
        match self {
            Style::Dollar => r"\$\{\s*(?:[\w.]+\.)?(?P<dollar_braced>\w+)\s*\}|\$(?P<dollar>[A-Za-z_]\w*)",
            Style::Braces => r"\{(?P<braces>\w+)(?::[^}]*)?\??\}",
            Style::Angle => r"<(?:\w+:)?(?P<angle>\w+)>",
            Style::Colon => r"(?P<lead>^|/):(?P<colon>\w+)",
        }
    }
}

impl PathParams {
    pub fn new(styles: &[Style]) -> PathParams {
        // A `{{name}}` already in the path (a URL read out of a variable) is kept as it is.
        let mut alternatives = vec![r"(?P<keep>\{\{\s*\w+\s*\}\})"];
        alternatives.extend(ALL_STYLES.iter().filter(|style| styles.contains(style)).map(|style| style.alternative()));
        let regex = Regex::new(&alternatives.join("|")).expect("the path parameter alternatives form a valid regex");
        PathParams { regex }
    }

    /// `path` with every parameter as `{{name}}`, in one pass, so nothing is converted twice.
    pub fn convert(&self, path: &str) -> String {
        self.regex
            .replace_all(path, |captures: &regex::Captures| {
                if let Some(kept) = captures.name("keep") {
                    return kept.as_str().to_string();
                }
                let lead = captures.name("lead").map_or("", |lead| lead.as_str());
                let name = NAME_GROUPS.iter().find_map(|group| captures.name(group)).map_or("", |name| name.as_str());
                format!("{lead}{{{{{name}}}}}")
            })
            .into_owned()
    }
}

/// Converts the path parameters of `path`, written in any of `styles`, to `{{name}}`.
pub fn convert_params(path: &str, styles: &[Style]) -> String {
    PathParams::new(styles).convert(path)
}
