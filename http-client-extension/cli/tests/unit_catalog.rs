//! Unit tests for `catalog.rs`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use zed_http::catalog::{convert_params, Catalog, Language, Style, BUILTIN};
use zed_http::infer::{Spec, Ty};

fn ext_of(l: &Language) -> String {
    l.extensions.iter().find(|e| *e != "*").cloned().unwrap_or_else(|| "txt".into())
}

/// Every framework's `examples` in frameworks.json must produce exactly its `expect` list.
#[test]
fn framework_examples() {
    let c = Catalog::builtin();
    let mut results = Vec::new();
    for l in &c.languages {
        for f in &l.frameworks {
            assert!(!f.examples.is_empty(), "{}/{} has no examples", l.key, f.key);
            let key = format!("{}/{}", l.key, f.key);
            for ex in &f.examples {
                let path = PathBuf::from(format!("example.{}", ext_of(l)));
                let got: Vec<String> = c
                    .routes(&ex.code, &path)
                    .iter()
                    .filter(|r| r.framework == key)
                    .map(|r| format!("{} {}", r.endpoint.method, r.endpoint.url))
                    .collect();
                results.push((key.clone(), ex.code.clone(), ex.expect.clone(), got));
            }
        }
    }
    let failures: Vec<&(String, String, Vec<String>, Vec<String>)> =
        results.iter().filter(|(_, _, expect, got)| expect != got).collect();
    assert!(failures.is_empty(), "{} of {} examples failed: {failures:#?}", failures.len(), results.len());
}

/// Every framework must have an example file under `examples/code/` that `detect` attributes to it,
/// so the examples folder always covers the whole catalog.
#[test]
fn every_framework_has_an_example_file() {
    let c = Catalog::builtin();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/code");
    let mut found: BTreeSet<String> = BTreeSet::new();
    for lang_dir in fs::read_dir(&dir).expect("examples/code").flatten() {
        for file in fs::read_dir(lang_dir.path()).expect("language dir").flatten() {
            let path = file.path();
            let Ok(text) = fs::read_to_string(&path) else { continue };
            found.extend(c.routes(&text, &path).into_iter().map(|r| r.framework));
        }
    }
    let missing: Vec<String> = c
        .languages
        .iter()
        .flat_map(|l| l.frameworks.iter().map(move |f| format!("{}/{}", l.key, f.key)))
        // Weak-only matchers have no lens and no example of their own.
        .filter(|k| k != "*/url-literal" && !found.contains(k))
        .collect();
    assert!(missing.is_empty(), "no example file for: {}", missing.join(", "));
}

/// The language server is attached to languages by their Zed names listed in extension.toml.
#[test]
fn zed_languages_registered() {
    let manifest = include_str!("../../extension.toml");
    for l in &Catalog::builtin().languages {
        for z in &l.zed {
            assert!(manifest.contains(&format!("\"{z}\"")), "{z} ({}) is missing in extension.toml", l.key);
        }
    }
}

#[test]
fn project_catalog_extends_builtin() {
    let extra = r#"{ "languages": { "dart": { "frameworks": { "my-client": {
            "name": "My client", "imports": ["my_client.dart"],
            "routes": ["api.call(\"{method}\", \"{route}\""],
            "examples": [{ "code": "api.call('POST', '/x')", "expect": ["POST {{baseUrl}}/x"] }]
        } } } } }"#;
    let c = Catalog::from_sources(&[BUILTIN, extra]).unwrap();
    let dart = c.languages.iter().find(|l| l.key == "dart").unwrap();
    assert!(dart.frameworks.iter().any(|f| f.key == "dio"), "built-in frameworks are kept");
    let routes = c.routes("import 'my_client.dart';\napi.call('POST', '/x/$id');", Path::new("a.dart"));
    assert_eq!(routes[0].endpoint.url, "{{baseUrl}}/x/{{id}}");
    assert_eq!(routes[0].framework, "dart/my-client");

    let bad = r#"{ "languages": { "dart": { "frameworks": { "x": { "name": "x", "routes": ["{nope}"] } } } } }"#;
    let err = Catalog::from_sources(&[BUILTIN, bad]).err().unwrap();
    assert!(err.contains("languages.dart.frameworks.x.routes") && err.contains("{nope}"), "{err}");
    let typo = r#"{ "languages": { "dart": { "frameworks": { "x": { "name": "x", "routs": [] } } } } }"#;
    assert!(Catalog::from_sources(&[BUILTIN, typo]).err().unwrap().contains("routs"));
}

fn req(code: &str, file: &str, line: usize) -> Spec {
    let (_, spec, _) = Catalog::builtin().request(code, Path::new(file), line).expect("route on line");
    spec
}

fn names(v: &[(String, Ty)]) -> Vec<&str> {
    v.iter().map(|(n, _)| n.as_str()).collect()
}

#[test]
fn infer_spring() {
    let java = r#"import org.springframework.web.bind.annotation.*;
@RestController
@RequestMapping("/api/users")
public class C {
    @PostMapping("/{id}")
    @PreAuthorize("hasRole('ADMIN')")
    public User create(@PathVariable long id, @RequestParam(defaultValue = "1") int page,
                       @RequestHeader("X-Request-Id") String rid, @RequestBody CreateUser body) {}
}
public record CreateUser(String name, int age, List<String> tags, Address address) {}
class Address {
    private String city;
    private boolean primary;
    public String getCity() { return city; }
}
"#;
    let s = req(java, "C.java", 4);
    assert_eq!(s.query, vec![("page".into(), Ty::Int)]);
    assert_eq!(s.headers, vec!["X-Request-Id"]);
    assert!(s.auth);
    let body = s.body.unwrap();
    assert_eq!(names(&body), vec!["name", "age", "tags", "address"]);
    assert_eq!(body[3].1, Ty::Obj(vec![("city".into(), Ty::Str), ("primary".into(), Ty::Bool)]));
}

#[test]
fn infer_dart() {
    let dart = r#"import 'package:retrofit/retrofit.dart';
import 'package:dio/dio.dart';
class Item {
  final int id;
  final String title;
  final bool done;
}
abstract class TaskApi {
  @PUT('/tasks/{id}')
  Future<Task> update(@Path('id') String id, @Query('notify') bool notify, @Header('X-Trace') String t, @Body() Item item);
}
Future f(Dio dio, String token) => dio.post('/login', data: {'email': e, 'password': p}, options: Options(headers: {'Authorization': 'Bearer $token'}));
"#;
    let s = req(dart, "a.dart", 8);
    assert_eq!(s.query, vec![("notify".into(), Ty::Bool)]);
    assert_eq!(s.headers, vec!["X-Trace"]);
    assert_eq!(names(&s.body.unwrap()), vec!["id", "title", "done"]);
    let s = req(dart, "a.dart", 11);
    assert!(s.auth);
    assert_eq!(names(&s.body.unwrap()), vec!["email", "password"]);
}

#[test]
fn infer_fastapi() {
    let py = r#"from fastapi import FastAPI, Header
class ItemIn(BaseModel):
    name: str
    price: float
    tags: list[str] = []

@app.post("/items/{item_id}")
def create(item_id: int, item: ItemIn, q: str | None = None, x_token: str = Header(None),
           user: User = Depends(current_user)):
    pass
"#;
    let s = req(py, "main.py", 6);
    assert_eq!(s.query, vec![("q".into(), Ty::Str)]);
    assert_eq!(s.headers, vec!["X-Token"]);
    assert_eq!(s.body.unwrap(), vec![("name".into(), Ty::Str), ("price".into(), Ty::Float), ("tags".into(), Ty::List)]);
}

#[test]
fn infer_express_go_rust() {
    let ts = "import express from 'express';\napp.post('/users', auth, (req, res) => {\n  const { name, email } = req.body;\n  const page = req.query.page;\n  const t = req.header('X-Tenant');\n});";
    let s = req(ts, "s.ts", 1);
    assert_eq!(names(&s.body.unwrap()), vec!["name", "email"]);
    assert_eq!(names(&s.query), vec!["page"]);
    assert_eq!(s.headers, vec!["X-Tenant"]);

    let go = "import \"net/http\"\ntype Order struct {\n\tID    int    `json:\"id\"`\n\tItems []Item `json:\"items\"`\n}\nfunc main() {\n\tmux.HandleFunc(\"POST /orders\", createOrder)\n}\nfunc createOrder(w http.ResponseWriter, r *http.Request) {\n\tvar o Order\n\tjson.NewDecoder(r.Body).Decode(&o)\n\tr.URL.Query().Get(\"dry\")\n}";
    let s = req(go, "m.go", 6);
    assert_eq!(s.body.unwrap(), vec![("id".into(), Ty::Int), ("items".into(), Ty::List)]);
    assert_eq!(names(&s.query), vec!["dry"]);

    let rs = "use axum::Router;\n#[derive(Deserialize)]\nstruct CreateUser {\n    pub name: String,\n    admin: bool,\n}\nfn r() { Router::new().route(\"/users\", post(create_user)); }\nasync fn create_user(Json(body): Json<CreateUser>) {}";
    let s = req(rs, "m.rs", 6);
    assert_eq!(s.body.unwrap(), vec![("name".into(), Ty::Str), ("admin".into(), Ty::Bool)]);
}

/// A catalog for a made-up language, used to reach the corners of the engine.
const DEMO: &str = r#"{
      "languages": {
        "demo": {
          "name": "Demo",
          "extensions": ["demo"],
          "definitions": ["fn {handler}("],
          "variables": ["let {var}: {type}"],
          "models": {
            "declarations": ["model {type}"],
            "block": "braces",
            "modifiers": ["pub"],
            "fields": ["{name}: {type}"]
          },
          "frameworks": {
            "one": {
              "name": "Demo framework",
              "routes": ["route(\"{method}\", \"{route}\")"],
              "query": ["query({keys})"],
              "body": ["body({var})", "typed({type})", "readBody()"],
              "examples": [{ "code": "route(\"GET\", \"/x\")", "expect": ["GET {{baseUrl}}/x"] }]
            }
          }
        }
      }
    }"#;

fn demo() -> Catalog {
    Catalog::from_sources(&[BUILTIN, DEMO]).unwrap()
}

fn demo_spec(code: &str) -> Spec {
    demo().request(code, Path::new("x.demo"), 0).expect("a route on line 1").1
}

#[test]
fn body_sources_that_resolve_to_nothing() {
    // `{var}` whose type is never declared, and a pattern with no captures at all.
    let spec = demo_spec("route(\"POST\", \"/a\")\nbody(mystery)\nreadBody()");
    assert_eq!(spec.body, Some(vec![]));

    // A model name that is not a model: lowercase, or never declared.
    assert_eq!(demo_spec("route(\"POST\", \"/a\")\ntyped(lowercase)").body, Some(vec![]));
    assert_eq!(demo_spec("route(\"POST\", \"/a\")\ntyped(Unknown)").body, Some(vec![]));
}

#[test]
fn a_get_request_turns_body_fields_into_query_parameters() {
    let code = "route(\"GET\", \"/a\")\nbody(v)\nlet v: Filter\nmodel Filter { pub page: int\n tag: string }";
    let spec = demo_spec(code);
    assert_eq!(spec.body, None, "GET has no body");
    assert_eq!(spec.query, vec![("page".into(), Ty::Int), ("tag".into(), Ty::Str)]);
}

#[test]
fn model_declarations_that_are_skipped() {
    // A declaration followed by `;` before any `{` is a forward declaration, not a body.
    assert_eq!(demo_spec("route(\"POST\", \"/a\")\ntyped(Fwd)\nmodel Fwd;\nfn other() { }").body, Some(vec![]));
    // An unbalanced brace yields no fields.
    assert_eq!(demo_spec("route(\"POST\", \"/a\")\ntyped(Open)\nmodel Open { a: int").body, Some(vec![]));
    // A nested block inside a model is skipped, and its lines do not become fields.
    let code = "route(\"POST\", \"/a\")\ntyped(M)\nmodel M { a: int\n fn helper() {\n   hidden: string\n }\n b: bool }";
    assert_eq!(demo_spec(code).body, Some(vec![("a".into(), Ty::Int), ("b".into(), Ty::Bool)]));
}

#[test]
fn a_model_without_fields_is_a_string() {
    let code = "route(\"POST\", \"/a\")\ntyped(M)\nmodel M { inner: Empty }\nmodel Empty { }";
    assert_eq!(demo_spec(code).body, Some(vec![("inner".into(), Ty::Str)]));
}

#[test]
fn type_mapping_corners() {
    let c = demo();
    let lang = c.languages.iter().find(|l| l.key == "demo").unwrap();
    assert_eq!(c.ty_of(lang, "", "Map<String, String>", 0), Ty::Obj(vec![]), "a map is an open object");
    assert_eq!(c.ty_of(lang, "", "whatever", 0), Ty::Str, "unknown lowercase types are strings");
    assert_eq!(c.unwrap_type("Optional<Mono<Dto>>"), "Dto", "wrappers are unwrapped");
    assert_eq!(c.unwrap_type("Map<String, Dto>"), "Map<String, Dto>", "non-wrappers are kept");
}

#[test]
fn annotated_model_fields_and_empty_prefixes() {
    let java = r#"
import org.springframework.web.bind.annotation.*;
@RestController
@RequestMapping("")
public class C {
    @PostMapping
    User create(@RequestBody Optional<Dto> body) { return null; }
    @GetMapping("/x")
    User get() { return null; }
}
class Dto {
    @JsonProperty("renamed")
    private String name;
    private Map<String, String> extra;
}
"#;
    let c = Catalog::builtin();
    let routes = c.routes(java, Path::new("C.java"));
    let urls: Vec<&str> = routes.iter().map(|r| r.endpoint.url.as_str()).collect();
    assert_eq!(urls, vec!["{{baseUrl}}/", "{{baseUrl}}/x"], "an empty class prefix still yields a path");
    let (_, spec, _) = c.request(java, Path::new("C.java"), routes[0].line).unwrap();
    assert_eq!(spec.body, Some(vec![("name".into(), Ty::Str), ("extra".into(), Ty::Obj(vec![]))]));
}

#[test]
fn duplicates_are_collected_once() {
    // The same query parameter, header and model field appear twice in the window.
    let code =
        "route(\"POST\", \"/a\")\nquery({'page': 1})\nquery({'page': 2})\ntyped(M)\nmodel M { a: int\n a: string }";
    let spec = demo_spec(code);
    assert_eq!(spec.query, vec![("page".into(), Ty::Str)]);
    assert_eq!(spec.body, Some(vec![("a".into(), Ty::Int)]), "the first declaration wins");

    let java = "import org.springframework.web.bind.annotation.*;\n@PostMapping(\"/x\")\nvoid a(@RequestHeader(\"X-Id\") String a, @RequestHeader(\"x-id\") String b) {}";
    let (_, spec, _) = Catalog::builtin().request(java, Path::new("C.java"), 1).unwrap();
    assert_eq!(spec.headers, vec!["X-Id"], "headers are matched case-insensitively");
}

#[test]
fn a_delete_without_a_body_has_none() {
    let spec = demo_spec("route(\"DELETE\", \"/a\")");
    assert_eq!(spec.body, None);
    assert_eq!(demo_spec("route(\"OPTIONS\", \"/a\")").body, None);
}

#[test]
fn a_controller_token_without_a_class_is_left_alone() {
    // The prefix is class-level, but the declaration is a record: there is no class name to use.
    let cs = "using Microsoft.AspNetCore.Mvc;\n[Route(\"api/[controller]\")]\npublic record UsersEndpoint {\n    [HttpGet(\"{id}\")]\n    void Get() {}\n}";
    let routes = Catalog::builtin().routes(cs, Path::new("X.cs"));
    assert_eq!(routes[0].endpoint.url, "{{baseUrl}}/api/[controller]/{{id}}");

    // Without a declaration at all the prefix does not apply.
    let cs = "using Microsoft.AspNetCore.Mvc;\n[Route(\"api/[controller]\")]\n[HttpGet(\"{id}\")]\nvoid Get() {}";
    let routes = Catalog::builtin().routes(cs, Path::new("X.cs"));
    assert_eq!(routes[0].endpoint.url, "{{baseUrl}}/{{id}}");
}

#[test]
fn project_catalog_can_replace_scalar_fields() {
    let over = r#"{"languages": {"demo": {"name": "Renamed"}}}"#;
    let c = Catalog::from_sources(&[BUILTIN, DEMO, over]).unwrap();
    assert_eq!(c.languages.iter().find(|l| l.key == "demo").unwrap().name, "Renamed");
}

#[test]
fn path_params() {
    assert_eq!(convert_params("/a/{id}/b/{n:int}/{o?}", &[Style::Braces]), "/a/{{id}}/b/{{n}}/{{o}}");
    assert_eq!(convert_params("$base/x/${item.id}/$y", &[Style::Dollar]), "{{base}}/x/{{id}}/{{y}}");
    assert_eq!(convert_params("/a/:id/<int:n>", &[Style::Colon, Style::Angle]), "/a/{{id}}/{{n}}");
    assert_eq!(convert_params("http://h:8080/x", &[Style::Colon]), "http://h:8080/x");
}

#[test]
fn catalog_loading_errors() {
    let dir = std::env::temp_dir().join(format!("zed-http-catalog-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let err = Catalog::for_root(&dir).err().unwrap();
    assert!(err.contains("frameworks.json"), "{err}");

    let bad_language = r#"{"languages": {"demo": {"name": 42}}}"#;
    let err = Catalog::from_sources(&[bad_language]).err().unwrap();
    assert!(err.contains("languages.demo"), "{err}");

    let bad_type = r#"{"types": "not an object", "languages": {}}"#;
    assert!(Catalog::from_sources(&[bad_type]).is_err(), "a wrong field type is reported");

    let no_routes = r#"{"languages": {"demo": {"name": "D", "extensions": ["d"], "frameworks":
        {"f": {"name": "F", "routes": []}}}}}"#;
    let err = Catalog::from_sources(&[no_routes]).err().unwrap();
    assert!(err.contains("routes: empty"), "{err}");

    let no_fields = r#"{"languages": {"demo": {"name": "D", "extensions": ["d"],
        "models": {"declarations": ["class {type}"], "fields": []},
        "frameworks": {"f": {"name": "F", "routes": ["get(\"{route}\")"]}}}}}"#;
    let err = Catalog::from_sources(&[no_fields]).err().unwrap();
    assert!(err.contains("models.fields: empty"), "{err}");
    let _ = fs::remove_dir_all(&dir);
}

/// A call that passes its URL in a variable: the variable's value, or its name, has to say that
/// it holds a URL — otherwise the call is not a request at all.
#[test]
fn urls_held_in_variables() {
    let c = Catalog::builtin();
    let found = |code: &str| {
        c.routes(code, Path::new("app.dart"))
            .iter()
            .filter(|r| !r.endpoint.weak)
            .map(|r| format!("{} {}", r.endpoint.method, r.endpoint.url))
            .collect::<Vec<_>>()
    };
    let dart = "import 'package:http/http.dart' as http;\n";

    // Assigned a URL somewhere in the file.
    let code = format!("{dart}final base = 'https://api.example.com/v1';\nf() => http.get(base);\n");
    assert_eq!(found(&code), ["GET https://api.example.com/v1"]);

    // Assigned something that is not a URL: the name stands in for it, to be filled in later.
    let code = format!("{dart}final apiUrl = 'production';\nf() => http.get(apiUrl);\n");
    assert_eq!(found(&code), ["GET {{apiUrl}}"]);

    // Never assigned here, but named like a URL.
    let code = format!("{dart}f(Uri apiUrl) => http.get(apiUrl);\n");
    assert_eq!(found(&code), ["GET {{apiUrl}}"]);

    // Neither: not a request.
    let code = format!("{dart}f(Map headers) => headers.get(key);\n");
    assert_eq!(found(&code), Vec::<String>::new());
}

/// Without a `urls` section there is nothing to look up, and only the variable name is left.
#[test]
fn urls_without_a_lookup_section() {
    let extra = r#"{ "languages": { "toy": {
        "name": "Toy", "extensions": ["toy"], "frameworks": { "client": {
            "name": "Toy client", "routes": [".{method}({ref})"]
        } } } } }"#;
    let c = Catalog::from_sources(&[extra]).unwrap();
    let routes = c.routes("final endpoint = 'https://example.com';\nclient.get(endpoint);\n", Path::new("a.toy"));
    assert_eq!(routes[0].endpoint.url, "{{endpoint}}");
    assert!(c.routes("client.get(payload);\n", Path::new("a.toy")).is_empty());
}

/// Every tag set in `runnables.scm` is bound to a task in `tasks.json`, or its ▶ would do nothing.
#[test]
fn every_runnable_has_a_task() {
    let query = include_str!("../../languages/http/runnables.scm");
    let tasks: serde_json::Value = serde_json::from_str(include_str!("../../languages/http/tasks.json")).unwrap();
    let tagged: BTreeSet<&str> = tasks
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|t| t["tags"].as_array().into_iter().flatten())
        .filter_map(|t| t.as_str())
        .collect();
    let tags: BTreeSet<&str> = query.split("(#set! tag ").skip(1).filter_map(|rest| rest.split(')').next()).collect();
    assert_eq!(tags, BTreeSet::from(["http-request", "http-request-named"]));
    assert_eq!(tags, tagged);
}
