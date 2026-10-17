fn main() {
    let src = std::path::PathBuf::from(std::env::var("GRAMMAR_SRC").unwrap());
    println!("cargo:rerun-if-env-changed=GRAMMAR_SRC");
    let mut b = cc::Build::new();
    b.include(&src).file(src.join("parser.c")).warnings(false);
    if src.join("scanner.c").exists() { b.file(src.join("scanner.c")); }
    b.compile("tree-sitter-dart");
}
