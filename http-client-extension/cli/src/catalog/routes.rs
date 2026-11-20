//! Finding routes and HTTP calls in a source file, and the endpoint each one stands for:
//! method, prefix, base URL and path parameters.

use super::compile::{Framework, Language, ALL_STYLES};
use super::model::{Scope, Style};
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
            let context = RouteContext { urls: &self.urls, language, framework, text, lines: &lines, source: path };
            for hit in framework.routes.hits(text) {
                let line = lines.line_of(hit.start);
                if context.is_class_prefix(&hit, line) {
                    continue;
                }
                let Some(endpoint) = context.endpoint(&hit, line) else { continue };
                let replaces = by_line.get(&line).is_none_or(|found| found.endpoint.weak && !endpoint.weak);
                if replaces {
                    let route = Route {
                        line,
                        end_line: lines.line_of(hit.end.saturating_sub(1)),
                        endpoint,
                        framework: format!("{}/{}", language.key, framework.key),
                        language_index: candidate.language,
                        framework_index: candidate.framework,
                        hit,
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
            .or_else(|| definition.method.clone())
            .unwrap_or_else(|| "GET".into());
        let mut path = match (&definition.url, hit.get("ref")) {
            (Some(template), _) => fill(template, hit),
            (None, Some(reference)) => self.urls.held_by(self.text, reference)?,
            (None, None) => hit.get("path").or(hit.get("route")).unwrap_or_default().to_string(),
        };
        let absolute = path.starts_with("http://") || path.starts_with("https://");
        if !absolute && definition.prefixed.unwrap_or(true) {
            if let Some(prefix) = self.prefix(line) {
                path = join(&prefix, &path);
            }
        }
        let mut path = self.framework.path_params.convert(&path);
        if !absolute {
            if let Some(base) = self.base_url(line) {
                path = format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'));
                path = path.trim_end_matches('/').to_string();
            }
        }
        Some(Endpoint { method, url: endpoint::to_http_url(&path), weak: definition.weak })
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

    /// The base URL of a client class (`@FeignClient(url = ...)`) above `line`: an absolute URL
    /// once the project's configuration resolves its placeholders, else `{{variable}}` for what is
    /// missing (`${catalog.url}` → `{{catalogUrl}}`).
    fn base_url(&self, line: usize) -> Option<String> {
        let matcher = self.framework.base.as_ref()?;
        let owner = self.language.enclosing_class(self.text, line)?;
        let hit = matcher.hits(self.text).filter(|hit| self.annotates(hit, line) == Some(owner)).last()?;
        let raw = match (hit.get("path"), hit.get("name")) {
            (Some(url), _) if !url.trim().is_empty() => url.trim().to_string(),
            (_, Some(name)) => format!("${{spring.cloud.openfeign.client.config.{name}.url}}"),
            _ => return None,
        };
        if !raw.contains("${") {
            return Some(raw);
        }
        let properties = server::project_properties(self.source).unwrap_or_default();
        let resolution = server::resolve(&raw, &properties);
        let mut resolved = resolution.text;
        for key in resolution.missing {
            resolved = resolved.replace(&format!("${{{key}}}"), &format!("{{{{{}}}}}", server::variable_for(&key)));
        }
        if !resolved.contains("://") && !resolved.starts_with("{{") {
            resolved = format!("http://{resolved}");
        }
        Some(resolved)
    }
}

/// `https://{host}{path}` with captures filled in.
fn fill(template: &str, hit: &Hit) -> String {
    let mut out = template.to_string();
    for (name, value) in &hit.caps {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// A prefix and a path joined by exactly one `/`, always starting with one.
fn join(prefix: &str, path: &str) -> String {
    let (prefix, path) = (prefix.trim_matches('/'), path.trim_start_matches('/'));
    match (prefix.is_empty(), path.is_empty()) {
        (true, true) => "/".into(),
        (true, false) => format!("/{path}"),
        (false, true) => format!("/{prefix}"),
        (false, false) => format!("/{prefix}/{path}"),
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
