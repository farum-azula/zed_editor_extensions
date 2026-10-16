//! The inferred shape of a request (query, headers, auth, JSON body) and how it is rendered.
//! Inference itself is driven by `frameworks.json`, see `catalog.rs`.

use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Str,
    Int,
    Float,
    Bool,
    List,
    Obj(Vec<(String, Ty)>),
}

#[derive(Debug, Default, PartialEq)]
pub struct Spec {
    pub query: Vec<(String, Ty)>,
    pub headers: Vec<String>,
    pub auth: bool,
    pub body: Option<Vec<(String, Ty)>>,
}

pub fn sample(ty: &Ty) -> Value {
    match ty {
        Ty::Str => Value::String(String::new()),
        Ty::Int => Value::from(0),
        Ty::Float => Value::from(0.0),
        Ty::Bool => Value::Bool(false),
        Ty::List => Value::Array(vec![]),
        Ty::Obj(fields) => Value::Object(fields.iter().map(|(n, t)| (n.clone(), sample(t))).collect::<Map<_, _>>()),
    }
}

/// Value for a `@var = ...` declaration.
pub fn sample_var(ty: &Ty) -> &'static str {
    match ty {
        Ty::Int | Ty::Float => "1",
        Ty::Bool => "true",
        _ => "value",
    }
}

/// `X-Request-Id` → `xRequestId` (variable names can't contain `-`).
pub fn var_name(header: &str) -> String {
    let mut out = String::new();
    for (i, part) in header.split(['-', '_']).filter(|p| !p.is_empty()).enumerate() {
        let p = part.to_lowercase();
        if i == 0 {
            out.push_str(&p);
        } else {
            let mut cs = p.chars();
            out.extend(cs.next().map(|c| c.to_ascii_uppercase()));
            out.push_str(cs.as_str());
        }
    }
    out
}
