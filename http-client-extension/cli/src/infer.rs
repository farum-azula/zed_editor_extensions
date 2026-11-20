//! The inferred shape of a request (query, headers, auth, JSON body) and the sample values it is
//! rendered with. Inference itself is driven by `frameworks.json`, see `catalog/spec.rs`.

use serde_json::{Map, Value};

/// The JSON shape of a source type, which decides the sample value a field gets.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    List,
    /// A model and its fields; an open object (a map) has none.
    Object(Vec<Field>),
}

/// A query parameter or a field of a JSON body.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub kind: FieldType,
}

impl Field {
    pub fn new(name: impl Into<String>, kind: FieldType) -> Field {
        Field { name: name.into(), kind }
    }
}

/// What a request needs besides its method and URL.
#[derive(Debug, Default, PartialEq)]
pub struct RequestSpec {
    pub query: Vec<Field>,
    pub headers: Vec<String>,
    pub auth: bool,
    pub body: Option<Vec<Field>>,
}

impl FieldType {
    /// The value a field of this type gets in a generated JSON body.
    pub fn sample_value(&self) -> Value {
        match self {
            FieldType::String => Value::String(String::new()),
            FieldType::Integer => Value::from(0),
            FieldType::Float => Value::from(0.0),
            FieldType::Boolean => Value::Bool(false),
            FieldType::List => Value::Array(vec![]),
            FieldType::Object(fields) => Value::Object(
                fields.iter().map(|field| (field.name.clone(), field.kind.sample_value())).collect::<Map<_, _>>(),
            ),
        }
    }

    /// The value of the `@name = ...` declaration of a parameter of this type.
    pub fn sample_variable(&self) -> &'static str {
        match self {
            FieldType::Integer | FieldType::Float => "1",
            FieldType::Boolean => "true",
            _ => "value",
        }
    }
}

/// `X-Request-Id` → `xRequestId` (variable names can't contain `-`).
pub fn variable_name(header: &str) -> String {
    let mut out = String::new();
    for (index, part) in header.split(['-', '_']).filter(|part| !part.is_empty()).enumerate() {
        let lower = part.to_lowercase();
        if index == 0 {
            out.push_str(&lower);
        } else {
            let mut chars = lower.chars();
            out.extend(chars.next().map(|first| first.to_ascii_uppercase()));
            out.push_str(chars.as_str());
        }
    }
    out
}
