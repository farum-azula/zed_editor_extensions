//! Unit tests for `infer.rs`.

use serde_json::Value;
use zed_http::infer::{variable_name, Field, FieldType};

#[test]
fn sample_values_per_type() {
    assert_eq!(FieldType::String.sample_value(), Value::String(String::new()));
    assert_eq!(FieldType::Integer.sample_value(), Value::from(0));
    assert_eq!(FieldType::Float.sample_value(), Value::from(0.0));
    assert_eq!(FieldType::Boolean.sample_value(), Value::Bool(false));
    assert_eq!(FieldType::List.sample_value(), Value::Array(vec![]));
    assert_eq!(
        FieldType::Object(vec![Field::new("a", FieldType::Integer)]).sample_value(),
        serde_json::json!({"a": 0})
    );
    assert_eq!(FieldType::Integer.sample_variable(), "1");
    assert_eq!(FieldType::Float.sample_variable(), "1");
    assert_eq!(FieldType::Boolean.sample_variable(), "true");
    assert_eq!(FieldType::String.sample_variable(), "value");
    assert_eq!(FieldType::List.sample_variable(), "value");
    assert_eq!(FieldType::Object(vec![]).sample_variable(), "value");
    // A file in a JSON body is a string.
    assert_eq!(FieldType::File.sample_value(), Value::String(String::new()));
    assert_eq!(FieldType::File.sample_variable(), "value");
}

#[test]
fn header_var_names() {
    assert_eq!(variable_name("X-Request-Id"), "xRequestId");
    assert_eq!(variable_name("x_token"), "xToken");
}
