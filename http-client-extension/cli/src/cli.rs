//! Command-line entry points: argument parsing and the `run` / `list` / `envs` / `new` /
//! `detect` / `frameworks` commands.

use crate::parser::{self, Body, HttpFile, Request};
use crate::vars::{Context, Environments};
use crate::{catalog, endpoint, lsp, vars};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

pub const USAGE: &str = "\
zed-http — IntelliJ-style .http file runner

USAGE:
    zed-http run <file.http> [--line N | --name NAME | --all] [--env ENV] [--raw] [--timeout SECS]
    zed-http list <file.http>
    zed-http envs <file.http>
    zed-http new [ROOT] [--name NAME] [--method METHOD] [--url URL] [--from FILE:LINE] [--no-open]
    zed-http detect <file> [--line N]
    zed-http frameworks [ROOT]
    zed-http lsp

Without --env, the environment is the one the file names with `# @env NAME`, then the
HTTP_CLIENT_ENV variable, then dev / local / development, then the first one defined.
Responses are saved to `.zed-http/` next to the .http file.";

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const CYAN: &str = "\x1b[36m";

#[derive(Default)]
/// The options of `run`, `list` and `envs`.
struct RunOptions {
    command: String,
    file: PathBuf,
    line: Option<usize>,
    name: Option<String>,
    all: bool,
    env: Option<String>,
    raw: bool,
    timeout: u64,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<RunOptions, String> {
    let mut args = args.peekable();
    let mut options = RunOptions { timeout: 60, ..Default::default() };
    options.command = args.next().ok_or(USAGE)?;
    if matches!(options.command.as_str(), "-h" | "--help" | "help") {
        return Err(USAGE.into());
    }
    while let Some(a) = args.next() {
        let mut val = |n: &str| args.next().ok_or_else(|| format!("{n} requires a value"));
        match a.as_str() {
            "--line" | "-l" => options.line = Some(val("--line")?.parse().map_err(|_| "invalid --line")?),
            "--name" | "-n" => options.name = Some(val("--name")?),
            "--env" | "-e" => options.env = Some(val("--env")?),
            "--timeout" => options.timeout = val("--timeout")?.parse().map_err(|_| "invalid --timeout")?,
            "--all" | "-a" => options.all = true,
            "--raw" => options.raw = true,
            s if s.starts_with('-') => return Err(format!("unknown option {s}\n\n{USAGE}")),
            s => options.file = PathBuf::from(s),
        }
    }
    if options.file.as_os_str().is_empty() {
        return Err(format!("missing <file.http>\n\n{USAGE}"));
    }
    Ok(options)
}

/// Runs one command and converts its exit code. `args` are the arguments after the program name.
pub fn run_main(args: Vec<String>) -> ExitCode {
    ExitCode::from(run_args(args))
}

/// A command with arguments of its own (`run`, `list` and `envs` share theirs).
struct Subcommand {
    name: &'static str,
    /// Runs it with the arguments after its name; returns the exit code.
    run: fn(arguments: Vec<String>) -> u8,
}

/// Commands besides `run` / `list` / `envs`; adding one is one more entry.
const SUBCOMMANDS: [Subcommand; 4] = [
    Subcommand {
        name: "lsp",
        run: |_| {
            lsp::serve();
            0
        },
    },
    Subcommand { name: "new", run: |arguments| exit_code(new_endpoint(arguments.into_iter()).map(|()| true)) },
    Subcommand {
        name: "frameworks",
        run: |arguments| exit_code(frameworks(arguments.into_iter().next()).map(|()| true)),
    },
    Subcommand { name: "detect", run: |arguments| exit_code(detect(arguments.into_iter())) },
];

/// Runs one command, returning the process exit code: 0 ok, 1 request failed, 2 usage/IO error.
pub fn run_args(args: Vec<String>) -> u8 {
    let subcommand = args.first().and_then(|name| SUBCOMMANDS.iter().find(|subcommand| subcommand.name == name));
    if let Some(subcommand) = subcommand {
        return (subcommand.run)(args.into_iter().skip(1).collect());
    }
    let options = match parse_args(args.into_iter()) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    exit_code(run(options))
}

/// 0 when it went well, 1 when a request failed or nothing was found, 2 on an error (reported).
fn exit_code(result: Result<bool, String>) -> u8 {
    match result {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            eprintln!("{RED}error:{RESET} {e}");
            2
        }
    }
}

fn run(options: RunOptions) -> Result<bool, String> {
    let src = fs::read_to_string(&options.file).map_err(|e| format!("{}: {e}", options.file.display()))?;
    let file = parser::parse(&src);
    let dir = options.file.parent().map(Path::to_path_buf).unwrap_or_default();
    let dir = if dir.as_os_str().is_empty() { PathBuf::from(".") } else { dir };
    let envs = Environments::load(&dir)?;

    match options.command.as_str() {
        "envs" => {
            for n in &envs.names {
                println!("{n}");
            }
            Ok(true)
        }
        "list" => {
            for r in &file.requests {
                let label = r.name.as_ref().or(r.title.as_ref()).map(|s| format!("  {DIM}({s}){RESET}"));
                println!("{:>4}: {BOLD}{}{RESET} {}{}", r.start_line, r.method, r.url, label.unwrap_or_default());
            }
            Ok(true)
        }
        "run" => {
            let selected = select_requests(&file, &options)?;
            // Each request in its own environment: --env, then its `# @env`, then the file's.
            // All are checked before anything is sent.
            let mut chosen = Vec::new();
            for req in &selected {
                let own = options.env.clone().or_else(|| req.env.as_ref().map(|directive| directive.name.clone()));
                chosen.push(envs.select(vars::requested_env(own, &src).as_deref())?);
            }
            let store_path = dir.join(".zed-http").join("responses.json");
            let mut context = Context { responses: load_store(&store_path), ..Default::default() };
            let stored = context.responses.clone();

            let mut ok = true;
            let mut shown = None;
            for (i, (req, env)) in selected.iter().zip(&chosen).enumerate() {
                if i > 0 {
                    println!("\n{DIM}{}{RESET}\n", "─".repeat(60));
                }
                if shown != Some(env) {
                    if let Some(e) = env {
                        println!("{DIM}environment: {e}{RESET}");
                    }
                    shown = Some(env);
                }
                context.vars = envs.vars(env.as_deref());
                context.vars.extend(file.variable_pairs());
                match execute(req, &mut context, &dir, &options) {
                    Ok(success) => ok &= success,
                    Err(e) => {
                        eprintln!("{RED}error:{RESET} {e}");
                        ok = false;
                    }
                }
            }
            context.responses.retain(|k, _| !context.unlogged.contains(k));
            // Nothing new to keep (only `# @no-log` requests, say): nothing is written.
            if context.responses != stored {
                save_store(&store_path, &context.responses);
            }
            Ok(ok)
        }
        other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    }
}

fn select_requests<'a>(file: &'a HttpFile, options: &RunOptions) -> Result<Vec<&'a Request>, String> {
    if file.requests.is_empty() {
        return Err("no requests found in file".into());
    }
    if options.all {
        return Ok(file.requests.iter().collect());
    }
    if let Some(n) = &options.name {
        return file
            .requests
            .iter()
            .find(|r| r.name.as_deref() == Some(n) || r.title.as_deref() == Some(n))
            .map(|r| vec![r])
            .ok_or_else(|| format!("request `{n}` not found"));
    }
    let line = options.line.ok_or("specify --line, --name or --all")?;
    file.requests
        .iter()
        .find(|r| line >= r.start_line && line <= r.end_line)
        .or_else(|| file.requests.iter().rev().find(|r| r.start_line <= line))
        .map(|r| vec![r])
        .ok_or_else(|| format!("no request at line {line}"))
}

fn execute(req: &Request, context: &mut Context, dir: &Path, options: &RunOptions) -> Result<bool, String> {
    let url = context.resolve(&req.url)?;
    let url = if url.contains("://") { url } else { format!("http://{url}") };
    let mut headers = Vec::new();
    for (k, v) in &req.headers {
        let mut v = context.resolve(v)?;
        if k.eq_ignore_ascii_case("authorization") {
            v = expand_basic_auth(&v);
        }
        headers.push((context.resolve(k)?, v));
    }
    let body: Option<Vec<u8>> = match &req.body {
        None => None,
        Some(Body::Inline(b)) => Some(resolve_body(context, b, dir)?.into_bytes()),
        Some(Body::File(p)) => {
            let p = context.resolve(p)?;
            let path = dir.join(&p);
            Some(fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?)
        }
    };

    println!("{BOLD}{CYAN}{}{RESET} {BOLD}{url}{RESET}", req.method);
    for (k, v) in &headers {
        println!("{DIM}{k}: {v}{RESET}");
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(options.timeout))
        .redirects(if req.no_redirect { 0 } else { 10 })
        .build();
    let mut call = agent.request(&req.method, &url);
    for (k, v) in &headers {
        call = call.set(k, v);
    }

    let started = Instant::now();
    let result = match &body {
        Some(b) => call.send_bytes(b),
        None => call.call(),
    };
    let resp = match result {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(ureq::Error::Transport(t)) => return Err(format!("request failed: {t}")),
    };

    let status = resp.status();
    let status_text = resp.status_text().to_string();
    let version = resp.http_version().to_string();
    let mut resp_headers = serde_json::Map::new();
    let mut header_lines = Vec::new();
    for name in resp.headers_names() {
        for v in resp.all(&name) {
            header_lines.push(format!("{name}: {v}"));
            resp_headers.insert(name.clone(), Value::String(v.to_string()));
        }
    }
    let content_type = resp.content_type().to_string();
    let mut bytes = Vec::new();
    resp.into_reader()
        .take(256 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("failed to read response: {e}"))?;
    let elapsed = started.elapsed();

    let color = match status {
        200..=299 => GREEN,
        300..=399 => YELLOW,
        _ => RED,
    };
    println!(
        "\n{BOLD}{version} {color}{status} {status_text}{RESET}  {DIM}{} ms · {}{RESET}",
        elapsed.as_millis(),
        human_size(bytes.len())
    );
    for h in &header_lines {
        println!("{DIM}{h}{RESET}");
    }
    println!();

    let text = String::from_utf8_lossy(&bytes);
    let json: Option<Value> = serde_json::from_slice(&bytes).ok();
    let pretty = match (&json, options.raw) {
        (Some(j), false) => serde_json::to_string_pretty(j).expect("a parsed JSON value re-serializes"),
        _ => text.to_string(),
    };
    if is_textual(&content_type) || json.is_some() {
        println!("{pretty}");
    } else {
        println!("{DIM}<{} of binary data ({content_type})>{RESET}", human_size(bytes.len()));
    }

    let key = req.name.clone().unwrap_or_else(|| format!("line-{}", req.start_line));
    context.responses.insert(
        key.clone(),
        serde_json::json!({
            "status": status,
            "headers": resp_headers,
            "body": json.clone().unwrap_or_else(|| Value::String(text.to_string())),
        }),
    );
    let ext = extension_for(&content_type, json.is_some());
    let out_dir = dir.join(".zed-http");
    let saved = out_dir.join(format!("{}.{ext}", sanitize(&key)));
    if req.no_log {
        // Usable by the requests after it in this run, but not written anywhere.
        context.unlogged.insert(key);
    } else {
        let data = if json.is_some() { pretty.into_bytes() } else { bytes.clone() };
        match fs::create_dir_all(&out_dir).and_then(|()| fs::write(&saved, data)) {
            Ok(()) => println!("\n{DIM}saved: {}{RESET}", saved.display()),
            // The request went through either way: a response that cannot be kept is only reported.
            Err(error) => eprintln!("{YELLOW}not saved:{RESET} {}: {error}", saved.display()),
        }
    }

    if let Some(redirect) = &req.redirect {
        let target = dir.join(context.resolve(&redirect.path)?);
        let target = if !redirect.overwrite && target.exists() { unique_path(&target) } else { target };
        if let Some(p) = target.parent() {
            let _ = fs::create_dir_all(p);
        }
        fs::write(&target, &bytes).map_err(|e| format!("{}: {e}", target.display()))?;
        println!("{DIM}written to: {}{RESET}", target.display());
    }

    Ok(status < 400)
}

fn resolve_body(context: &Context, body: &str, dir: &Path) -> Result<String, String> {
    let mut out = Vec::new();
    for line in body.lines() {
        if let Some(p) = line.trim().strip_prefix("< ") {
            let path = dir.join(context.resolve(p.trim())?);
            out.push(fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?);
        } else {
            out.push(context.resolve(line)?);
        }
    }
    Ok(out.join("\n"))
}

/// Encodes `Basic user password` / `Basic user:password`; other values are passed through.
pub fn expand_basic_auth(v: &str) -> String {
    use base64::Engine;
    if let Some(rest) = v.strip_prefix("Basic ") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() == 2 || (parts.len() == 1 && parts[0].contains(':') && !rest.ends_with('=')) {
            let creds = if parts.len() == 2 { format!("{}:{}", parts[0], parts[1]) } else { parts[0].to_string() };
            return format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(creds));
        }
    }
    v.to_string()
}

/// Content types printed as text, besides `text/*` and an unknown one.
const TEXTUAL: [&str; 7] = ["json", "xml", "javascript", "html", "yaml", "x-www-form-urlencoded", "graphql"];

/// Whether a response of this content type should be printed as text.
pub fn is_textual(ct: &str) -> bool {
    ct.starts_with("text/") || TEXTUAL.iter().any(|t| ct.contains(t)) || ct.is_empty()
}

/// A saved response's file extension, by a word of its content type.
struct SavedExtension {
    content_type: &'static str,
    extension: &'static str,
}

/// Tried in order, after JSON; other text is `txt`, anything else `bin`.
const SAVED_EXTENSIONS: [SavedExtension; 5] = [
    SavedExtension { content_type: "html", extension: "html" },
    SavedExtension { content_type: "xml", extension: "xml" },
    SavedExtension { content_type: "yaml", extension: "yaml" },
    SavedExtension { content_type: "png", extension: "png" },
    SavedExtension { content_type: "jpeg", extension: "jpg" },
];

/// File extension for a saved response.
pub fn extension_for(ct: &str, is_json: bool) -> &'static str {
    if is_json {
        return "json";
    }
    match SAVED_EXTENSIONS.iter().find(|saved| ct.contains(saved.content_type)) {
        Some(saved) => saved.extension,
        None if is_textual(ct) => "txt",
        None => "bin",
    }
}

/// Makes a request name safe to use as a file name.
pub fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// `report.json` → `report-1.json` while the path is taken.
pub fn unique_path(p: &Path) -> PathBuf {
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("response");
    let ext = p.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    let mut counter = 1;
    loop {
        let candidate = p.with_file_name(format!("{stem}-{counter}{ext}"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

/// Formats a byte count as B / KB / MB.
pub fn human_size(n: usize) -> String {
    match n {
        n if n < 1024 => format!("{n} B"),
        n if n < 1024 * 1024 => format!("{:.1} KB", n as f64 / 1024.0),
        n => format!("{:.1} MB", n as f64 / 1024.0 / 1024.0),
    }
}

/// Reads the stored responses; a missing or unparsable file yields an empty store.
pub fn load_store(path: &Path) -> serde_json::Map<String, Value> {
    fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Writes the stored responses, creating the directory. A failure is reported, not fatal: the
/// requests have run either way, only later references to their responses miss them.
pub fn save_store(path: &Path, store: &serde_json::Map<String, Value>) {
    let text = serde_json::to_string_pretty(store).unwrap_or_default();
    let written = fs::create_dir_all(path.parent().unwrap_or(Path::new(""))).and_then(|()| fs::write(path, text));
    if let Err(error) = written {
        eprintln!("{YELLOW}responses not kept:{RESET} {}: {error}", path.display());
    }
}

/// `zed-http new`: creates `<root>/.zed/http/endpoints/<name>.http` from a template.
/// Whether `.zed/http/` is committed is up to the user (no `.gitignore` is generated).
fn new_endpoint(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let (mut root, mut name, mut method, mut url, mut open) = (None, None, None, None, true);
    let mut generated: Option<String> = None;
    let mut source: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        let mut val = |n: &str| args.next().ok_or_else(|| format!("{n} requires a value"));
        match a.as_str() {
            "--name" | "-n" => name = Some(val("--name")?),
            "--method" | "-m" => method = Some(val("--method")?),
            "--url" | "-u" => url = Some(val("--url")?),
            "--no-open" => open = false,
            "--from" => {
                let from = val("--from")?;
                let (file, line) = from.rsplit_once(':').ok_or("--from expects FILE:LINE")?;
                let line: usize = line.parse().map_err(|_| "invalid line in --from")?;
                let src = fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
                let catalog = catalog::for_source(Path::new(file))?;
                let request = endpoint::generate(&catalog, &src, Path::new(file), line.saturating_sub(1))
                    .ok_or_else(|| format!("no HTTP endpoint found at {from}"))?;
                method.get_or_insert(request.endpoint.method);
                url.get_or_insert(request.endpoint.url);
                generated = Some(request.content);
                source = Some(PathBuf::from(file));
            }
            s if s.starts_with('-') => return Err(format!("unknown option {s}\n\n{USAGE}")),
            s => root = Some(PathBuf::from(s)),
        }
    }
    let root = match root.filter(|p| !p.as_os_str().is_empty()) {
        Some(r) => r,
        None => std::env::current_dir().unwrap_or_default(),
    };
    let method = match method {
        Some(m) => m,
        None => prompt("Method", "GET")?,
    }
    .to_uppercase();
    let url = match url {
        Some(u) => u,
        None => prompt("URL", "{{baseUrl}}/")?,
    };
    let default_name = endpoint::default_name(&method, &url);
    let name = match name {
        Some(n) => n,
        None => prompt("Name", &default_name)?,
    };
    let name = Some(endpoint::slug(&name)).filter(|s| !s.is_empty()).unwrap_or_else(|| "endpoint".into());

    let base = root.join(vars::ZED_HTTP_DIR);
    let dir = base.join("endpoints");
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let had_env = vars::find_env_dir(&dir).is_some();
    let env_source = source.clone().unwrap_or_else(|| root.clone());
    if let Some(env) = endpoint::ensure_env(&base, &env_source).map_err(|e| format!("{}: {e}", base.display()))? {
        println!("{GREEN}{}:{RESET} {}", if had_env { "updated" } else { "created" }, env.display());
    }

    let text = match generated {
        // Rendered with the default name; swap in the chosen one.
        Some(t) => {
            let default = endpoint::default_name(&method, &url);
            t.replace(&format!("### {default}\n# @name {default}"), &format!("### {name}\n# @name {name}"))
        }
        None => endpoint::template(&method, &url, &name),
    };
    // Requests generated from a source file are collected in `<source file>.http`.
    let path = match &source {
        Some(src) => endpoint::file_for_source(&dir, src),
        None => endpoint::free_path(&dir, &name),
    };
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if endpoint::find_request(&existing, &name).is_some() {
        println!("{YELLOW}exists:{RESET} {name} in {}", path.display());
    } else {
        let appended =
            fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| {
                std::io::Write::write_all(&mut f, endpoint::append_text(&existing, &text).as_bytes())
            });
        appended.map_err(|e| format!("{}: {e}", path.display()))?;
    }
    println!("{GREEN}saved:{RESET} {}", path.display());

    // Open in Zed if its CLI is available; otherwise the printed path is cmd+clickable.
    if open {
        let _ = std::process::Command::new("zed").arg(&path).status();
    }
    Ok(())
}

fn prompt(label: &str, default: &str) -> Result<String, String> {
    use std::io::IsTerminal;
    let tty = std::io::stdin().is_terminal();
    prompt_from(label, default, tty, &mut std::io::stdin().lock(), &mut std::io::stdout())
}

/// Asks for a value; without a terminal (CI, an editor task, an agent) the default is used.
pub fn prompt_from(
    label: &str,
    default: &str,
    is_terminal: bool,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
) -> Result<String, String> {
    if !is_terminal {
        return Ok(default.into());
    }
    write!(out, "{BOLD}{label}{RESET} {DIM}[{default}]{RESET}: ").map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    input.read_line(&mut line).map_err(|e| e.to_string())?;
    let line = line.trim();
    Ok(if line.is_empty() { default.into() } else { line.into() })
}

/// `zed-http detect`: prints `LINE<TAB>METHOD<TAB>URL<TAB>language/framework` for routes in a source file.
fn detect(mut args: impl Iterator<Item = String>) -> Result<bool, String> {
    let (mut file, mut only) = (None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--line" | "-l" => only = Some(args.next().and_then(|v| v.parse::<usize>().ok()).ok_or("invalid --line")?),
            s if s.starts_with('-') => return Err(format!("unknown option {s}\n\n{USAGE}")),
            s => file = Some(s.to_string()),
        }
    }
    let file = file.ok_or_else(|| format!("missing <file>\n\n{USAGE}"))?;
    let src = fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?;
    let catalog = catalog::for_source(Path::new(&file))?;
    let mut found = false;
    for r in catalog.routes(&src, Path::new(&file)) {
        if only.is_some_and(|l| l != r.line + 1) {
            continue;
        }
        println!("{}\t{}\t{}\t{}", r.line + 1, r.endpoint.method, r.endpoint.url, r.framework);
        found = true;
    }
    Ok(found)
}

/// `zed-http frameworks`: prints the supported languages and frameworks (validating the project catalog).
fn frameworks(root: Option<String>) -> Result<(), String> {
    let root = root.map(PathBuf::from).unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let custom = catalog::project_file(&root);
    let catalog = if custom.exists() {
        println!("{DIM}built-in catalog + {}{RESET}\n", custom.display());
        catalog::Catalog::for_root(&root)?
    } else {
        catalog::Catalog::from_sources(&[catalog::BUILTIN])?
    };
    print!("{}", catalog.tree());
    Ok(())
}
