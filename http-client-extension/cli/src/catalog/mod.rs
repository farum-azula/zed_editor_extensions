//! The catalog of supported languages and frameworks (`frameworks.json`) and the engine that
//! applies it: finding routes / HTTP calls in source code and inferring their requests.
//!
//! The built-in catalog is embedded at compile time; a project can extend or override it with
//! `.zed/http/frameworks.json` (merged key by key: languages → frameworks → fields).
//!
//! | Module      | What it does                                                        |
//! |-------------|---------------------------------------------------------------------|
//! | `model`     | the shape of `frameworks.json` (serde definitions)                  |
//! | `compile`   | definitions → compiled languages and frameworks                     |
//! | `source`    | lines, classes, decorators, definitions, the lines of a route       |
//! | `routes`    | routes found in a file and the endpoint each stands for             |
//! | `urls`      | the URL a variable holds                                            |
//! | `registrations` | the name a client class is registered under, here or elsewhere  |
//! | `types`     | source types → JSON shapes, fields of models                        |
//! | `spec`      | what a route's request carries, by a chain of contributors          |

mod compile;
mod model;
mod registrations;
mod routes;
mod source;
mod spec;
mod types;
mod urls;

pub use compile::{Framework, Language};
pub use model::{Block, Example, HeaderCase, RouteDefinition, Scope, Style};
pub use routes::{convert_params, Route};
pub use spec::InferredRequest;

use crate::infer::FieldType;
use crate::pattern::Matcher;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use types::{TypeResolver, TypeTable};
use urls::Urls;

pub const BUILTIN: &str = include_str!("../../../frameworks.json");
pub const PROJECT_FILE: &str = "frameworks.json";

/// The compiled catalog.
pub struct Catalog {
    types: TypeTable,
    auth: Option<Matcher>,
    urls: Urls,
    pub languages: Vec<Language>,
}

impl Catalog {
    /// The built-in catalog. It is validated by tests, so failing here is a build defect.
    pub fn builtin() -> &'static Catalog {
        static BUILT_IN: OnceLock<Catalog> = OnceLock::new();
        BUILT_IN.get_or_init(|| Catalog::from_sources(&[BUILTIN]).expect("the built-in frameworks.json is valid"))
    }

    /// Built-in catalog extended with `<root>/.zed/http/frameworks.json`, if present.
    pub fn for_root(root: &Path) -> Result<Catalog, String> {
        let path = project_file(root);
        let extra = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        Catalog::from_sources(&[BUILTIN, &extra]).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// The catalog of `sources` merged in order: the built-in one first, then the project's.
    pub fn from_sources(sources: &[&str]) -> Result<Catalog, String> {
        compile::compile_catalog(sources)
    }

    fn language_for(&self, path: &Path) -> Option<usize> {
        let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or_default().to_lowercase();
        self.languages.iter().position(|language| language.extensions.contains(&extension))
    }

    /// The language whose frameworks apply to files of any language (`"extensions": ["*"]`).
    fn any_language(&self) -> Option<usize> {
        self.languages.iter().position(|language| language.extensions.iter().any(|extension| extension == "*"))
    }

    /// Strips wrapper types (`Optional<T>`, `Json<T>`…) down to the type they carry.
    pub fn unwrap_type<'a>(&self, type_name: &'a str) -> &'a str {
        self.types.unwrap(type_name)
    }

    /// The JSON shape of the source type `type_name`, with the models declared in `text`.
    pub fn field_type(&self, language: &Language, text: &str, type_name: &str) -> FieldType {
        TypeResolver { table: &self.types, language, text }.field_type(type_name, 0)
    }

    /// Human-readable tree of the catalog.
    pub fn tree(&self) -> String {
        let mut out = String::new();
        for language in &self.languages {
            let extensions = language.extensions.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(" ");
            out.push_str(&format!("{} ({extensions})\n", language.name));
            for (index, framework) in language.frameworks.iter().enumerate() {
                let last = index + 1 == language.frameworks.len();
                let branch = if last { "└──" } else { "├──" };
                let when = if framework.imports.is_empty() {
                    "always".to_string()
                } else {
                    let imports = framework.imports.iter().map(|import| format!("`{import}`")).collect::<Vec<_>>();
                    format!("when the file contains {}", imports.join(" or "))
                };
                let count = framework.route_definitions.len();
                let plural = if count == 1 { "" } else { "s" };
                out.push_str(&format!(
                    "  {branch} {} [{}] — {count} route pattern{plural}, {when}\n",
                    framework.name, framework.key
                ));
                if let Some(docs) = &framework.docs {
                    let pad = if last { " " } else { "│" };
                    out.push_str(&format!("  {pad}     {docs}\n"));
                }
            }
        }
        out
    }
}

/// `<root>/.zed/http/frameworks.json`.
pub fn project_file(root: &Path) -> PathBuf {
    root.join(crate::vars::ZED_HTTP_DIR).join(PROJECT_FILE)
}

/// The catalog for a source file: built-in plus the nearest `.zed/http/frameworks.json` above it.
pub fn for_source(source: &Path) -> Result<Catalog, String> {
    let mut directory = source.parent();
    while let Some(current) = directory {
        if project_file(current).exists() {
            return Catalog::for_root(current);
        }
        directory = current.parent();
    }
    Catalog::from_sources(&[BUILTIN])
}
