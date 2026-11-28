//! Unit tests for `server.rs` and the environment handling built on it (`endpoint::ensure_env`).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use zed_http::endpoint::{ensure_env, generate, local_server_for, LOCAL_SERVER};
use zed_http::server::*;

/// The resolution of a text to `text`, with `missing` keys left in it.
fn resolved(text: &str, missing: Vec<&str>) -> Resolution {
    Resolution { text: text.into(), missing: missing.into_iter().map(String::from).collect() }
}

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
    assert_eq!(resolve("${catalog.url}/v1", &p), resolved("https://c.example/v1", vec![]));
    assert_eq!(resolve("${url}", &p), resolved("http://h:1", vec![]));
    assert_eq!(resolve("${missing}/x", &p), resolved("${missing}/x", vec!["missing"]));
    assert_eq!(resolve("${a:${host}}", &p), resolved("h", vec![]));
    assert_eq!(variable_for("catalog.url"), "catalogUrl");
    assert_eq!(variable_for("spring.cloud.openfeign.client.config.mail.url"), "mailUrl");
    // Not `baseUrl`, the application's own address: the group names the variable.
    assert_eq!(variable_for("spring.http.serviceclient.catalog.base-url"), "catalogBaseUrl");
    assert_eq!(variable_for("api_url"), "apiUrl");
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
    // A line without a key is no entry.
    let keyless = parse_properties("=orphan\n : x\nkey=v\n");
    assert_eq!(keyless.len(), 1, "{keyless:?}");
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
    let content = generate(&catalog, code, &source, 3).unwrap().content;
    assert!(content.contains("gives http://localhost:9090; baseUrl in http-client.env.json is http://localhost:7000"));
    // The same address as the application's: nothing to say.
    fs::write(&env, "{\"dev\": {\"baseUrl\": \"http://localhost:9090\"}}").unwrap();
    let content = generate(&catalog, code, &source, 3).unwrap().content;
    assert!(!content.contains("gives"), "{content}");
}

/// Spring Cloud OpenFeign (Java): `@FeignClient(url = ...)` with the configuration's values.
#[test]
fn feign_urls_come_from_the_configuration() {
    let dir = project(
        "feign",
        &[("pom.xml", ""), ("src/main/resources/application.properties", "catalog.url=https://catalog.example\n")],
    );
    let source = dir.join("src/main/java/Catalog.java");
    let code = "import org.springframework.cloud.openfeign.FeignClient;\nimport org.springframework.web.bind.annotation.*;\n@FeignClient(name = \"catalog\", url = \"${catalog.url}\", path = \"/api\")\npublic interface Catalog {\n  @GetMapping(\"/books/{isbn}\")\n  Object find(@PathVariable String isbn);\n}\n";
    let catalog = zed_http::catalog::for_source(&source).unwrap();
    let endpoint = generate(&catalog, code, &source, 4).unwrap().endpoint;
    assert_eq!(endpoint.url, "https://catalog.example/api/books/{{isbn}}");
}

/// Spring for Dart's HTTP service clients: the interface is registered in a group by an
/// `@ImportHttpServices` in another file of the project, and the group's base URL is configured.
#[test]
fn http_service_clients_go_where_their_group_does() {
    let stale = "@ImportHttpServices(group: 'stale', types: [Stock])\n";
    let dir = project(
        "http-services",
        &[
            ("pubspec.yaml", "name: shop\n"),
            (
                "lib/resources/application.properties",
                "spring.http.serviceclient.supplier.base-url=${neon.supplier.url}\nneon.supplier.url=https://supplier.example/v1\n",
            ),
            (
                "lib/src/supplier/supplier_config.dart",
                "import 'package:spring_boot_starter_restclient/spring_boot_starter_restclient.dart';\n@Configuration()\n@ImportHttpServices(\n  group: 'supplier',\n  types: [\n    SupplierOffers,\n    SupplierDeliveries,\n  ],\n)\nclass SupplierClientConfig {}\n",
            ),
            // Not the project's sources: hidden directories, build output, other languages,
            // and the file's own copy on disk (the open document is what counts).
            (".dart_tool/build/generated.dart", stale),
            ("build/generated.dart", stale),
            ("lib/src/notes.txt", stale),
            ("lib/src/supplier/supplier_clients.dart", stale),
        ],
    );
    let source = dir.join("lib/src/supplier/supplier_clients.dart");
    let code = "import 'package:spring_boot_starter_restclient/spring_boot_starter_restclient.dart';\n@HttpExchange('/offers')\n@Retryable(\n  maxRetries: 2,\n)\nabstract interface class SupplierOffers {\n  @GetExchange('/{isbn}')\n  Future<Offer> offer(@PathVariable() String isbn, @RequestParam('copies') int copies);\n}\nabstract interface class Stock {\n  @GetExchange('/stock')\n  Future<int> stock();\n}\n";
    let catalog = zed_http::catalog::for_source(&source).unwrap();
    let offer = generate(&catalog, code, &source, 6).unwrap();
    assert_eq!(offer.endpoint.url, "https://supplier.example/v1/offers/{{isbn}}");
    assert!(offer.content.contains("/offers/{{isbn}}?copies={{copies}}"), "{}", offer.content);
    // Registered nowhere that counts: the application's own address.
    assert_eq!(generate(&catalog, code, &source, 10).unwrap().endpoint.url, "{{baseUrl}}/stock");
}

/// Spring's HTTP service clients (Java): the group of an `@ImportHttpServices` elsewhere,
/// matched by the whole class name, and its base URL from `application.yml`.
#[test]
fn java_http_service_clients_go_where_their_group_does() {
    let dir = project(
        "java-http-services",
        &[
            ("pom.xml", ""),
            (
                "src/main/resources/application.yml",
                "spring:\n  http:\n    serviceclient:\n      catalog:\n        base-url: http://catalog.internal:8081\n",
            ),
            (
                "src/main/java/com/shop/AClientConfig.java",
                "@ImportHttpServices(group = \"v2\", types = {BookClientV2.class, AuthorClient.class})\nclass AClientConfig {}\n",
            ),
            (
                "src/main/java/com/shop/ClientConfig.java",
                "@ImportHttpServices(group = \"catalog\", types = BookClient.class)\nclass ClientConfig {}\n",
            ),
            ("target/classes/Stale.java", "@ImportHttpServices(group = \"stale\", types = BookClient.class)\n"),
        ],
    );
    let source = dir.join("src/main/java/com/shop/BookClient.java");
    let code = "import org.springframework.web.service.annotation.*;\n@HttpExchange(\"/books\")\npublic interface BookClient {\n  @GetExchange(\"/{isbn}\")\n  Book find(@PathVariable String isbn);\n}\n";
    let catalog = zed_http::catalog::for_source(&source).unwrap();
    let endpoint = generate(&catalog, code, &source, 3).unwrap().endpoint;
    assert_eq!(endpoint.url, "http://catalog.internal:8081/books/{{isbn}}");
}

#[test]
fn edge_cases_of_placeholders_and_config_files() {
    let p = props(&[("loop", "${loop}"), ("a:b", "colon")]);
    // A placeholder that refers to itself stops after ten levels.
    assert_eq!(resolve("${loop}", &p).text, "${loop}");
    // An unclosed placeholder stays as it is.
    assert_eq!(resolve("x${open", &p), resolved("x${open", vec![]));
    // A nested placeholder in the key comes before the default's `:`.
    assert_eq!(resolve("${${k}:d}", &p).text, "d");
    // Nothing usable for a variable name.
    assert_eq!(variable_for("..."), "baseUrl");
    // A context path that already starts with a slash.
    assert_eq!(spring_base_url(&props(&[("server.servlet.context-path", "/api/")])), "http://localhost:8080/api");
    // A quoted YAML key holding a colon.
    let yaml = parse_yaml("\"a:b\": 1\n'c': 2\n");
    assert_eq!(yaml.get("a:b").map(String::as_str), Some("1"));
    assert_eq!(yaml.get("c").map(String::as_str), Some("2"));
}

#[test]
fn environment_files_ensure_env_leaves_alone_or_completes() {
    let dir = project(
        "env-odd",
        &[("pubspec.yaml", "name: shop\n"), ("lib/resources/application.properties", "server.port=9090\n")],
    );
    let source = dir.join("lib/src/api.dart");
    let base = dir.join(".zed/http");
    let env = base.join("http-client.env.json");
    fs::create_dir_all(&base).unwrap();
    // Not a JSON object: left alone.
    fs::write(&env, "[1, 2]").unwrap();
    assert_eq!(ensure_env(&base, &source).unwrap(), None);
    // No environment yet: a `dev` one is added.
    fs::write(&env, "{}").unwrap();
    assert_eq!(ensure_env(&base, &source).unwrap(), Some(env.clone()));
    assert!(fs::read_to_string(&env).unwrap().contains("\"dev\""));
    // No dev / local / development: the first environment gets the address.
    fs::write(&env, "{\"$shared\": {}, \"staging\": {\"token\": \"t\"}}").unwrap();
    assert_eq!(ensure_env(&base, &source).unwrap(), Some(env.clone()));
    let text = fs::read_to_string(&env).unwrap();
    assert!(text.contains("\"staging\"") && text.contains("http://localhost:9090"));
    // An environment that is not an object: left alone.
    fs::write(&env, "{\"dev\": 1}").unwrap();
    assert_eq!(ensure_env(&base, &source).unwrap(), None);
}

#[test]
fn feign_urls_without_a_scheme_or_a_value() {
    let dir = project(
        "feign-odd",
        &[("pom.xml", ""), ("src/main/resources/application.properties", "mail.host=localhost:9000\n")],
    );
    let source = dir.join("src/main/java/Mail.java");
    let catalog = zed_http::catalog::for_source(&source).unwrap();
    let client = |annotation: &str| {
        format!("import org.springframework.web.bind.annotation.*;\n{annotation}\ninterface Mail {{\n  @GetMapping(\"/inbox\")\n  Object inbox();\n}}\n")
    };
    // A configured address without a scheme: http.
    let endpoint = generate(&catalog, &client("@FeignClient(url = \"${mail.host}\")"), &source, 3).unwrap().endpoint;
    assert_eq!(endpoint.url, "http://localhost:9000/inbox");
    // An empty url and no name: the environment's baseUrl.
    let endpoint = generate(&catalog, &client("@FeignClient(url = \"\")"), &source, 3).unwrap().endpoint;
    assert_eq!(endpoint.url, "{{baseUrl}}/inbox");
}
