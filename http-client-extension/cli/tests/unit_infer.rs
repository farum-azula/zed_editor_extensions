//! Unit tests for `infer.rs`.

use serde_json::Value;
use zed_http::infer::{sample, sample_var, var_name, Ty};

#[test]
fn sample_values_per_type() {
    assert_eq!(sample(&Ty::Str), Value::String(String::new()));
    assert_eq!(sample(&Ty::Int), Value::from(0));
    assert_eq!(sample(&Ty::Float), Value::from(0.0));
    assert_eq!(sample(&Ty::Bool), Value::Bool(false));
    assert_eq!(sample(&Ty::List), Value::Array(vec![]));
    assert_eq!(sample(&Ty::Obj(vec![("a".into(), Ty::Int)])), serde_json::json!({"a": 0}));
    assert_eq!(sample_var(&Ty::Int), "1");
    assert_eq!(sample_var(&Ty::Float), "1");
    assert_eq!(sample_var(&Ty::Bool), "true");
    assert_eq!(sample_var(&Ty::Str), "value");
    assert_eq!(sample_var(&Ty::List), "value");
    assert_eq!(sample_var(&Ty::Obj(vec![])), "value");
}

#[test]
fn header_var_names() {
    assert_eq!(var_name("X-Request-Id"), "xRequestId");
    assert_eq!(var_name("x_token"), "xToken");
}
