//! Runs the Dart grammar (built from GRAMMAR_SRC by build.rs), this extension's Dartdoc grammar and
//! those of the languages of examples (Cargo.toml) for check_queries.py:
//!
//!     dart-queries-check compile <query.scm>...       each query compiles against its grammar, the
//!                                                     `grammar` of its language's config.toml
//!     dart-queries-check errors <file.dart>           the file parses without errors
//!     dart-queries-check captures <highlights.scm> <file.dart>
//!         every capture, in the order Zed applies them: `row:column<TAB>capture<TAB>text`
//!     dart-queries-check layers <languages> <file.dart>
//!         the file's syntax layers as Zed builds them from the injections.scm of each language
//!         in <languages>, and for each layer its parse errors and highlights.scm captures:
//!             layer<TAB>depth<TAB>language<TAB>start-end,...   (byte ranges it parses)
//!             error<TAB>depth<TAB>language<TAB>row:column<TAB>node
//!             capture<TAB>depth<TAB>row:column<TAB>capture<TAB>text
//!             pending<TAB>depth<TAB>language                   (no such language)
//!     dart-queries-check runnables <runnables.scm> <file.dart>
//!         every `@run` capture with its tag: `row:column<TAB>text<TAB>tag`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Range, Tree};

extern "C" {
    fn tree_sitter_dart() -> *const std::ffi::c_void;
    fn tree_sitter_dartdoc() -> *const std::ffi::c_void;
}

fn dart() -> Language {
    unsafe { std::mem::transmute(tree_sitter_dart()) }
}

fn dartdoc() -> Language {
    unsafe { std::mem::transmute(tree_sitter_dartdoc()) }
}

fn grammar(name: &str) -> Language {
    match name {
        "dart" => dart(),
        "dartdoc" => dartdoc(),
        "java" => tree_sitter_java::LANGUAGE.into(),
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "python" => tree_sitter_python::LANGUAGE.into(),
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        _ => panic!("no grammar {name}"),
    }
}

/// The `key = "value"` of a config.toml.
fn config_value(config: &str, key: &str) -> Option<String> {
    config.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        (k.trim() == key).then(|| v.trim().to_string())
    })
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("compile") => {
            let mut failed = false;
            for path in &args[2..] {
                let config = Path::new(path).with_file_name("config.toml");
                let name = config_value(&read(config.to_str().unwrap()), "grammar").unwrap();
                let language = grammar(name.trim_matches('"'));
                if let Err(e) = Query::new(&language, &read(path)) {
                    println!("{path}: {e}");
                    failed = true;
                }
            }
            std::process::exit(failed as i32);
        }
        Some("errors") => {
            let text = read(&args[2]);
            let tree = parse(&dart(), &text, None);
            let errors = errors(tree.root_node());
            for (node, p) in &errors {
                println!("{}:{} {}", p.row + 1, p.column + 1, node);
            }
            std::process::exit(!errors.is_empty() as i32);
        }
        Some("captures") => {
            let query = Query::new(&dart(), &read(&args[2])).unwrap();
            let text = read(&args[3]);
            let tree = parse(&dart(), &text, None);
            for (position, name, text) in captures(&query, &tree, &text) {
                println!("{position}\t{name}\t{text}");
            }
        }
        Some("layers") => layers(Path::new(&args[2]), &read(&args[3])),
        Some("runnables") => {
            // Each `@run` capture with the `tag` its pattern sets: what Zed
            // shows a run button for, and which tasks it offers there.
            let query = Query::new(&dart(), &read(&args[2])).unwrap();
            let text = read(&args[3]);
            let tree = parse(&dart(), &text, None);
            let run = query.capture_index_for_name("run").unwrap();
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), text.as_bytes());
            while let Some(m) = matches.next() {
                let tag = query
                    .property_settings(m.pattern_index)
                    .iter()
                    .find(|p| &*p.key == "tag")
                    .and_then(|p| p.value.as_deref().map(str::to_string))
                    .unwrap_or_default();
                for node in m.nodes_for_capture_index(run) {
                    let p = node.start_position();
                    let name = node.utf8_text(text.as_bytes()).unwrap();
                    println!("{}:{}\t{name}\t{tag}", p.row + 1, p.column + 1);
                }
            }
        }
        _ => {
            eprintln!("usage: dart-queries-check compile|errors|captures|layers|runnables ...");
            std::process::exit(64);
        }
    }
}

fn parse(language: &Language, text: &str, ranges: Option<&[Range]>) -> Tree {
    let mut parser = Parser::new();
    parser.set_language(language).unwrap();
    if let Some(ranges) = ranges {
        parser.set_included_ranges(ranges).unwrap();
    }
    parser.parse(text, None).unwrap()
}

fn errors(root: Node) -> Vec<(String, tree_sitter::Point)> {
    let mut errors = Vec::new();
    let mut cursor = root.walk();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            errors.push((node.to_sexp(), node.start_position()));
        }
        stack.extend(node.children(&mut cursor));
    }
    errors
}

fn captures(query: &Query, tree: &Tree, text: &str) -> Vec<(String, String, String)> {
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(query, tree.root_node(), text.as_bytes());
    let mut out = Vec::new();
    while let Some((m, i)) = captures.next() {
        let capture = m.captures[*i];
        let p = capture.node.start_position();
        out.push((
            format!("{}:{}", p.row + 1, p.column + 1),
            query.capture_names()[capture.index as usize].to_string(),
            capture.node.utf8_text(text.as_bytes()).unwrap().replace('\n', " "),
        ));
    }
    out
}

/// A language of `languages/<dir>/config.toml`.
struct LanguageDir {
    name: String,
    grammar: String,
    suffixes: Vec<String>,
    dir: PathBuf,
}

fn languages(root: &Path) -> Vec<LanguageDir> {
    let mut languages = Vec::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let dir = entry.unwrap().path();
        let Ok(config) = std::fs::read_to_string(dir.join("config.toml")) else {
            continue;
        };
        let value = |key: &str| config_value(&config, key);
        let unquote = |s: &str| s.trim().trim_matches('"').to_string();
        languages.push(LanguageDir {
            name: unquote(&value("name").unwrap()),
            grammar: unquote(&value("grammar").unwrap()),
            suffixes: value("path_suffixes")
                .map(|v| {
                    v.trim_matches(|c| c == '[' || c == ']')
                        .split(',')
                        .map(unquote)
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            dir,
        });
    }
    languages
}

/// Zed's `language_for_name_or_extension`: a language named so, or with that path suffix, in any
/// case.
fn find(languages: &[LanguageDir], name: &str) -> Option<usize> {
    let name = name.to_lowercase();
    languages.iter().position(|l| {
        l.name.to_lowercase() == name || l.suffixes.iter().any(|s| s.to_lowercase() == name)
    })
}

/// What Zed's SyntaxSnapshot::reparse does from the root layer down (crates/language/src/
/// syntax_map.rs): each layer's injections.scm gives the layers below it: one per match, one for
/// the matches of each `injection.host` node, or one for all the matches of a combined pattern,
/// whose ranges get the newlines between them.
fn layers(root: &Path, text: &str) {
    let languages = languages(root);
    let mut queue = vec![(0usize, find(&languages, "dart").unwrap(), None::<Vec<Range>>)];
    while !queue.is_empty() {
        let (depth, index, ranges) = queue.remove(0);
        let language = &languages[index];
        let grammar = grammar(&language.grammar);
        let tree = parse(&grammar, text, ranges.as_deref());
        let spans: Vec<String> = match &ranges {
            Some(ranges) => ranges.iter().map(|r| format!("{}-{}", r.start_byte, r.end_byte)).collect(),
            None => vec![format!("0-{}", text.len())],
        };
        println!("layer\t{depth}\t{}\t{}", language.name, spans.join(","));
        for (node, p) in errors(tree.root_node()) {
            println!("error\t{depth}\t{}\t{}:{}\t{node}", language.name, p.row + 1, p.column + 1);
        }
        if let Ok(highlights) = std::fs::read_to_string(language.dir.join("highlights.scm")) {
            let query = Query::new(&grammar, &highlights).unwrap();
            for (position, name, text) in captures(&query, &tree, text) {
                println!("capture\t{depth}\t{position}\t{name}\t{text}");
            }
        }
        let Ok(injections) = std::fs::read_to_string(language.dir.join("injections.scm")) else {
            continue;
        };
        let query = Query::new(&grammar, &injections).unwrap();
        let index_of = |names: &[&str]| names.iter().find_map(|n| query.capture_index_for_name(n));
        let content = index_of(&["injection.content", "content"]).unwrap();
        let named = index_of(&["injection.language", "language"]);
        let host = index_of(&["injection.host"]);
        // Matches sharing a layer: of an `injection.host` node, or of a combined pattern.
        let mut groups: BTreeMap<(usize, Option<(usize, usize)>), Vec<Range>> = BTreeMap::new();
        let mut previous = None;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), text.as_bytes());
        while let Some(m) = matches.next() {
            let nodes: Vec<Range> = m.nodes_for_capture_index(content).map(|n| n.range()).collect();
            let (Some(first), Some(last)) = (nodes.first(), nodes.last()) else {
                continue;
            };
            // Zed skips a match like the one before it.
            let key = (m.pattern_index, first.start_byte, last.end_byte);
            if previous == Some(key) {
                continue;
            }
            previous = Some(key);
            let properties = query.property_settings(m.pattern_index);
            let property = |names: &[&str]| {
                properties.iter().find(|p| names.contains(&&*p.key)).map(|p| p.value.clone())
            };
            let name = match property(&["injection.language", "language"]) {
                Some(Some(name)) => name.to_string(),
                _ => match named.and_then(|i| m.nodes_for_capture_index(i).next()) {
                    Some(node) => {
                        let name = node.utf8_text(text.as_bytes()).unwrap();
                        name.rsplit('.').next().unwrap().to_string()
                    }
                    None => continue,
                },
            };
            let Some(injected) = find(&languages, &name) else {
                println!("pending\t{}\t{name}", depth + 1);
                continue;
            };
            let host = host.and_then(|i| m.nodes_for_capture_index(i).next());
            if let Some(host) = host {
                groups.entry((injected, Some((host.start_byte(), host.end_byte())))).or_default().extend(nodes);
            } else if property(&["injection.combined", "combined"]).is_some() {
                groups.entry((injected, None)).or_default().extend(nodes);
            } else {
                queue.push((depth + 1, injected, Some(nodes)));
            }
        }
        for ((injected, host), mut ranges) in groups {
            ranges.sort_by_key(|r| (r.start_byte, r.end_byte));
            ranges.dedup();
            // Only a combined layer gets the newlines; a host's is parsed as its ranges are.
            if host.is_none() {
                insert_newlines_between_ranges(&mut ranges, text);
            }
            queue.push((depth + 1, injected, Some(ranges)));
        }
    }
}

/// Zed's `insert_newlines_between_ranges`, for a layer made from scratch: a range on a line of
/// its own ends with that line's newline, so that the injected language sees lines.
fn insert_newlines_between_ranges(ranges: &mut Vec<Range>, text: &str) {
    let line_len = |row: usize| text.split('\n').nth(row).map_or(0, str::len);
    let mut ix = ranges.len();
    while ix > 1 {
        ix -= 1;
        let b = ranges[ix];
        let a = &mut ranges[ix - 1];
        if a.end_point.column == 0 || a.end_point.row >= b.start_point.row {
            continue;
        }
        let line_end = line_len(a.end_point.row);
        if a.end_point.column >= line_end {
            a.end_byte += 1;
            a.end_point.row += 1;
            a.end_point.column = 0;
        } else {
            let offset = a.end_byte - a.end_point.column + line_end;
            let at = tree_sitter::Point { row: a.end_point.row, column: line_end };
            ranges.insert(
                ix,
                Range {
                    start_byte: offset,
                    end_byte: offset + 1,
                    start_point: at,
                    end_point: tree_sitter::Point { row: at.row + 1, column: 0 },
                },
            );
        }
    }
}
