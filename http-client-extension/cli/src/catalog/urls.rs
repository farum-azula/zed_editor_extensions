//! The URL a variable holds, for calls that pass one instead of a literal (`http.post(_endpoint, …)`).

use super::compile::compile_patterns;
use super::model::UrlsDefinition;
use crate::endpoint;
use crate::pattern::Matcher;

/// Compiled [`UrlsDefinition`]. Assignment patterns are compiled per lookup, with `{var}` bound.
pub(super) struct Urls {
    assignments: Vec<String>,
    literals: Option<Matcher>,
    env: Option<Matcher>,
}

impl Urls {
    pub fn compile(definition: UrlsDefinition) -> Result<Urls, String> {
        Ok(Urls {
            assignments: definition.assignments,
            literals: compile_patterns(&definition.literals, "urls.literals")?,
            env: compile_patterns(&definition.env, "urls.env")?,
        })
    }

    /// The URL the variable `reference` holds: a literal or environment variable assigned to it
    /// anywhere in the file, or the variable itself when its name names a URL (`_endpoint` →
    /// `{{endpoint}}`, filled in from the environment file). `None` when neither holds, which is
    /// what tells a `{ref}` match apart from an unrelated call.
    pub fn held_by(&self, text: &str, reference: &str) -> Option<String> {
        let variable = reference.rsplit('.').next().unwrap_or(reference).trim_start_matches(['$', '@']);
        if let Some(url) = self.assigned_expression(text, variable).and_then(|expression| self.url_in(&expression)) {
            return Some(url);
        }
        let name = variable.trim_start_matches('_');
        endpoint::names_a_url(name).then(|| format!("{{{{{name}}}}}"))
    }

    /// A URL literal in `expression`, else the environment variable it reads as `{{NAME}}`.
    fn url_in(&self, expression: &str) -> Option<String> {
        let literal = self
            .literals
            .iter()
            .flat_map(|literals| literals.hits(expression))
            .filter_map(|hit| hit.get("path").map(str::to_string))
            .find(|path| looks_like_url(path));
        literal.or_else(|| {
            let key = self
                .env
                .iter()
                .filter_map(|env| env.first(expression))
                .find_map(|hit| hit.get("name").map(str::to_string))?;
            Some(format!("{{{{{key}}}}}"))
        })
    }

    /// The expression assigned to `variable` anywhere in `text`.
    fn assigned_expression(&self, text: &str, variable: &str) -> Option<String> {
        let matcher = Matcher::new(&self.assignments, &[("var", variable)]).ok()??;
        let found = matcher.hits(text).find_map(|hit| hit.get("expr").map(str::to_string));
        found
    }
}

/// Is this string literal a URL or a path, rather than any other text a variable may hold?
/// Interpolation counts: `$baseUrl/items`, `{base}/users` and `${API}/users` are all URLs.
fn looks_like_url(text: &str) -> bool {
    text.contains("://") || text.starts_with('/') || text.starts_with('$') || text.starts_with('{')
}
