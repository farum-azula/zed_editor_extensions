# HTTP Client for Zed

An HTTP client for the [Zed](https://zed.dev) editor, modeled on the built-in HTTP Client in IntelliJ IDEA.
Write requests in `.http` / `.rest` files, click ▶ next to a request, and the response appears in Zed's terminal.

The file format is compatible with IntelliJ IDEA / WebStorm / PhpStorm and largely with VS Code REST Client,
so existing `.http` files and `http-client.env.json` can be used unchanged.

---

## Supported Languages and Frameworks

48 frameworks and HTTP clients across 10 languages. In any of these files, every route or HTTP call
gets a **🌐 Generate .http request** lens that turns it into a ready `.http` request
(see [Generating a Request from Code](#generating-a-request-from-code)).
Each one has a runnable example in [`examples/code/`](examples/code), and the catalog itself lives in
[`frameworks.json`](frameworks.json) — adding a framework means adding patterns there, not writing code.

| Language | Files | Frameworks and clients |
|---|---|---|
| **Java** | `.java` | [Spring Web (MVC / WebFlux / OpenFeign)](https://docs.spring.io/spring-framework/reference/web/webmvc/mvc-controller/ann-requestmapping.html) · [example](examples/code/java/SpringUserController.java)<br>[JAX-RS (Quarkus, Jakarta EE, Helidon, Dropwizard)](https://quarkus.io/guides/rest) · [example](examples/code/java/JaxRsGreetingResource.java)<br>[Micronaut HTTP](https://docs.micronaut.io/latest/guide/#routing) · [example](examples/code/java/MicronautBookController.java)<br>[Retrofit](https://square.github.io/retrofit/) · [example](examples/code/java/RetrofitGitHubApi.java)<br>[OpenFeign (native annotations)](https://github.com/OpenFeign/feign) · [example](examples/code/java/FeignRepoClient.java) |
| **Kotlin** | `.kt`, `.kts` | [Spring Web (Kotlin)](https://docs.spring.io/spring-framework/reference/languages/kotlin.html) · [example](examples/code/kotlin/SpringOrderController.kt)<br>[Ktor server](https://ktor.io/docs/server-routing.html) · [example](examples/code/kotlin/KtorServerRoutes.kt)<br>[Ktor client](https://ktor.io/docs/client-requests.html) · [example](examples/code/kotlin/KtorClientApi.kt)<br>[Retrofit (Kotlin)](https://square.github.io/retrofit/) · [example](examples/code/kotlin/RetrofitUserApi.kt)<br>[JAX-RS (Quarkus Kotlin)](https://quarkus.io/guides/kotlin) · [example](examples/code/kotlin/JaxRsHelloResource.kt) |
| **Dart / Flutter** | `.dart` | [Dio](https://pub.dev/packages/dio) · [example](examples/code/dart/dio_client.dart)<br>[package:http](https://pub.dev/packages/http) · [example](examples/code/dart/http_client.dart)<br>[retrofit.dart](https://pub.dev/packages/retrofit) · [example](examples/code/dart/retrofit_api.dart)<br>[Chopper](https://pub.dev/packages/chopper) · [example](examples/code/dart/chopper_service.dart)<br>[shelf_router](https://pub.dev/packages/shelf_router) · [example](examples/code/dart/shelf_router_routes.dart) |
| **TypeScript / JavaScript** | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`, `.mts`, `.cts` | [Express](https://expressjs.com/en/guide/routing.html) · [example](examples/code/typescript/express_server.ts)<br>[Fastify](https://fastify.dev/docs/latest/Reference/Routes/) · [example](examples/code/typescript/fastify_server.ts)<br>[Hono](https://hono.dev/docs/api/routing) · [example](examples/code/typescript/hono_server.ts)<br>[Koa router](https://github.com/koajs/router) · [example](examples/code/typescript/koa_server.ts)<br>[NestJS](https://docs.nestjs.com/controllers) · [example](examples/code/typescript/nestjs_controller.ts)<br>[Axios](https://axios-http.com/docs/api_intro) · [example](examples/code/typescript/axios_client.ts)<br>[Angular HttpClient](https://angular.dev/guide/http/making-requests) · [example](examples/code/typescript/angular_service.ts)<br>[fetch](https://developer.mozilla.org/docs/Web/API/Fetch_API) · [example](examples/code/typescript/fetch_client.ts) |
| **Python** | `.py` | [FastAPI](https://fastapi.tiangolo.com/tutorial/first-steps/) · [example](examples/code/python/fastapi_main.py)<br>[Flask](https://flask.palletsprojects.com/en/stable/quickstart/#routing) · [example](examples/code/python/flask_app.py)<br>[Django URLconf](https://docs.djangoproject.com/en/stable/topics/http/urls/) · [example](examples/code/python/django_urls.py)<br>[requests / httpx / aiohttp](https://requests.readthedocs.io/) · [example](examples/code/python/http_clients.py) |
| **Go** | `.go` | [gorilla/mux](https://github.com/gorilla/mux) · [example](examples/code/go/gorilla_mux_server.go)<br>[net/http (server and client)](https://pkg.go.dev/net/http) · [example](examples/code/go/net_http_server.go)<br>[Gin](https://gin-gonic.com/docs/examples/) · [example](examples/code/go/gin_server.go)<br>[Echo](https://echo.labstack.com/docs/routing) · [example](examples/code/go/echo_server.go)<br>[chi](https://go-chi.io/#/pages/routing) · [example](examples/code/go/chi_server.go)<br>[Fiber](https://docs.gofiber.io/guide/routing) · [example](examples/code/go/fiber_server.go) |
| **Rust** | `.rs` | [axum](https://docs.rs/axum/latest/axum/routing/struct.Router.html#method.route) · [example](examples/code/rust/axum_server.rs)<br>[actix-web](https://actix.rs/docs/url-dispatch/) · [example](examples/code/rust/actix_server.rs)<br>[Rocket](https://rocket.rs/guide/v0.5/requests/) · [example](examples/code/rust/rocket_server.rs)<br>[reqwest](https://docs.rs/reqwest/latest/reqwest/) · [example](examples/code/rust/reqwest_client.rs) |
| **PHP** | `.php` | [Laravel](https://laravel.com/docs/routing) · [example](examples/code/php/laravel_routes.php)<br>[Symfony](https://symfony.com/doc/current/routing.html) · [example](examples/code/php/symfony_controller.php)<br>[Slim](https://www.slimframework.com/docs/v4/objects/routing.html) · [example](examples/code/php/slim_routes.php)<br>[Guzzle](https://docs.guzzlephp.org/en/stable/quickstart.html) · [example](examples/code/php/guzzle_client.php) |
| **Ruby** | `.rb` | [Rails routes](https://guides.rubyonrails.org/routing.html) · [example](examples/code/ruby/rails_routes.rb)<br>[Sinatra](https://sinatrarb.com/intro.html) · [example](examples/code/ruby/sinatra_app.rb)<br>[Faraday / HTTParty](https://lostisland.github.io/faraday/) · [example](examples/code/ruby/http_clients.rb) |
| **C#** | `.cs` | [ASP.NET Core controllers](https://learn.microsoft.com/aspnet/core/mvc/controllers/routing) · [example](examples/code/csharp/AspNetUsersController.cs)<br>[ASP.NET Core minimal APIs](https://learn.microsoft.com/aspnet/core/fundamentals/minimal-apis) · [example](examples/code/csharp/MinimalApi.cs)<br>[Refit](https://github.com/reactiveui/refit) · [example](examples/code/csharp/RefitGitHubApi.cs)<br>[HttpClient](https://learn.microsoft.com/dotnet/fundamentals/networking/http/httpclient) · [example](examples/code/csharp/HttpClientService.cs) |

`zed-http frameworks` prints this list from the catalog, including any project-local additions.

---

## Contents

- [Supported Languages and Frameworks](#supported-languages-and-frameworks)
- [Features](#features)
- [How It Works](#how-it-works)
- [Requirements](#requirements)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Running Requests](#running-requests)
- [.http File Syntax](#http-file-syntax)
- [Variables](#variables)
- [Environments](#environments)
- [Variable Navigation](#variable-navigation)
- [Request Chaining](#request-chaining)
- [Request Body](#request-body)
- [Working with Responses](#working-with-responses)
- [CLI zed-http](#cli-zed-http)
- [New Endpoint](#new-endpoint)
- [Generating a Request from Code](#generating-a-request-from-code)
- [Keyboard Shortcuts](#keyboard-shortcuts)
- [Project Structure](#project-structure)
- [Development](#development)
- [Limitations](#limitations)
- [Troubleshooting](#troubleshooting)

---

## Features

- **HTTP** language for `.http` and `.rest` files:
  - highlighting of methods, URLs, headers, `{{...}}` variables, HTTP versions, `###` separators;
  - JSON, XML, and GraphQL highlighting inside the request body;
  - outline panel (`cmd+shift+o`) listing the file's requests;
  - `#` and `//` comments (`cmd+/`), auto-closing of brackets and quotes;
  - **go to variable definition** via `cmd+click` / `F12`, and a hover showing the value.
- A **▶ button on every request** and Zed tasks to run one or all requests in a file.
- The **`zed-http`** runner:
  - any HTTP method, headers, body, multi-line URLs;
  - file variables, environments (`http-client.env.json` + private file), `$shared`;
  - dynamic variables: `$uuid`, `$timestamp`, `$isoTimestamp`, `$randomInt`, `$random.*`, `$processEnv.*`;
  - named requests and references to the body, headers, and status of previous responses;
  - body from a file (`< ./file.json`), file inclusion inside the body (multipart);
  - saving the response to a file (`>>` and `>>!`);
  - shorthand Basic authorization;
  - disabling redirects (`# @no-redirect`);
  - colored output: status, time, size, headers, formatted JSON;
  - auto-saving every response to `.zed-http/` for viewing in Zed.

## How It Works

The Zed extension API does not allow adding custom panels or windows, so the project consists of two parts:

| Part | Where | What it does |
|---|---|---|
| Zed extension | `extension.toml`, `languages/http/`, `src/lib.rs` | Registers the HTTP language, tree-sitter grammar, highlighting, outline, ▶ buttons, tasks, and starts the language server |
| `zed-http` CLI | `cli/` | Executes requests (`zed-http run`) and acts as a language server (`zed-http lsp`) for variable navigation and hover |

When you click ▶, Zed runs the task:

```sh
zed-http run "<file path>" --line <line number>
```

The runner finds the request containing that line, executes it, and prints the result in the built-in terminal.

When a `.http` file is opened, Zed starts `zed-http lsp`, which handles "go to definition" and hover.

## Requirements

- Zed (a recent version with support for tasks and runnables);
- Rust toolchain (`cargo`) to build the runner and the dev extension;
- macOS or Linux (Windows should work but is untested).

## Installation

### 1. Runner

```sh
git clone <this repository>
cd zed-editor-http-client-extension
cargo install --path cli
```

The `zed-http` binary is installed to `~/.cargo/bin`. Make sure that directory is in your `PATH`:

```sh
zed-http --help
```

> Zed runs tasks through your shell. If the command isn't found inside Zed,
> make sure `~/.cargo/bin` is added to `PATH` in `~/.zshrc` / `~/.bashrc`, not just in the current session.

### 2. Extension

1. Open Zed.
2. Command palette (`cmd+shift+p`) → **`zed: install dev extension`**.
3. Select the root folder of this repository.

Zed compiles the WASM part of the extension (requires `rustup`; Zed installs the `wasm32-wasip2` target itself)
and the tree-sitter-http grammar. After that, `.http` and `.rest` files open as the **HTTP** language,
and the **zed-http** language server appears at the bottom.

### Updating

```sh
git pull
cargo install --path cli --force
```

In Zed: `zed: extensions` → click **Rebuild** on the dev extension.

## Quick Start

Create `api.http`:

```http
@baseUrl = https://httpbin.org

### Get data
GET {{baseUrl}}/get?client=zed
Accept: application/json

### Create a record
POST {{baseUrl}}/post
Content-Type: application/json

{
  "id": "{{$uuid}}",
  "name": "Zed"
}
```

Click ▶ to the left of `GET`, and the terminal shows:

```
GET https://httpbin.org/get?client=zed
Accept: application/json

HTTP/1.1 200 OK  519 ms · 304 B
content-type: application/json
...

{
  "args": { "client": "zed" },
  ...
}

saved: ./.zed-http/line-3.json
```

A more complete example is in [`examples/requests.http`](examples/requests.http) and
[`examples/http-client.env.json`](examples/http-client.env.json).

## Running Requests

| Method | What runs |
|---|---|
| ▶ next to the method | This request |
| `task: spawn` → "HTTP: GET …" | The request under the cursor |
| `task: spawn` → "HTTP: run all requests in …" | All requests in the file, in order |
| `task: rerun` | Repeat the last run |
| Keyboard shortcut (see [below](#keyboard-shortcuts)) | The request nearest the cursor |

From the terminal:

```sh
zed-http run api.http --line 7
zed-http run api.http --name login
zed-http run api.http --all --env prod
```

## .http File Syntax

### Request Structure

```http
### Optional title
# @name requestName
METHOD URL [HTTP/version]
Header: value
Header: value

request body
```

- Requests are separated by a line starting with `###`. Text after `###` is the request title.
- The method is optional: a line `https://example.com` means `GET`.
- Supported methods: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`, `TRACE`, `CONNECT`.
- If the URL has no scheme, `http://` is assumed.
- A version (`HTTP/1.1`, `HTTP/2`) at the end of the line is allowed and ignored.
- Headers follow the request line directly; the body comes after an empty line.

### Multi-line URL

Continuation lines are indented and joined without spaces:

```http
GET https://api.example.com/search
    ?q=zed
    &limit=10
    &sort=desc
```

### Comments

```http
# comment
// also a comment
```

Comments between headers are skipped.

### Directives

| Directive | Meaning |
|---|---|
| `# @name login` | Request name: for `--name` and response references |
| `# @no-redirect` | Do not follow 3xx redirects |

## Variables

Variables are substituted via `{{name}}` in the URL, headers, body, and file paths.
Spaces inside the braces are allowed: `{{ host }}`. Variable values can themselves contain variables.

### File Variables

```http
@host = api.example.com
@baseUrl = https://{{host}}/v1

GET {{baseUrl}}/users
```

File variables apply to the whole file and **override** environment values.

### Dynamic Variables

| Variable | Result |
|---|---|
| `{{$uuid}}`, `{{$random.uuid}}` | UUID v4 |
| `{{$timestamp}}` | Unix time in seconds |
| `{{$isoTimestamp}}` | UTC time in ISO 8601, e.g. `2026-10-16T11:01:53.123Z` |
| `{{$randomInt}}` | Integer 0–999 |
| `{{$randomInt 1 100}}`, `{{$random.integer 1 100}}` | Integer in the range `[1, 100)` |
| `{{$random.float}}` | Float between 0 and 1 |
| `{{$random.alphabetic 8}}` | 8 random letters |
| `{{$random.alphanumeric 12}}` | 12 random letters and digits |
| `{{$random.hexadecimal 16}}` | 16 hex characters |
| `{{$processEnv.TOKEN}}`, `{{$env.TOKEN}}` | OS environment variable `TOKEN` |

Each substitution generates a new value.

### Lookup Order

1. Dynamic variables (`$...`).
2. File variables (`@name = ...`).
3. The selected environment (private file over public).
4. The `$shared` block.
5. References to stored responses (`name.response...`).

An unknown variable is an error; the request is not sent.

## Environments

Files are searched in the `.http` file's directory (and its `.zed/http/`), then up the tree until the first match.

**`http-client.env.json`** is committed to the repository:

```json
{
  "$shared": {
    "apiVersion": "v1"
  },
  "dev": {
    "baseUrl": "http://localhost:8080"
  },
  "prod": {
    "baseUrl": "https://api.example.com"
  }
}
```

**`http-client.private.env.json`** holds secrets; add it to `.gitignore`:

```json
{
  "dev":  { "token": "dev-secret" },
  "prod": { "token": "prod-secret" }
}
```

Values from the private file are merged with the public one and override it. `$shared` is available in all environments.
Numbers and booleans are converted to strings.

### Selecting an Environment

By priority:

1. The `--env prod` flag.
2. The `HTTP_CLIENT_ENV` variable.
3. `dev`, `local`, or `development`, if present.
4. The first environment in the file.

The selected environment is printed on the first line of output. To make Zed tasks use a different environment,
set the variable in settings (`zed: open settings`):

```json
{
  "terminal": {
    "env": { "HTTP_CLIENT_ENV": "prod" }
  }
}
```

Or per project in `.zed/settings.json`.

List environments:

```sh
zed-http envs api.http
```

## Variable Navigation

`cmd+click` (or `F12`, `editor: go to definition`) on any `{{variable}}`:

| Under the cursor | Goes to |
|---|---|
| `{{token}}` declared in the file via `@token = ...` | The `@token` line |
| `{{baseUrl}}` from an environment | The `"baseUrl"` key in `http-client.env.json` / `http-client.private.env.json` |
| `{{login.response.body.token}}` | `# @name login` |
| `{{$uuid}}` and other dynamic variables | Nowhere |

For environment variables, definitions in the **current environment** and in `$shared` are shown first.
If a variable exists in both the public and private files, Zed shows both locations as a list.
If it isn't in the current environment, all environments where it is declared are shown.

A file variable overrides the environment, so when names match, navigation goes to `@var`.

Hovering a variable shows its resolved value (with nested variables substituted) and its source:

```
baseUrl · environment `dev`
https://httpbin.org
```

For response references, the value is taken from `.zed-http/responses.json`; if the request hasn't run yet, the hover says so.
For dynamic variables, a sample value is shown.

The current environment for navigation is determined the same way as for running (`HTTP_CLIENT_ENV`, then `dev`/`local`).
After changing `HTTP_CLIENT_ENV`, restart the server: `editor: restart language server`.

## Request Chaining

Responses of named requests are stored and can be referenced in subsequent requests:

```http
### Login
# @name login
POST {{baseUrl}}/auth/login
Content-Type: application/json

{ "user": "admin", "password": "{{password}}" }

### Profile
GET {{baseUrl}}/me
Authorization: Bearer {{login.response.body.accessToken}}
```

| Expression | Value |
|---|---|
| `{{login.response.body}}` | The whole body |
| `{{login.response.body.data.token}}` | JSON field by path |
| `{{login.response.body.$.data.token}}` | Same, JSONPath style |
| `{{login.response.body.items[0].id}}` | Array element |
| `{{login.response.headers.Location}}` | Header (case-insensitive) |
| `{{login.response.status}}` | Status code |

Responses are stored in `.zed-http/responses.json` next to the `.http` file and **survive restarts**:
run `login` once, and the other requests can then be run individually.
If a reference points to a request that hasn't run yet, the runner reports it.

Requests without `@name` are stored under the key `line-<line number>`.

## Request Body

### Inline Body

Everything after the empty line up to the next `###`. Trailing empty lines are trimmed.

```http
POST {{baseUrl}}/items
Content-Type: application/json

{
  "title": "New task",
  "createdAt": "{{$isoTimestamp}}"
}
```

### Body from File

```http
POST {{baseUrl}}/upload
Content-Type: application/json

< ./payloads/item.json
```

The path is relative to the `.http` file's directory. The file is sent as is (bytes); variables in it are not substituted.

### Form URL-encoded

```http
POST {{baseUrl}}/login
Content-Type: application/x-www-form-urlencoded

user=admin&password={{password}}
```

### Multipart

Lines of the form `< ./file` inside the body are replaced with the file's contents:

```http
POST {{baseUrl}}/upload
Content-Type: multipart/form-data; boundary=WebAppBoundary

--WebAppBoundary
Content-Disposition: form-data; name="meta"
Content-Type: application/json

{ "name": "report" }
--WebAppBoundary
Content-Disposition: form-data; name="file"; filename="report.txt"

< ./report.txt
--WebAppBoundary--
```

Only text files are inlined; for binary uploads, use a whole-body file.

### Authorization

```http
GET {{baseUrl}}/secure
Authorization: Basic {{user}} {{password}}
```

`Basic user password` and `Basic user:password` are automatically Base64-encoded.
An already encoded value is passed through unchanged. `Bearer` and other schemes are sent as is.

## Working with Responses

### Output

- the request line and sent headers;
- HTTP version, status (green 2xx, yellow 3xx, red 4xx/5xx), time, size;
- response headers;
- body: JSON is pretty-printed (`--raw` disables formatting), text is printed as is,
  for binary data only the size and type are shown.

### Auto-save

Every response is written to `.zed-http/<name or line-N>.<ext>` next to the `.http` file.
The extension is chosen by `Content-Type`: `json`, `html`, `xml`, `yaml`, `png`, `jpg`, `txt`, `bin`.
The path in the terminal is clickable (`cmd+click`); the response opens in Zed with highlighting and search.

Add to `.gitignore`:

```gitignore
.zed-http/
http-client.private.env.json
```

### Redirecting to a File

```http
GET {{baseUrl}}/report.csv

>> ./out/report.csv
```

| Syntax | Behavior |
|---|---|
| `>> path` | If the file exists, `report-1.csv`, `report-2.csv`, … is created |
| `>>! path` | The file is overwritten |

Directories are created automatically.

### Exit Code

| Code | Meaning |
|---|---|
| `0` | All requests returned status < 400 |
| `1` | At least one request returned 4xx/5xx or failed |
| `2` | Argument, file, variable, or environment error |

Handy for CI and scripts: `zed-http run smoke.http --all --env staging`.

## CLI zed-http

```
zed-http run <file.http> [--line N | --name NAME | --all] [--env ENV] [--raw] [--timeout SECS]
zed-http list <file.http>
zed-http envs <file.http>
zed-http new [ROOT] [--name NAME] [--method METHOD] [--url URL] [--from FILE:LINE] [--no-open]
zed-http detect <file> [--line N]
zed-http frameworks [ROOT]
zed-http lsp
```

| Command / flag | Description |
|---|---|
| `run` | Execute requests |
| `list` | List requests: line, method, URL, name |
| `envs` | List available environments |
| `new` | Create an endpoint `ROOT/.zed/http/endpoints/<name>.http` from a template (see [New Endpoint](#new-endpoint)) |
| `detect` | Find endpoints in source code: `line<TAB>method<TAB>URL<TAB>language/framework` (exit code 1 if none) |
| `frameworks` | Print the supported languages and frameworks; validates `ROOT/.zed/http/frameworks.json` if present |
| `--from FILE:LINE` | (`new`) take the method and URL from an endpoint in source code |
| `--no-open` | (`new`) don't open the file in Zed |
| `lsp` | Language server over stdio (started by Zed automatically) |
| `-l`, `--line N` | The request containing line `N` (1-based) |
| `-n`, `--name NAME` | Request by `@name` or by the title after `###` |
| `-a`, `--all` | All requests in the file, in order |
| `-e`, `--env ENV` | Environment |
| `--raw` | Don't format JSON |
| `--timeout SECS` | Request timeout, 60 s by default |

## New Endpoint

The **`http: new endpoint`** task (`task: spawn`) prompts in the terminal for the method, URL, and name,
creates `.zed/http/endpoints/<name>.http`, and opens it in Zed (if the `zed` CLI is in `PATH`).

```
.zed/
├── settings.json, tasks.json     committed as usual
└── http/                         generated requests and profiles
    ├── http-client.env.json      created with a dev profile if no environments exist yet
    ├── http-client.private.env.json
    └── endpoints/*.http
```

No `.gitignore` is generated: whether these files are committed is up to you. To keep them local, add to `.gitignore`:

```gitignore
.zed/http/
```

`http-client.env.json` is searched up the directories from the `.http` file,
at each level first in the directory itself, then in its `.zed/http/`. So profiles from `.zed/http/`
are also visible to the project's regular `.http` files.

Extension tasks are only visible when a `.http` file is open. To make the command available everywhere,
add it to `~/.config/zed/tasks.json`:

```json
[
  {
    "label": "http: new endpoint",
    "command": "zed-http",
    "args": ["new", "\"$ZED_WORKTREE_ROOT\""],
    "use_new_terminal": true,
    "reveal": "always",
    "hide": "on_success"
  }
]
```

and, optionally, a keyboard shortcut in the keymap:

```json
{ "bindings": { "cmd-alt-n": ["task::Spawn", { "task_name": "http: new endpoint" }] } }
```

## Generating a Request from Code

Every route or HTTP client call in your code gets a clickable **🌐 Generate .http request** lens above it.
One click appends a full request to `.zed/http/endpoints/<source file>.http`
(`server.ts` → `server.ts.http`, `UserController.java` → `UserController.java.http`) and opens it at that request.
Once a request is generated, its lens turns into **🔗 Open <source file>.http** and jumps to it.
The lens uses colour emoji (🌐 / 🔗), so it keeps its own colour and doesn't look like the ▶ that runs tasks.
The same actions are available from `cmd+.` on the line.

Code lenses are off by default in Zed. Enable them in `settings.json`:

```json
{ "code_lens": "on" }
```

With `"menu"` the lenses appear in the `cmd+.` menu instead; with `"off"` the code actions are still there.
If a language has an explicit `language_servers` list without `"..."`, add `"zed-http"` to it.

A gutter ▶ like the one in `.http` files is not possible here: Zed draws gutter buttons only from a
language's own `runnables.scm`, and an extension can ship that file only for languages it defines itself
(there is no way to add queries to Rust, Dart, TypeScript… from another extension, and no user-level query
overrides). A code lens is the closest equivalent the extension API allows.

### What gets generated

From the handler signature and the model classes/structs **in the same file**:

| | Sources |
|---|---|
| Path variables | `{id}`, `:id`, `<id>`, `$id`… → `{{id}}`, declared as `@id = 1` at the top of the file |
| Query parameters | `@RequestParam`, `@Query`, FastAPI scalar params, `req.query.x`, `r.URL.Query().Get`, gin/echo `c.Query`, `request.args`, `$request->query`, dio `queryParameters`, axum `Query<T>` |
| Headers | `@RequestHeader`, `@Header`/`@Headers`, FastAPI `Header()`, `req.header`, `r.Header.Get`, `request.headers.get`, `$request->header`, dio `headers` |
| Auth | `Authorization`, `Bearer`, `@PreAuthorize`, `@Secured`, `@UseGuards`, JWT, auth `Depends` → `Authorization: Bearer {{token}}` |
| JSON body | `@RequestBody T`, `@Body() T`, FastAPI models, `Json<T>`, Go `Decode(&v)`/`ShouldBindJSON`, Dart `x.toJson()`, `req.body` fields/destructuring, dio `data`/`jsonEncode` maps, Laravel `validate([...])` |

Model fields are resolved for Java/Kotlin classes and records, Dart classes, Python (pydantic) classes,
TypeScript interfaces, Go structs (`json` tags respected) and Rust structs, including one level of nesting.
Values are type-based placeholders: `""`, `0`, `0.0`, `false`, `[]`, nested objects.

Example — the Spring `create` handler in [`examples/code/java/SpringUserController.java`](examples/code/java/SpringUserController.java) adds to `SpringUserController.java.http`:

```http
### post-api-users
# @name post-api-users
# Requires `token` in .zed/http/http-client.env.json
@xRequestId = value
POST {{baseUrl}}/api/users
Accept: application/json
Authorization: Bearer {{token}}
X-Request-Id: {{xRequestId}}
Content-Type: application/json

{
  "name": "",
  "email": "",
  "age": 0,
  "roles": [],
  "address": {
    "city": "",
    "zip": ""
  }
}
```

The `zed-http new --from FILE:LINE` command uses the same inference and the same `<source file>.http`.
Without `--from`, `zed-http new` creates a separate `<name>.http` per endpoint.

### Supported frameworks

See the table at the top: [Supported Languages and Frameworks](#supported-languages-and-frameworks).
A framework is active in a file only when the file contains one of its `imports`
(e.g. `package:dio/`, `org.springframework`), so frameworks of the same language don't interfere.
Path parameters (`{id}`, `:id`, `<id>`, `<int:id>`, `$id`, `${user.id}`) become `{{id}}`,
and relative paths get the `{{baseUrl}}` prefix. Bare `https://…` literals get a `cmd+.` action but no lens.

Limits: matching is per pattern, not a full parse — a path assembled from variables elsewhere
won't be found, and models defined in other files are not resolved (the body is then `{}`).

### Pattern language

Patterns are source code with typed placeholders — no regular expressions:

```json
"spring": {
  "name": "Spring Web",
  "imports": ["org.springframework"],
  "prefix": ["@RequestMapping(\"{path}\""],
  "routes": ["@{method}Mapping(\"{path}\"", "@{method}Mapping"],
  "query": ["@RequestParam {type} {name}"],
  "headers": ["@RequestHeader(\"{name}\""],
  "body": ["@RequestBody {type} {_}"],
  "auth": ["@PreAuthorize"],
  "examples": [{ "code": "@GetMapping(\"/users/{id}\")", "expect": ["GET {{baseUrl}}/users/{{id}}"] }]
}
```

| Token | Matches |
|---|---|
| `{method}` | an HTTP method word in any case: `get`, `Post`, `DELETE` |
| `{path}` / `{route}` | string contents / string contents starting with `/` or `http(s)://` |
| `{host}` | a host name (for `Uri.https('host', '/path')`-style calls, with `"url": "https://{host}{path}"`) |
| `{name}` | an identifier or header name |
| `{type}` | a type: `int`, `List<String>`, `list[str]`, `*Order`, `Dto?` — models are expanded into fields |
| `{var}` | a variable; its declared type is looked up (`Decode(&{var})`, `{var}.toJson()`) |
| `{handler}` | a function name; its body is searched too (`post({handler})`) |
| `{keys}` | a `{…}` / `[…]` literal; its keys or destructured names are taken |
| `{generic}` | an optional `<…>` type argument list |
| `{_}` | any identifier, ignored |
| `{x?}` | any placeholder made optional |
| `...` | anything on the same line except parentheses |
| `"` | any quote: `"`, `'` or `` ` `` |
| whitespace | optional whitespace (required between two words); also implied between tokens outside strings |
| `^` at the start | the beginning of a line |
| `{{` / `}}` | literal braces |

In every list the first pattern that matches at a position wins, so put specific patterns first.
Each framework has `examples`; the test suite checks that every example produces exactly its `expect` list.
The full field reference is in [`frameworks.schema.json`](frameworks.schema.json) — Zed's JSON language server
uses it for completion and validation while you edit.

### Adding a framework or language

- **For one project** — create `.zed/http/frameworks.json`. It is merged into the built-in catalog
  (languages → frameworks → fields), and the language server picks up changes on save:

  ```json
  {
    "$schema": "https://raw.githubusercontent.com/farum-azula/zed_editor_extensions/main/zed-editor-http-client-extension/frameworks.schema.json",
    "languages": {
      "dart": {
        "frameworks": {
          "my-api-client": {
            "name": "Our ApiClient",
            "imports": ["package:my_app/api_client.dart"],
            "routes": ["api.call(\"{method}\", \"{route}\""],
            "body": ["api.call(...data: {keys}"]
          }
        }
      }
    }
  }
  ```

  `zed-http frameworks` in the project root validates the merged catalog and prints it.

- **Built in** — add it to [`frameworks.json`](frameworks.json) with `examples`, run `cargo test` in `cli/`,
  reinstall the CLI. A new language also needs its Zed name in `extension.toml` → `languages`
  (a test checks this).

### Try it

[`examples/code/`](examples/code) has one file per framework — 48 files, one folder per language.
Open any of them in Zed, put the cursor on a route and press `cmd+.` (or click the lens). Or from the terminal:

```sh
$ zed-http detect examples/code/kotlin/KtorServerRoutes.kt
10	GET	{{baseUrl}}/users/{{id}}	kotlin/ktor-server
14	GET	{{baseUrl}}/users	kotlin/ktor-server
18	POST	{{baseUrl}}/users	kotlin/ktor-server
22	DELETE	{{baseUrl}}/users/{{id}}	kotlin/ktor-server
```

A test asserts that every framework in the catalog is covered by at least one of these files.

### For AI Agents

The non-interactive CLI suits agents: without a terminal, no prompts are shown and defaults are used.

```sh
zed-http detect lib/api/auth_api.dart                     # 12	POST	{{baseUrl}}/auth/login	dart/dio
zed-http new . --from lib/api/auth_api.dart:12 --no-open  # creates .zed/http/endpoints/post-auth-login.http
zed-http new . -m GET -u '{{baseUrl}}/users/{{id}}' -n get-user --no-open
```

To let the Zed agent know about this, add to the project's `.rules` (or `AGENTS.md` / `CLAUDE.md`):

```md
HTTP requests to the project's API are stored in `.zed/http/endpoints/*.http` (IntelliJ HTTP Client format).
For a new endpoint: `zed-http new . --from <file>:<line> --no-open`, then fill in the body/headers.
Find endpoints in a file: `zed-http detect <file>`. Run: `zed-http run <file.http> --name <name> --env dev`.
Profiles: `.zed/http/http-client.env.json`.
```

## Keyboard Shortcuts

Run `zed: open keymap` and add:

```json
[
  {
    "context": "Editor && extension == http",
    "bindings": {
      "cmd-enter": ["editor::SpawnNearestTask", { "reveal": "no_focus" }]
    }
  }
]
```

`cmd-enter` runs the request under the cursor; focus stays in the editor.

## Project Structure

```
.
├── frameworks.json             supported languages → frameworks → patterns
├── frameworks.schema.json      JSON Schema for frameworks.json
├── extension.toml              extension manifest, grammar, language server
├── Cargo.toml                  WASM part of the extension
├── src/lib.rs                  launches `zed-http lsp` from Zed
├── languages/http/
│   ├── config.toml             file extensions, comments, brackets
│   ├── highlights.scm          highlighting
│   ├── injections.scm          JSON / XML / GraphQL in the body
│   ├── outline.scm             request list in the outline
│   ├── runnables.scm           ▶ buttons
│   └── tasks.json              run tasks
├── cli/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             arguments, request execution, output, new/detect
│       ├── catalog.rs          loads frameworks.json, finds routes, infers requests
│       ├── pattern.rs          the pattern language of frameworks.json
│       ├── endpoint.rs         .http request rendering and files
│       ├── infer.rs            request shape (query, headers, body) and sample values
│       ├── lsp.rs              language server: definition, hover, code actions, code lenses
│       ├── parser.rs           .http file parsing
│       └── vars.rs             variables, environments, response references
└── examples/
    ├── requests.http
    ├── http-client.env.json
    └── code/                   one file per framework, one folder per language
        ├── java/        Spring, JAX-RS (Quarkus), Micronaut, Retrofit, Feign
        ├── kotlin/      Spring, Ktor server/client, Retrofit, JAX-RS
        ├── dart/        dio, package:http, retrofit, chopper, shelf_router
        ├── typescript/  Express, Fastify, Hono, Koa, NestJS, Axios, Angular, fetch
        ├── python/      FastAPI, Flask, Django, requests/httpx
        ├── go/          net/http, gorilla/mux, Gin, Echo, chi, Fiber
        ├── rust/        axum, actix-web, Rocket, reqwest
        ├── php/         Laravel, Symfony, Slim, Guzzle
        ├── ruby/        Rails, Sinatra, Faraday/HTTParty
        └── csharp/      ASP.NET controllers, minimal APIs, Refit, HttpClient
```

Grammar: [rest-nvim/tree-sitter-http](https://github.com/rest-nvim/tree-sitter-http); the commit is pinned in `extension.toml`.

## Development

```sh
cd cli
cargo test
cargo run -- list ../examples/requests.http
cargo run -- run ../examples/requests.http --all
```

Check the WASM build:

```sh
cargo build --release --target wasm32-wasip2
```

After changing `src/lib.rs` or files in `languages/http/`, click **Rebuild** on the dev extension in `zed: extensions`.
Extension load logs: `zed: open log`.

To update the grammar, change `commit` in `extension.toml` and verify that the nodes
used in the `.scm` files (`request`, `method`, `target_url`, `header`, `variable`, …) haven't been renamed.

## Limitations

- JavaScript handlers (`> {% ... %}`, `> ./handler.js`) and pre-request scripts are skipped.
  To pass data between requests, use `{{name.response...}}`.
- No UI environment switcher; only `HTTP_CLIENT_ENV` or `--env`.
- The response is shown in the terminal, not in a separate panel (a Zed extension API limitation).
- No support for WebSocket, gRPC, GraphQL subscriptions, or `HTTP/2`-specific features; send GraphQL queries as `POST` with JSON.
- No cookie jar between requests; pass `Cookie` manually or via `{{name.response.headers.Set-Cookie}}`.
- `$processEnv` values come from the Zed terminal's environment.

## Troubleshooting

**No ▶ buttons.**
Check that the file language in the bottom right is **HTTP**. If not, reinstall the extension and check `zed: open log`.
The buttons also require `"gutter": { "runnables": true }` to be enabled (it is by default).

**▶ is there, but the "HTTP: …" task isn't offered.**
Copy `languages/http/tasks.json` to the project's `.zed/tasks.json` (or the global `~/.config/zed/tasks.json`).

**`cmd+click` on a variable does nothing.**
Check that a recent `zed-http` is in `PATH` (`zed-http lsp` shouldn't complain about an unknown command),
then run `editor: restart language server`. Server errors are visible in `dev: open language server logs`.
Without `zed-http` in `PATH`, the extension can't start the server: highlighting works, navigation doesn't.

**`zed-http: command not found`.**
`~/.cargo/bin` isn't in the `PATH` of the shell Zed launches. Add it to `~/.zshrc` and restart Zed,
or specify the full path in `tasks.json`: `"command": "/Users/<you>/.cargo/bin/zed-http"`.

**`unknown variable {{baseUrl}}`.**
`http-client.env.json` wasn't found, or the selected environment lacks the variable. Check `zed-http envs file.http`
and the `environment: ...` line at the start of the output.

**`no stored response for request 'login'`.**
Run the request with `# @name login` first.

**TLS errors for local servers with a self-signed certificate.**
System root certificates are used; add your CA to the system store.
