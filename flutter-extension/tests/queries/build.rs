fn main() {
    // The Dart grammar, from the checkout check_queries.py makes of the commit extension.toml pins.
    let dart = std::path::PathBuf::from(std::env::var("GRAMMAR_SRC").unwrap());
    println!("cargo:rerun-if-env-changed=GRAMMAR_SRC");
    let mut b = cc::Build::new();
    b.include(&dart).file(dart.join("parser.c")).warnings(false);
    if dart.join("scanner.c").exists() {
        b.file(dart.join("scanner.c"));
    }
    b.compile("tree-sitter-dart");

    // The Dartdoc grammar of this extension.
    let dartdoc = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tree-sitter-dartdoc/src");
    println!("cargo:rerun-if-changed={}", dartdoc.join("parser.c").display());
    cc::Build::new()
        .include(&dartdoc)
        .file(dartdoc.join("parser.c"))
        .warnings(false)
        .compile("tree-sitter-dartdoc");
}
