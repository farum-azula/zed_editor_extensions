//! Unit tests for `vars.rs`.

use std::collections::HashMap;
use std::fs;
use zed_http::vars::{Context, Environments};

#[test]
fn dynamic_variables() {
    let ctx = Context { vars: HashMap::new(), ..Default::default() };
    let uuid = ctx.resolve("{{$uuid}}").unwrap();
    assert_eq!(uuid.len(), 36);
    assert_eq!(uuid.chars().nth(14), Some('4'), "version 4");
    assert_eq!(ctx.resolve("{{$random.uuid}}").unwrap().len(), 36);
    assert!(ctx.resolve("{{$timestamp}}").unwrap().parse::<i64>().is_ok());
    assert!(ctx.resolve("{{$isoTimestamp}}").unwrap().ends_with('Z'));
    let n: i64 = ctx.resolve("{{$randomInt 5 10}}").unwrap().parse().unwrap();
    assert!((5..10).contains(&n), "{n}");
    let n: i64 = ctx.resolve("{{$random.integer 1, 3}}").unwrap().parse().unwrap();
    assert!((1..3).contains(&n), "{n}");
    let n: i64 = ctx.resolve("{{$randomInt}}").unwrap().parse().unwrap();
    assert!((0..1000).contains(&n), "default range: {n}");
    assert!(ctx.resolve("{{$random.float}}").unwrap().parse::<f64>().is_ok());
    assert_eq!(ctx.resolve("{{$random.alphabetic 8}}").unwrap().len(), 8);
    assert!(ctx.resolve("{{$random.alphanumeric}}").unwrap().chars().all(|c| c.is_ascii_alphanumeric()));
    assert!(ctx.resolve("{{$random.hexadecimal 4}}").unwrap().chars().all(|c| c.is_ascii_hexdigit()));
    std::env::set_var("ZED_HTTP_TEST_VAR", "from-env");
    assert_eq!(ctx.resolve("{{$processEnv.ZED_HTTP_TEST_VAR}}").unwrap(), "from-env");
    assert_eq!(ctx.resolve("{{$env.ZED_HTTP_TEST_VAR}}").unwrap(), "from-env");
    std::env::remove_var("ZED_HTTP_TEST_VAR");
    assert!(ctx.resolve("{{$processEnv.ZED_HTTP_MISSING}}").is_err());
    assert!(ctx.resolve("{{$nope}}").unwrap_err().contains("unknown dynamic variable"));
}

#[test]
fn malformed_and_recursive_references() {
    let vars = HashMap::from([("a".to_string(), "{{b}}".to_string()), ("b".to_string(), "{{a}}".to_string())]);
    let ctx = Context { vars, ..Default::default() };
    assert!(ctx.resolve("{{a}}").unwrap_err().contains("recursion too deep"));
    assert_eq!(ctx.resolve("text {{unterminated").unwrap(), "text {{unterminated", "left as is");
}

#[test]
fn response_values() {
    let responses = serde_json::from_str(
            r#"{"r": {"status": 201, "headers": {"X-Trace": "t-1"}, "body": {"list": [{"id": 9}], "flag": true, "name": "x"}}}"#,
        )
        .unwrap();
    let ctx = Context { vars: HashMap::new(), responses, ..Default::default() };
    assert_eq!(ctx.resolve("{{r.response.status}}").unwrap(), "201");
    assert_eq!(ctx.resolve("{{r.response.headers.x-trace}}").unwrap(), "t-1");
    assert_eq!(ctx.resolve("{{r.response.body.name}}").unwrap(), "x");
    assert_eq!(ctx.resolve("{{r.response.body.flag}}").unwrap(), "true", "non-string values are stringified");
    assert_eq!(ctx.resolve("{{r.response.body.list[0].id}}").unwrap(), "9");
    assert_eq!(
        ctx.resolve("{{r.response.body}}").unwrap(),
        r#"{"list":[{"id":9},],"flag":true,"name":"x"}"#.replace(",]", "]")
    );
    assert!(ctx.resolve("{{r.response.headers.nope}}").is_err());
    assert!(ctx.resolve("{{r.response.body.list[9]}}").is_err());
    assert!(ctx.resolve("{{r.response.body.missing}}").is_err());
    assert!(ctx.resolve("{{r.response.whatever}}").is_err());
    assert!(ctx.resolve("{{other.response.status}}").unwrap_err().contains("run it first"));

    let responses = serde_json::from_str(r#"{"list": {"body": [{"id": 1}, {"id": 2}]}}"#).unwrap();
    let ctx = Context { vars: HashMap::new(), responses, ..Default::default() };
    assert_eq!(ctx.resolve("{{list.response.body[1].id}}").unwrap(), "2", "index at the root of the body");
}

#[test]
fn environment_files() {
    let dir = std::env::temp_dir().join(format!("zed-http-vars-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    assert!(Environments::load(&dir).unwrap().names.is_empty(), "no files: no environments");

    fs::write(dir.join("http-client.env.json"), r#"{"$shared": {"n": 1}, "prod": {"a": "1"}, "stage": {}}"#).unwrap();
    let envs = Environments::load(&dir).unwrap();
    assert_eq!(envs.names, vec!["prod", "stage"], "$shared is not an environment");
    assert_eq!(envs.select(None).unwrap().as_deref(), Some("prod"), "first environment by default");
    assert_eq!(envs.select(Some("stage")).unwrap().as_deref(), Some("stage"));
    assert!(envs.select(Some("nope")).unwrap_err().contains("not found"));
    assert_eq!(envs.vars(Some("prod")).get("n").map(String::as_str), Some("1"), "numbers become strings");

    fs::write(dir.join("http-client.private.env.json"), r#"{"prod": "not an object", "extra": {"k": "v"}}"#).unwrap();
    let envs = Environments::load(&dir).unwrap();
    assert_eq!(envs.vars(Some("prod")).get("a").map(String::as_str), Some("1"), "the scalar entry is ignored");
    assert_eq!(envs.vars(Some("extra")).get("k").map(String::as_str), Some("v"));
    fs::remove_file(dir.join("http-client.private.env.json")).unwrap();

    fs::write(dir.join("http-client.env.json"), r#"["not", "an object"]"#).unwrap();
    let err = Environments::load(&dir).err().unwrap();
    assert!(err.contains("expected a JSON object"), "{err}");

    fs::write(dir.join("http-client.env.json"), "{ broken").unwrap();
    assert!(Environments::load(&dir).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resolves_nested_and_responses() {
    let mut vars = HashMap::new();
    vars.insert("host".into(), "localhost:{{port}}".into());
    vars.insert("port".into(), "8080".into());
    let mut responses = serde_json::Map::new();
    responses.insert(
        "login".into(),
        serde_json::json!({"status": 200, "headers": {"X-Id": "7"}, "body": {"data": {"items": [{"token": "abc"}]}}}),
    );
    let ctx = Context { vars, responses, ..Default::default() };
    assert_eq!(ctx.resolve("http://{{host}}/x").unwrap(), "http://localhost:8080/x");
    assert_eq!(ctx.resolve("{{login.response.body.data.items[0].token}}").unwrap(), "abc");
    assert_eq!(ctx.resolve("{{login.response.body.$.data.items[0].token}}").unwrap(), "abc");
    assert_eq!(ctx.resolve("{{login.response.headers.x-id}}").unwrap(), "7");
    assert_eq!(ctx.resolve("{{$uuid}}").unwrap().len(), 36);
    assert!(ctx.resolve("{{missing}}").is_err());
}

#[test]
fn an_unreadable_environment_file_is_an_error() {
    let dir = std::env::temp_dir().join(format!("zed-http-unreadable-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("http-client.env.json")).unwrap();
    let err = Environments::load(&dir).err().unwrap();
    assert!(err.contains("http-client.env.json"), "{err}");
    let _ = fs::remove_dir_all(&dir);
}
