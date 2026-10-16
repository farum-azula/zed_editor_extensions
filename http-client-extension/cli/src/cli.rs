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
struct Opts {
    command: String,
    file: PathBuf,
    line: Option<usize>,
    name: Option<String>,
    all: bool,
    env: Option<String>,
    raw: bool,
    timeout: u64,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Opts, String> {
    let mut args = args.peekable();
    let mut o = Opts { timeout: 60, ..Default::default() };
    o.command = args.next().ok_or(USAGE)?;
    if matches!(o.command.as_str(), "-h" | "--help" | "help") {
        return Err(USAGE.into());
    }
    while let Some(a) = args.next() {
        let mut val = |n: &str| args.next().ok_or_else(|| format!("{n} requires a value"));
        match a.as_str() {
            "--line" | "-l" => o.line = Some(val("--line")?.parse().map_err(|_| "invalid --line")?),
            "--name" | "-n" => o.name = Some(val("--name")?),
            "--env" | "-e" => o.env = Some(val("--env")?),
            "--timeout" => o.timeout = val("--timeout")?.parse().map_err(|_| "invalid --timeout")?,
            "--all" | "-a" => o.all = true,
            "--raw" => o.raw = true,
            s if s.starts_with('-') => return Err(format!("unknown option {s}\n\n{USAGE}")),
            s => o.file = PathBuf::from(s),
        }
    }
    if o.file.as_os_str().is_empty() {
        return Err(format!("missing <file.http>\n\n{USAGE}"));
    }
    Ok(o)
}

/// Runs one command and converts its exit code. `args` are the arguments after the program name.
pub fn run_main(args: Vec<String>) -> ExitCode {
    ExitCode::from(run_args(args))
}

/// Runs one command, returning the process exit code: 0 ok, 1 request failed, 2 usage/IO error.
pub fn run_args(args: Vec<String>) -> u8 {
    if args.first().map(String::as_str) == Some("lsp") {
        lsp::serve();
        return 0;
    }
    if args.first().map(String::as_str) == Some("new") {
        return match new_endpoint(args.into_iter().skip(1)) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{RED}error:{RESET} {e}");
                2
            }
        };
    }
    if args.first().map(String::as_str) == Some("frameworks") {
        return match frameworks(args.into_iter().nth(1)) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{RED}error:{RESET} {e}");
                2
            }
        };
    }
    if args.first().map(String::as_str) == Some("detect") {
        return match detect(args.into_iter().skip(1)) {
            Ok(true) => 0,
            Ok(false) => 1,
            Err(e) => {
                eprintln!("{RED}error:{RESET} {e}");
                2
            }
        };
    }
    let opts = match parse_args(args.into_iter()) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    match run(opts) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            eprintln!("{RED}error:{RESET} {e}");
            2
        }
    }
}

fn run(o: Opts) -> Result<bool, String> {
    let src = fs::read_to_string(&o.file).map_err(|e| format!("{}: {e}", o.file.display()))?;
    let file = parser::parse(&src);
    let dir = o.file.parent().map(Path::to_path_buf).unwrap_or_default();
    let dir = if dir.as_os_str().is_empty() { PathBuf::from(".") } else { dir };
    let envs = Environments::load(&dir)?;

    match o.command.as_str() {
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
            let selected = select_requests(&file, &o)?;
            // Each request in its own environment: --env, then its `# @env`, then the file's.
            // All are checked before anything is sent.
            let mut chosen = Vec::new();
            for req in &selected {
                let own = o.env.clone().or_else(|| req.env.as_ref().map(|(_, name)| name.clone()));
                chosen.push(envs.select(vars::requested_env(own, &src).as_deref())?);
            }
            let store_path = dir.join(".zed-http").join("responses.json");
            let mut ctx = Context { vars: Default::default(), responses: load_store(&store_path) };

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
                ctx.vars = envs.vars(env.as_deref());
                ctx.vars.extend(file.variables.iter().cloned());
                match execute(req, &mut ctx, &dir, &o) {
                    Ok(success) => ok &= success,
                    Err(e) => {
                        eprintln!("{RED}error:{RESET} {e}");
                        ok = false;
                    }
                }
            }
            save_store(&store_path, &ctx.responses);
            Ok(ok)
        }
        other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    }
}

fn select_requests<'a>(file: &'a HttpFile, o: &Opts) -> Result<Vec<&'a Request>, String> {
    if file.requests.is_empty() {
        return Err("no requests found in file".into());
    }
    if o.all {
        return Ok(file.requests.iter().collect());
    }
    if let Some(n) = &o.name {
        return file
            .requests
            .iter()
            .find(|r| r.name.as_deref() == Some(n) || r.title.as_deref() == Some(n))
            .map(|r| vec![r])
            .ok_or_else(|| format!("request `{n}` not found"));
    }
    let line = o.line.ok_or("specify --line, --name or --all")?;
    file.requests
        .iter()
        .find(|r| line >= r.start_line && line <= r.end_line)
        .or_else(|| file.requests.iter().rev().find(|r| r.start_line <= line))
        .map(|r| vec![r])
        .ok_or_else(|| format!("no request at line {line}"))
}

fn execute(req: &Request, ctx: &mut Context, dir: &Path, o: &Opts) -> Result<bool, String> {
    let url = ctx.resolve(&req.url)?;
    let url = if url.contains("://") { url } else { format!("http://{url}") };
    let mut headers = Vec::new();
    for (k, v) in &req.headers {
        let mut v = ctx.resolve(v)?;
        if k.eq_ignore_ascii_case("authorization") {
            v = expand_basic_auth(&v);
        }
        headers.push((ctx.resolve(k)?, v));
    }
    let body: Option<Vec<u8>> = match &req.body {
        None => None,
        Some(Body::Inline(b)) => Some(resolve_body(ctx, b, dir)?.into_bytes()),
        Some(Body::File(p)) => {
            let p = ctx.resolve(p)?;
            let path = dir.join(&p);
            Some(fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?)
        }
    };

    println!("{BOLD}{CYAN}{}{RESET} {BOLD}{url}{RESET}", req.method);
    for (k, v) in &headers {
        println!("{DIM}{k}: {v}{RESET}");
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(o.timeout))
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
    let pretty = match (&json, o.raw) {
        (Some(j), false) => serde_json::to_string_pretty(j).expect("a parsed JSON value re-serializes"),
        _ => text.to_string(),
    };
    if is_textual(&content_type) || json.is_some() {
        println!("{pretty}");
    } else {
        println!("{DIM}<{} of binary data ({content_type})>{RESET}", human_size(bytes.len()));
    }

    let key = req.name.clone().unwrap_or_else(|| format!("line-{}", req.start_line));
    ctx.responses.insert(
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
    if fs::create_dir_all(&out_dir).is_ok() {
        let data = if json.is_some() { pretty.into_bytes() } else { bytes.clone() };
        if fs::write(&saved, data).is_ok() {
            println!("\n{DIM}saved: {}{RESET}", saved.display());
        }
    }

    if let Some((target, force)) = &req.redirect {
        let target = dir.join(ctx.resolve(target)?);
        let target = if !*force && target.exists() { unique_path(&target) } else { target };
        if let Some(p) = target.parent() {
            let _ = fs::create_dir_all(p);
        }
        fs::write(&target, &bytes).map_err(|e| format!("{}: {e}", target.display()))?;
        println!("{DIM}written to: {}{RESET}", target.display());
    }

    Ok(status < 400)
}

fn resolve_body(ctx: &Context, body: &str, dir: &Path) -> Result<String, String> {
    let mut out = Vec::new();
    for line in body.lines() {
        if let Some(p) = line.trim().strip_prefix("< ") {
            let path = dir.join(ctx.resolve(p.trim())?);
            out.push(fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?);
        } else {
            out.push(ctx.resolve(line)?);
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

/// Whether a response of this content type should be printed as text.
pub fn is_textual(ct: &str) -> bool {
    ct.starts_with("text/")
        || ["json", "xml", "javascript", "html", "yaml", "x-www-form-urlencoded", "graphql"]
            .iter()
            .any(|t| ct.contains(t))
        || ct.is_empty()
}

/// File extension for a saved response.
pub fn extension_for(ct: &str, is_json: bool) -> &'static str {
    if is_json {
        "json"
    } else if ct.contains("html") {
        "html"
    } else if ct.contains("xml") {
        "xml"
    } else if ct.contains("yaml") {
        "yaml"
    } else if ct.contains("png") {
        "png"
    } else if ct.contains("jpeg") {
        "jpg"
    } else if is_textual(ct) {
        "txt"
    } else {
        "bin"
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
    (1..).map(|i| p.with_file_name(format!("{stem}-{i}{ext}"))).find(|c| !c.exists()).unwrap()
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

/// Writes the stored responses, creating the directory; failures are ignored.
pub fn save_store(path: &Path, store: &serde_json::Map<String, Value>) {
    if let Some(p) = path.parent() {
        let _ = fs::create_dir_all(p);
    }
    let _ = fs::write(path, serde_json::to_string_pretty(store).unwrap_or_default());
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
                let (ep, _, content) = endpoint::generate(&catalog, &src, Path::new(file), line.saturating_sub(1))
                    .ok_or_else(|| format!("no HTTP endpoint found at {from}"))?;
                method.get_or_insert(ep.method);
                url.get_or_insert(ep.url);
                generated = Some(content);
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
    if vars::find_env_dir(&dir).is_none() {
        let env = base.join("http-client.env.json");
        fs::write(&env, endpoint::DEFAULT_ENV).map_err(|e| format!("{}: {e}", env.display()))?;
        println!("{GREEN}created:{RESET} {}", env.display());
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
