//! Unit tests for `server.rs` and the environment handling built on it (`endpoint::ensure_env`).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use zed_http::endpoint::{ensure_env, generate, local_server_for, LOCAL_SERVER};
use zed_http::server::*;

fn props(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("zed-http-server-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for (path, text) in files {
        let file = dir.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }
    dir
}

#[test]
fn base_url_from_spring_properties() {
    assert_eq!(spring_base_url(&props(&[])), "http://localhost:8080");
    assert_eq!(spring_base_url(&props(&[("server.port", "9090")])), "http://localhost:9090");
    assert_eq!(spring_base_url(&props(&[("server.port", "${PORT:7000}")])), "http://localhost:7000");
    assert_eq!(spring_base_url(&props(&[("server.port", "0")])), "http://localhost:8080");
    assert_eq!(
        spring_base_url(&props(&[("server.port", "8443"), ("server.ssl.certificate", "cert.pem")])),
        "https://localhost:8443"
    );
    assert_eq!(
        spring_base_url(&props(&[("server.ssl.key-store", "ks.p12"), ("server.ssl.enabled", "false")])),
        "http://localhost:8080"
    );
    assert_eq!(
        spring_base_url(&props(&[("server.servlet.context-path", "shop/"), ("server.address", "0.0.0.0")])),
        "http://localhost:8080/shop"
    );
    assert_eq!(spring_base_url(&props(&[("server.address", "api.local")])), "http://api.local:8080");
}

#[test]
fn placeholders() {
    let p = props(&[("catalog.url", "https://c.example"), ("host", "h"), ("url", "http://${host}:${port:1}")]);
    assert_eq!(resolve("${catalog.url}/v1", &p), ("https://c.example/v1".into(), vec![]));
    assert_eq!(resolve("${url}", &p), ("http://h:1".into(), vec![]));
    assert_eq!(resolve("${missing}/x", &p), ("${missing}/x".into(), vec!["missing".into()]));
    assert_eq!(resolve("${a:${host}}", &p), ("h".into(), vec![]));
    assert_eq!(variable_for("catalog.url"), "catalogUrl");
    assert_eq!(variable_for("spring.cloud.openfeign.client.config.mail.url"), "mailUrl");
}

#[test]
fn yaml_and_properties() {
    let yaml = parse_yaml(
        "server:\n  port: 9090 # main\n  servlet:\n    context-path: \"/shop\"\nlist:\n  - a\nmulti: |\n  x\n---\nspring:\n  config:\n    activate:\n      on-profile: prod\nserver:\n  port: 80\n",
    );
    assert_eq!(yaml.get("server.port").map(String::as_str), Some("9090"));
    assert_eq!(yaml.get("server.servlet.context-path").map(String::as_str), Some("/shop"));
    assert!(!yaml.contains_key("multi"));
    let properties = parse_properties("# c\nserver.port=9191\nserver.address : 127.0.0.1\nlong=a\\\n  b\n");
    assert_eq!(properties.get("server.port").map(String::as_str), Some("9191"));
    assert_eq!(properties.get("server.address").map(String::as_str), Some("127.0.0.1"));
    assert_eq!(properties.get("long").map(String::as_str), Some("ab"));
}

#[test]
fn project_configuration_properties_win_over_yaml() {
    let dir = project(
        "config",
        &[
            ("pubspec.yaml", "name: shop\n"),
            ("lib/resources/application.yml", "server:\n  port: 9090\n"),
            ("lib/resources/application.properties", "server.port=9191\n"),
            ("lib/src/api.dart", ""),
        ],
    );
    assert_eq!(local_server_for(&dir.join("lib/src/api.dart")).as_deref(), Some("http://localhost:9191"));
    let java = project("java", &[("pom.xml", ""), ("src/main/resources/application.properties", "server.port=8181")]);
    assert_eq!(local_server_for(&java.join("src/main/java/A.java")).as_deref(), Some("http://localhost:8181"));
    let none = project("none", &[("pubspec.yaml", "name: x\n")]);
    assert_eq!(local_server_for(&none.join("lib/a.dart")), None);
}

#[test]
fn the_environment_follows_the_application() {
    let dir = project(
        "env",
        &[("pubspec.yaml", "name: shop\n"), ("lib/resources/application.yml", "server:\n  port: 9090\n")],
    );
    let source = dir.join("lib/src/api.dart");
    let base = dir.join(".zed/http");
    let env = base.join("http-client.env.json");
    // Created with the application's address.
    assert_eq!(ensure_env(&base, &source).unwrap(), Some(env.clone()));
    assert!(fs::read_to_string(&env).unwrap().contains("http://localhost:9090"));
    // The generic default is replaced, other values kept.
    fs::write(&env, format!("{{\"dev\": {{\"baseUrl\": \"{LOCAL_SERVER}\", \"token\": \"t\"}}, \"prod\": {{}}}}"))
        .unwrap();
    assert_eq!(ensure_env(&base, &source).unwrap(), Some(env.clone()));
    let text = fs::read_to_string(&env).unwrap();
    assert!(text.contains("http://localhost:9090") && text.contains("\"token\": \"t\"") && text.contains("prod"));
    // A hand-set address is kept, and a generated request says where the application listens.
    fs::write(&env, "{\"dev\": {\"baseUrl\": \"http://localhost:7000\"}}").unwrap();
    assert_eq!(ensure_env(&base, &source).unwrap(), None);
    let code = "import 'package:spring_boot_starter_web/spring_boot_starter_web.dart';\n@RestController()\nclass C {\n  @GetMapping('/x')\n  String x() => '';\n}\n";
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, code).unwrap();
    let catalog = zed_http::catalog::for_source(&source).unwrap();
    let (_, _, content) = generate(&catalog, code, &source, 3).unwrap();
    assert!(content.contains("gives http://localhost:9090; baseUrl in http-client.env.json is http://localhost:7000"));
}

#[test]
fn feign_urls_come_from_the_configuration() {
    let dir = project(
        "feign",
        &[
            ("pubspec.yaml", "name: shop\n"),
            ("lib/resources/application.properties", "catalog.url=https://catalog.example\n"),
        ],
    );
    let source = dir.join("lib/src/client.dart");
    let code = "import 'package:spring_boot_starter_feign/spring_boot_starter_feign.dart';\n@FeignClient('catalog', url: r'${catalog.url}', path: '/api')\nabstract interface class Catalog {\n  @GetMapping('/books/{isbn}')\n  Future<Object> find(@PathVariable() String isbn);\n}\n";
    let catalog = zed_http::catalog::for_source(&source).unwrap();
    let (endpoint, _, _) = generate(&catalog, code, &source, 3).unwrap();
    assert_eq!(endpoint.url, "https://catalog.example/api/books/{{isbn}}");
}
