//! From the definitions of `frameworks.json` ([`super::model`]) to the compiled catalog: the
//! sources merged, every pattern compiled once, errors named after where they are.

use super::model::{
    Block, CatalogFile, Example, FrameworkDefinition, HeaderCase, LanguageDefinition, ModelsDefinition,
    RouteDefinition, Scope, Style,
};
use super::routes::PathParams;
use super::types::TypeTable;
use super::urls::Urls;
use super::Catalog;
use crate::pattern::Matcher;
use regex::Regex;
use serde_json::Value;

/// A language of the catalog: how its source is laid out, and its frameworks.
pub struct Language {
    pub key: String,
    pub name: String,
    /// Zed's names for the language, under which the language server is attached to it.
    pub zed: Vec<String>,
    pub extensions: Vec<String>,
    pub(super) decorators: Vec<String>,
    pub(super) class_keywords: Option<Regex>,
    pub(super) definitions: Option<Matcher>,
    pub(super) definition_patterns: Vec<String>,
    pub(super) handler_refs: Option<Matcher>,
    pub(super) variables: Vec<String>,
    pub(super) models: Option<Models>,
    pub frameworks: Vec<Framework>,
}

/// How a language declares models (classes, structs, records…) and their fields.
pub(super) struct Models {
    pub declarations: Vec<String>,
    pub block: Block,
    pub modifiers: Vec<String>,
    pub fields: Matcher,
}

/// A framework or HTTP client: the patterns of its routes and of what a request carries.
pub struct Framework {
    pub key: String,
    pub name: String,
    pub docs: Option<String>,
    pub imports: Vec<String>,
    pub(super) prefix: Option<Matcher>,
    pub(super) prefix_scope: Scope,
    pub(super) base: Option<Matcher>,
    pub(super) controller_token: Option<String>,
    pub(super) routes: Matcher,
    pub(super) route_definitions: Vec<RouteDefinition>,
    pub(super) path_params: PathParams,
    pub(super) query: Option<Matcher>,
    pub(super) headers: Option<Matcher>,
    pub(super) header_case: Option<HeaderCase>,
    pub(super) body: Option<Matcher>,
    pub(super) params: Option<Matcher>,
    pub(super) ignore: Option<Matcher>,
    pub(super) auth: Option<Matcher>,
    pub examples: Vec<Example>,
}

/// Every path parameter style, in the order they are tried; a framework without a list of its
/// own accepts them all.
pub(super) const ALL_STYLES: [Style; 4] = [Style::Dollar, Style::Braces, Style::Angle, Style::Colon];

/// Merges `sources` (the built-in catalog first, then the project's) and compiles the result.
pub(super) fn compile_catalog(sources: &[&str]) -> Result<Catalog, String> {
    let mut merged: Option<Value> = None;
    for source in sources {
        let value: Value = serde_json::from_str(source).map_err(|error| format!("invalid JSON: {error}"))?;
        match merged.as_mut() {
            Some(base) => merge(base, value),
            None => merged = Some(value),
        }
    }
    let file: CatalogFile = serde_json::from_value(merged.unwrap_or(Value::Null)).map_err(|error| error.to_string())?;
    let mut languages = Vec::new();
    for (key, value) in file.languages {
        let definition: LanguageDefinition =
            serde_json::from_value(value).map_err(|error| format!("languages.{key}: {error}"))?;
        languages.push(compile_language(&key, definition)?);
    }
    Ok(Catalog {
        auth: compile_patterns(&file.auth, "auth")?,
        types: TypeTable::new(file.types),
        urls: Urls::compile(file.urls)?,
        languages,
    })
}

/// Recursively merges `over` into `base`: objects key by key, everything else replaced.
fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(base), Value::Object(over)) => {
            for (key, value) in over {
                match base.get_mut(&key) {
                    Some(slot) => merge(slot, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, over) => *base = over,
    }
}

/// Compiles a list of patterns, naming `context` (`languages.dart.frameworks.dio.query`) in errors.
pub(super) fn compile_patterns(patterns: &[String], context: &str) -> Result<Option<Matcher>, String> {
    Matcher::new(patterns, &[]).map_err(|error| format!("{context}: {error}"))
}

fn compile_language(key: &str, definition: LanguageDefinition) -> Result<Language, String> {
    let context = |what: &str| format!("languages.{key}.{what}");
    let mut frameworks = Vec::new();
    for (framework_key, value) in definition.frameworks {
        let framework_context = context(&format!("frameworks.{framework_key}"));
        let framework: FrameworkDefinition =
            serde_json::from_value(value).map_err(|error| format!("{framework_context}: {error}"))?;
        frameworks.push(compile_framework(&framework_context, &framework_key, framework)?);
    }
    let models = match definition.models {
        Some(models) => Some(compile_models(models, &context("models.fields"))?),
        None => None,
    };
    Ok(Language {
        key: key.to_string(),
        name: definition.name,
        zed: definition.zed,
        extensions: definition.extensions.iter().map(|extension| extension.to_lowercase()).collect(),
        decorators: definition.decorators,
        class_keywords: class_keywords(&definition.class_keywords),
        definitions: compile_patterns(&definition.definitions, &context("definitions"))?,
        definition_patterns: definition.definitions,
        handler_refs: compile_patterns(&definition.handler_refs, &context("handler_refs"))?,
        variables: definition.variables,
        models,
        frameworks,
    })
}

fn compile_models(models: ModelsDefinition, context: &str) -> Result<Models, String> {
    Ok(Models {
        fields: compile_patterns(&models.fields, context)?.ok_or_else(|| format!("{context}: empty"))?,
        declarations: models.declarations,
        block: models.block,
        modifiers: models.modifiers,
    })
}

/// A regex finding a declaration that starts with one of `keywords` (`class Name`, `struct Name`).
fn class_keywords(keywords: &[String]) -> Option<Regex> {
    if keywords.is_empty() {
        return None;
    }
    let alternatives = keywords.iter().map(|keyword| regex::escape(keyword)).collect::<Vec<_>>().join("|");
    // The keywords are escaped, so the pattern is always a valid regex.
    Some(Regex::new(&format!(r"\b(?:{alternatives})\s+\w")).expect("escaped keywords form a valid regex"))
}

fn compile_framework(context: &str, key: &str, framework: FrameworkDefinition) -> Result<Framework, String> {
    let route_definitions: Vec<RouteDefinition> =
        framework.routes.into_iter().map(|route| route.into_definition()).collect();
    let route_patterns: Vec<String> = route_definitions.iter().map(|route| route.pattern.clone()).collect();
    let compile = |patterns: &[String], what: &str| compile_patterns(patterns, &format!("{context}.{what}"));
    let styles = if framework.path_params.is_empty() { &ALL_STYLES[..] } else { &framework.path_params[..] };
    Ok(Framework {
        key: key.to_string(),
        name: framework.name,
        docs: framework.docs,
        imports: framework.imports,
        prefix: compile(&framework.prefix, "prefix")?,
        prefix_scope: framework.prefix_scope,
        base: compile(&framework.base, "base")?,
        controller_token: framework.controller_token,
        routes: compile(&route_patterns, "routes")?.ok_or_else(|| format!("{context}.routes: empty"))?,
        route_definitions,
        path_params: PathParams::new(styles),
        query: compile(&framework.query, "query")?,
        headers: compile(&framework.headers, "headers")?,
        header_case: framework.header_case,
        body: compile(&framework.body, "body")?,
        params: compile(&framework.params, "params")?,
        ignore: compile(&framework.ignore, "ignore")?,
        auth: compile(&framework.auth, "auth")?,
        examples: framework.examples,
    })
}
