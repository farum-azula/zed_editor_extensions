//! Runs the Dart grammar (built from GRAMMAR_SRC by build.rs) for check_queries.py:
//!
//!     dart-queries-check compile <query.scm>...       each query compiles against the grammar
//!     dart-queries-check errors <file.dart>           the file parses without errors
//!     dart-queries-check captures <highlights.scm> <file.dart>
//!         every capture, in the order Zed applies them: `row:column<TAB>capture<TAB>text`

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

extern "C" {
    fn tree_sitter_dart() -> *const std::ffi::c_void;
}

fn dart() -> Language {
    unsafe { std::mem::transmute(tree_sitter_dart()) }
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
                if let Err(e) = Query::new(&dart(), &read(path)) {
                    println!("{path}: {e}");
                    failed = true;
                }
            }
            std::process::exit(failed as i32);
        }
        Some("errors") => {
            let text = read(&args[2]);
            let mut parser = Parser::new();
            parser.set_language(&dart()).unwrap();
            let tree = parser.parse(&text, None).unwrap();
            let mut errors = Vec::new();
            let mut cursor = tree.walk();
            let mut stack = vec![tree.root_node()];
            while let Some(node) = stack.pop() {
                if node.is_error() || node.is_missing() {
                    let p = node.start_position();
                    errors.push(format!("{}:{} {}", p.row + 1, p.column + 1, node.to_sexp()));
                }
                stack.extend(node.children(&mut cursor));
            }
            for e in &errors {
                println!("{e}");
            }
            std::process::exit(!errors.is_empty() as i32);
        }
        Some("captures") => {
            let query = Query::new(&dart(), &read(&args[2])).unwrap();
            let text = read(&args[3]);
            let mut parser = Parser::new();
            parser.set_language(&dart()).unwrap();
            let tree = parser.parse(&text, None).unwrap();
            let mut cursor = QueryCursor::new();
            let mut captures = cursor.captures(&query, tree.root_node(), text.as_bytes());
            while let Some((m, i)) = captures.next() {
                let capture = m.captures[*i];
                let p = capture.node.start_position();
                println!(
                    "{}:{}\t{}\t{}",
                    p.row + 1,
                    p.column + 1,
                    query.capture_names()[capture.index as usize],
                    capture.node.utf8_text(text.as_bytes()).unwrap().replace('\n', " ")
                );
            }
        }
        _ => {
            eprintln!("usage: dart-queries-check compile|errors|captures ...");
            std::process::exit(64);
        }
    }
}
