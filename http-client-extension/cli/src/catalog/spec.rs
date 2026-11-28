//! What a route's request carries — headers, query parameters, a JSON body, authorization —
//! inferred from the lines around it by a chain of [`Contributor`]s, one per concern. A new
//! concern is one more contributor and one more entry in [`CONTRIBUTORS`].

use super::compile::{Framework, Language};
use super::model::HeaderCase;
use super::routes::{header_values, resolve_placeholders, Route};
use super::types::TypeResolver;
use super::Catalog;
use crate::endpoint::{self, Endpoint};
use crate::infer::{Field, FieldType, RequestSpec};
use crate::pattern::{keys_of, Hit, Matcher};
use std::collections::BTreeSet;
use std::path::Path;

/// The route on a line and the request inferred for it.
#[derive(Debug)]
pub struct InferredRequest {
    pub endpoint: Endpoint,
    pub spec: RequestSpec,
    /// `language/framework` key of the route, e.g. `java/spring`.
    pub framework: String,
}

/// Methods whose requests have no body: what was read as body becomes query parameters.
const WITHOUT_BODY: [&str; 2] = ["GET", "HEAD"];
/// Methods whose requests always have a body, an empty one if nothing was read.
const WITH_BODY: [&str; 3] = ["POST", "PUT", "PATCH"];
/// Headers rendered from the auth flag and the body instead of a variable of their own.
const RENDERED_HEADERS: [&str; 2] = ["authorization", "content-type"];
/// Parameters that are the receiver of a method, not a request parameter.
const RECEIVERS: [&str; 2] = ["self", "cls"];

/// What the contributors read.
struct Source<'a> {
    framework: &'a Framework,
    language: &'a Language,
    types: TypeResolver<'a>,
    /// The whole file.
    text: &'a str,
    /// The route's lines, and its handler's.
    window: &'a str,
    /// `{{name}}` variables of the URL: path parameters, never query parameters.
    path_variables: &'a [String],
    /// Patterns of the whole catalog that mean a request is authorized.
    catalog_auth: Option<&'a Matcher>,
    /// The headers the route's class gives every route of the class.
    class_headers: &'a [(String, String)],
    /// The file, whose project's configuration fills in placeholders.
    path: &'a Path,
}

impl Source<'_> {
    /// Hits of the framework's patterns `matcher` (if it has any) in the window.
    fn hits<'m>(&'m self, matcher: &'m Option<Matcher>) -> impl Iterator<Item = Hit> + 'm {
        matcher.iter().flat_map(|matcher| matcher.hits(self.window))
    }
}

/// The request as the contributors build it up.
#[derive(Default)]
struct Draft {
    headers: Vec<String>,
    header_values: Vec<(String, String)>,
    cookies: Vec<String>,
    /// Parameter names that are known to be something else than a query parameter.
    claimed: Vec<String>,
    query: Vec<Field>,
    body: Vec<Field>,
    parts: Vec<Field>,
    auth: bool,
}

/// One concern of a request, read from the source into the draft.
trait Contributor {
    fn contribute(&self, source: &Source, draft: &mut Draft);
}

/// Headers and cookies come first: their parameter names must not be reported as query parameters.
const CONTRIBUTORS: [&dyn Contributor; 9] = [
    &Headers,
    &HeaderValues,
    &Cookies,
    &IgnoredNames,
    &QueryParameters,
    &BodyFields,
    &BodyParts,
    &SignatureParameters,
    &Authorization,
];

/// `@RequestHeader("X-Id")`, `req.header('X-Tenant')`, `headers: {...}`.
struct Headers;
/// `headers: ['X-Api=1']` on the route's annotation, over those of its class.
struct HeaderValues;
/// `@CookieValue("session")`.
struct Cookies;
/// `@RequestPart("file") MultipartFile file`: a part of a multipart body.
struct BodyParts;
/// Parameters the framework injects, which are no request parameters.
struct IgnoredNames;
/// `@RequestParam int page`, `req.query.page`, `queryParameters: {...}`.
struct QueryParameters;
/// `@RequestBody CreateUser body`, `const { name } = req.body`, `data: {...}`.
struct BodyFields;
/// Signature parameters (FastAPI-style): models go to the body, scalars to the query.
struct SignatureParameters;
/// Whether the request needs a token.
struct Authorization;

impl Contributor for Headers {
    fn contribute(&self, source: &Source, draft: &mut Draft) {
        for hit in source.hits(&source.framework.headers) {
            let names = match hit.get("keys") {
                Some(keys) => keys_of(keys),
                None => hit.get("name").map(|name| vec![name.to_string()]).unwrap_or_default(),
            };
            for name in names {
                draft.claimed.push(name.clone());
                let header = match source.framework.header_case {
                    Some(case) => case.apply(&name),
                    None => name,
                };
                if !draft.headers.iter().any(|known| known.eq_ignore_ascii_case(&header)) {
                    draft.headers.push(header);
                }
            }
        }
    }
}

impl Contributor for HeaderValues {
    fn contribute(&self, source: &Source, draft: &mut Draft) {
        let own: Vec<(String, String)> = source
            .hits(&source.framework.header_values)
            .flat_map(|hit| header_values(hit.get("keys").or(hit.get("path")).unwrap_or_default()))
            .map(|(name, value)| (name, resolve_placeholders(&value, source.path)))
            .collect();
        // As Spring: the route's own headers replace those of its class of the same name.
        let inherited: Vec<(String, String)> = source
            .class_headers
            .iter()
            .filter(|(name, _)| !own.iter().any(|(mine, _)| mine.eq_ignore_ascii_case(name)))
            .cloned()
            .collect();
        for (name, value) in inherited.into_iter().chain(own) {
            if !draft.header_values.iter().any(|(known, _)| known.eq_ignore_ascii_case(&name)) {
                draft.header_values.push((name, value));
            }
        }
    }
}

impl Contributor for Cookies {
    fn contribute(&self, source: &Source, draft: &mut Draft) {
        for hit in source.hits(&source.framework.cookies) {
            let Some(name) = hit.get("name") else { continue };
            draft.claimed.push(name.to_string());
            if !draft.cookies.iter().any(|known| known == name) {
                draft.cookies.push(name.to_string());
            }
        }
    }
}

impl Contributor for BodyParts {
    fn contribute(&self, source: &Source, draft: &mut Draft) {
        for hit in source.hits(&source.framework.parts) {
            fields_from(&hit, source).into_iter().for_each(|field| add_field(&mut draft.parts, field));
        }
    }
}

impl Contributor for IgnoredNames {
    fn contribute(&self, source: &Source, draft: &mut Draft) {
        let names = source.hits(&source.framework.ignore).filter_map(|hit| hit.get("name").map(str::to_string));
        draft.claimed.extend(names);
    }
}

impl Contributor for QueryParameters {
    fn contribute(&self, source: &Source, draft: &mut Draft) {
        for hit in source.hits(&source.framework.query) {
            fields_from(&hit, source).into_iter().for_each(|field| add_field(&mut draft.query, field));
        }
    }
}

impl Contributor for BodyFields {
    fn contribute(&self, source: &Source, draft: &mut Draft) {
        for hit in source.hits(&source.framework.body) {
            fields_from(&hit, source).into_iter().for_each(|field| add_field(&mut draft.body, field));
        }
    }
}

impl Contributor for SignatureParameters {
    fn contribute(&self, source: &Source, draft: &mut Draft) {
        for hit in source.hits(&source.framework.params) {
            let Some(name) = hit.get("name") else { continue };
            let claimed = draft.claimed.iter().any(|claimed| claimed == name);
            if claimed || RECEIVERS.contains(&name) || source.path_variables.iter().any(|path| path == name) {
                continue;
            }
            match source.types.field_type(hit.get("type").unwrap_or("str"), 0) {
                FieldType::Object(fields) if !fields.is_empty() => {
                    fields.into_iter().for_each(|field| add_field(&mut draft.body, field))
                }
                kind => add_field(&mut draft.query, Field::new(name, kind)),
            }
        }
    }
}

impl Contributor for Authorization {
    fn contribute(&self, source: &Source, draft: &mut Draft) {
        let patterns = [source.catalog_auth, source.framework.auth.as_ref()];
        draft.auth = patterns.iter().flatten().any(|auth| auth.is_match(source.window))
            || draft.headers.iter().any(|header| header.eq_ignore_ascii_case("authorization"));
    }
}

impl HeaderCase {
    /// The header a parameter named `name` reads.
    fn apply(self, name: &str) -> String {
        match self {
            HeaderCase::Kebab => kebab_header(name),
        }
    }
}

/// `x_request_id` → `X-Request-Id`.
fn kebab_header(parameter: &str) -> String {
    let capitalized = parameter.split('_').map(|part| {
        let mut chars = part.chars();
        chars.next().map_or(String::new(), |first| first.to_uppercase().chain(chars).collect())
    });
    capitalized.collect::<Vec<_>>().join("-")
}

/// Adds `field` unless it has no name or one of `fields` already has its name.
fn add_field(fields: &mut Vec<Field>, field: Field) {
    if !field.name.is_empty() && !fields.iter().any(|known| known.name == field.name) {
        fields.push(field);
    }
}

/// How the fields named by one query/body hit are read, by what the hit captured.
trait CaptureReader {
    /// The fields, or `None` when the hit did not capture what this reader reads.
    fn fields(&self, hit: &Hit, source: &Source) -> Option<Vec<Field>>;
}

/// `{keys}`: the keys of a literal, as strings.
struct KeysCapture;
/// `{var}`: the fields of the variable's declared type.
struct VariableCapture;
/// `{name}`, typed by `{type}` if there is one.
struct NameCapture;
/// `{type}`: the fields of the model.
struct TypeCapture;

/// Tried in order; the first reader that applies decides.
const CAPTURE_READERS: [&dyn CaptureReader; 4] = [&KeysCapture, &VariableCapture, &NameCapture, &TypeCapture];

fn fields_from(hit: &Hit, source: &Source) -> Vec<Field> {
    CAPTURE_READERS.iter().find_map(|reader| reader.fields(hit, source)).unwrap_or_default()
}

impl CaptureReader for KeysCapture {
    fn fields(&self, hit: &Hit, _source: &Source) -> Option<Vec<Field>> {
        let keys = keys_of(hit.get("keys")?);
        Some(keys.into_iter().map(|key| Field::new(key, FieldType::String)).collect())
    }
}

impl CaptureReader for VariableCapture {
    fn fields(&self, hit: &Hit, source: &Source) -> Option<Vec<Field>> {
        let variable = hit.get("var")?;
        let declared = source
            .language
            .declared_type(source.window, variable)
            .or_else(|| source.language.declared_type(source.text, variable));
        Some(declared.map(|type_name| source.types.model_fields(&type_name, 0)).unwrap_or_default())
    }
}

impl CaptureReader for NameCapture {
    fn fields(&self, hit: &Hit, source: &Source) -> Option<Vec<Field>> {
        let name = hit.get("name")?;
        let kind = hit.get("type").map_or(FieldType::String, |type_name| source.types.field_type(type_name, 1));
        Some(vec![Field::new(name, kind)])
    }
}

impl CaptureReader for TypeCapture {
    fn fields(&self, hit: &Hit, source: &Source) -> Option<Vec<Field>> {
        Some(source.types.model_fields(hit.get("type")?, 0))
    }
}

impl Draft {
    /// The spec of a `method` request: rendered headers left out, path parameters out of the
    /// query, and the body where the method has one: multipart if it has parts, else JSON.
    fn finish(self, method: &str, path_variables: &[String]) -> RequestSpec {
        let Draft { mut headers, mut header_values, cookies, mut query, body, parts, auth, .. } = self;
        let rendered = |header: &str| RENDERED_HEADERS.iter().any(|rendered| header.eq_ignore_ascii_case(rendered));
        headers.retain(|header| !rendered(header));
        // A header a parameter sends is the parameter's variable, not the annotation's value.
        header_values
            .retain(|(name, _)| !rendered(name) && !headers.iter().any(|header| header.eq_ignore_ascii_case(name)));
        query.retain(|field| !path_variables.contains(&field.name));
        let mut spec = RequestSpec { query, headers, header_values, cookies, auth, ..RequestSpec::default() };
        if WITHOUT_BODY.contains(&method) {
            body.into_iter().for_each(|field| add_field(&mut spec.query, field));
        } else if !parts.is_empty() {
            spec.parts = parts;
        } else if !body.is_empty() || WITH_BODY.contains(&method) {
            spec.body = Some(body);
        }
        spec
    }
}

impl Catalog {
    /// The route on `line` and the request inferred for it.
    pub fn request(&self, text: &str, path: &Path, line: usize) -> Option<InferredRequest> {
        let routes = self.routes(text, path);
        let route = routes.iter().find(|route| route.line == line)?;
        let route_lines: BTreeSet<usize> =
            routes.iter().filter(|route| !route.endpoint.weak).map(|route| route.line).collect();
        let spec = self.infer(text, path, route, &route_lines);
        Some(InferredRequest { endpoint: route.endpoint.clone(), spec, framework: route.framework.clone() })
    }

    fn infer(&self, text: &str, path: &Path, route: &Route, route_lines: &BTreeSet<usize>) -> RequestSpec {
        let language = &self.languages[route.language_index];
        let framework = &language.frameworks[route.framework_index];
        let lines: Vec<&str> = text.lines().collect();
        let mut window = language.window(&lines, route.line, route_lines);
        let handler = route
            .hit
            .get("handler")
            .map(str::to_string)
            .or_else(|| lines.get(route.line).and_then(|line| language.handler_on(line)));
        if let Some(definition) = handler.and_then(|handler| language.find_definition(&lines, &handler)) {
            window.push('\n');
            window.push_str(&language.window(&lines, definition, route_lines));
        }
        let path_variables = endpoint::url_vars(&route.endpoint.url);
        let source = Source {
            framework,
            language,
            types: TypeResolver { table: &self.types, language, text },
            text,
            window: &window,
            path_variables: &path_variables,
            catalog_auth: self.auth.as_ref(),
            class_headers: &route.class_headers,
            path,
        };
        let mut draft = Draft::default();
        for contributor in CONTRIBUTORS {
            contributor.contribute(&source, &mut draft);
        }
        draft.finish(&route.endpoint.method, &path_variables)
    }
}
