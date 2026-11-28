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
    /// Uploaded files (`MultipartFile`, `Uint8List`): a part of a multipart body read from a file.
    #[serde(default)]
    pub file: Vec<String>,
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
    /// `{name}` the client name, whose [`base_property`](Self::base_property) is used.
    #[serde(default)]
    pub base: Vec<String>,
    /// The property holding the URL of the client named `{name}`, by a `base` pattern or a
    /// registration: `spring.http.serviceclient.{name}.base-url`.
    #[serde(default)]
    pub base_property: Option<String>,
    /// Where client classes are registered under a name, in their file or anywhere in their
    /// project: `@ImportHttpServices(group: 'catalog', types: [BookService])`.
    #[serde(default)]
    pub registrations: Vec<RegistrationEntry>,
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
    pub cookies: Vec<String>,
    #[serde(default)]
    pub body: Vec<String>,
    /// Parts of a `multipart/form-data` body, capturing `{name}` and its `{type}`.
    #[serde(default)]
    pub parts: Vec<String>,
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

/// A registration is a bare pattern, which captures the `{name}` it registers under, or a
/// pattern with the name it stands for when it captures none (`default`).
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub(super) enum RegistrationEntry {
    Pattern(String),
    Named(NamedRegistration),
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct NamedRegistration {
    pub pattern: String,
    pub name: String,
}

impl RegistrationEntry {
    /// The pattern, and the name it registers under when it captures none.
    pub fn into_parts(self) -> (String, Option<String>) {
        match self {
            RegistrationEntry::Pattern(pattern) => (pattern, None),
            RegistrationEntry::Named(named) => (named.pattern, Some(named.name)),
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
