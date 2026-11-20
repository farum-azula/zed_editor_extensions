//! The shape of `frameworks.json`, as read from disk. [`super::compile`] turns it into the
//! compiled catalog the engine runs on.

use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CatalogFile {
    #[serde(rename = "$schema", default)]
    _schema: Option<String>,
    #[serde(default)]
    pub types: TypesDefinition,
    #[serde(default)]
    pub auth: Vec<String>,
    #[serde(default)]
    pub urls: UrlsDefinition,
    /// Kept as JSON until each language is read, so that an error names the language it is in.
    pub languages: Map<String, Value>,
}

/// How to read the URL out of a variable, for calls that pass one (`http.post(_endpoint, …)`).
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct UrlsDefinition {
    /// Patterns binding `{var}` and capturing the `{expr}` assigned to it.
    #[serde(default)]
    pub assignments: Vec<String>,
    /// Patterns capturing a URL `{path}` in such an expression.
    #[serde(default)]
    pub literals: Vec<String>,
    /// Patterns capturing the `{name}` of an environment variable read in such an expression.
    #[serde(default)]
    pub env: Vec<String>,
}

/// Type names per JSON shape, and the wrapper types that only carry another type.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct TypesDefinition {
    #[serde(default)]
    pub int: Vec<String>,
    #[serde(default)]
    pub float: Vec<String>,
    #[serde(default)]
    pub bool: Vec<String>,
    #[serde(default)]
    pub string: Vec<String>,
    #[serde(default)]
    pub list: Vec<String>,
    #[serde(default)]
    pub object: Vec<String>,
    #[serde(default)]
    pub wrappers: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LanguageDefinition {
    pub name: String,
    #[serde(default)]
    pub zed: Vec<String>,
    pub extensions: Vec<String>,
    #[serde(default)]
    pub decorators: Vec<String>,
    #[serde(default)]
    pub class_keywords: Vec<String>,
    #[serde(default)]
    pub definitions: Vec<String>,
    #[serde(default)]
    pub handler_refs: Vec<String>,
    #[serde(default)]
    pub variables: Vec<String>,
    #[serde(default)]
    pub models: Option<ModelsDefinition>,
    /// Kept as JSON until each framework is read, so that an error names the framework it is in.
    pub frameworks: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModelsDefinition {
    pub declarations: Vec<String>,
    #[serde(default)]
    pub block: Block,
    #[serde(default)]
    pub modifiers: Vec<String>,
    pub fields: Vec<String>,
}

/// How the body of a model declaration is delimited.
#[derive(Deserialize, Default, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Block {
    #[default]
    Braces,
    Indent,
}

/// Where a framework's route prefix applies.
#[derive(Deserialize, Default, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// To the routes of the class the prefix annotates.
    #[default]
    Class,
    /// To every route in the file.
    File,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FrameworkDefinition {
    pub name: String,
    #[serde(default)]
    pub docs: Option<String>,
    #[serde(default)]
    pub imports: Vec<String>,
    #[serde(default)]
    pub prefix: Vec<String>,
    #[serde(default)]
    pub prefix_scope: Scope,
    /// Where a client's requests go (`@FeignClient(url = "${catalog.url}")`), at class level:
    /// `{path}` is the URL (placeholders resolved from the project's configuration), or
    /// `{name}` the client name, whose `spring.cloud.openfeign.client.config.<name>.url` is used.
    #[serde(default)]
    pub base: Vec<String>,
    #[serde(default)]
    pub controller_token: Option<String>,
    pub routes: Vec<RouteEntry>,
    #[serde(default)]
    pub path_params: Vec<Style>,
    #[serde(default)]
    pub query: Vec<String>,
    #[serde(default)]
    pub headers: Vec<String>,
    #[serde(default)]
    pub header_case: Option<HeaderCase>,
    #[serde(default)]
    pub body: Vec<String>,
    #[serde(default)]
    pub params: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub auth: Vec<String>,
    #[serde(default)]
    pub examples: Vec<Example>,
}

/// A route is a bare pattern or a pattern with options.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub(super) enum RouteEntry {
    Pattern(String),
    Full(RouteDefinition),
}

impl RouteEntry {
    pub fn into_definition(self) -> RouteDefinition {
        match self {
            RouteEntry::Pattern(pattern) => RouteDefinition { pattern, ..Default::default() },
            RouteEntry::Full(definition) => definition,
        }
    }
}

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(deny_unknown_fields)]
pub struct RouteDefinition {
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

/// A way of writing a path parameter in a route.
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

/// How a parameter name becomes a header name.
#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum HeaderCase {
    /// `x_request_id` → `X-Request-Id`
    Kebab,
}

/// A snippet of code and the requests that must be found in it (checked by the tests).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Example {
    pub code: String,
    pub expect: Vec<String>,
}
