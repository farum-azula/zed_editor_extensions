"""Checks the languages' tree-sitter queries against the grammars: Dart's, the commit
extension.toml pins, and Dartdoc's, tree-sitter-dartdoc in this extension.

- every query of languages/ compiles: Zed drops a query that does not, and with highlights.scm
  that is all of the file's highlighting;
- tests/queries/highlights.dart parses without errors (it uses the newest Dart syntax the grammar
  is pinned for), and highlights.scm gives each place marked in it the capture expected;
- tree-sitter-dartdoc/src is what its grammar.js generates, and its corpus (test/corpus) passes;
- in tests/queries/doc_comments.dart, built into layers the way Zed builds them (one Dartdoc layer
  for all the `///` comments, a Dartdoc Code layer for each Dart code block in them), the code
  blocks parse without errors and each place marked gets the capture expected: Dart's in a code
  block, `comment.documentation` elsewhere;
- languages/dartdoc-code is Dart's highlights.scm and nothing else, so that the code of examples
  stays out of the outline and gets no run button.

In the fixtures, a comment line with `^` (or `<-`, for its own first column) names the capture
expected at that column of the line above it; a capture it falls back from counts too
(`keyword.definition` is a `keyword`).

    python3 tests/check_queries.py        # needs cargo, git and npx
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHECKER = ROOT / "tests" / "queries"
LANGUAGES = ROOT / "languages"
LANGUAGE = LANGUAGES / "dart"
DARTDOC = ROOT / "tree-sitter-dartdoc"
# The tree-sitter CLI that generated tree-sitter-dartdoc/src.
TREE_SITTER_CLI = "0.27.1"

failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


def pinned_grammar():
    manifest = (ROOT / "extension.toml").read_text()
    section = manifest[manifest.index("[grammars.dart]"):]
    repository = re.search(r'^repository = "([^"]+)"', section, re.M).group(1)
    commit = re.search(r'^(?:commit|rev) = "([^"]+)"', section, re.M).group(1)
    return repository, commit


def expectations(fixture):
    """(row, column, capture) for each `^` / `<-` mark, 1-based."""
    expected = []
    code_row = None
    for row, line in enumerate(fixture.splitlines(), 1):
        mark = re.match(r"\s*//\s*(<-|\^)", line)
        if mark and code_row is not None:
            column = 1 if mark.group(1) == "<-" else line.index("^") + 1
            expected.append((code_row, column, line.split(mark.group(1), 1)[1].strip()))
        elif not line.lstrip().startswith("//") or line.lstrip().startswith("///"):
            code_row = row
    return expected


def main():
    repository, commit = pinned_grammar()
    with tempfile.TemporaryDirectory() as tmp:
        grammar = Path(tmp) / "grammar"
        subprocess.run(["git", "init", "-q", str(grammar)], check=True)
        for args in (
            ["remote", "add", "origin", repository],
            ["fetch", "-q", "--depth", "1", "origin", commit],
            ["-c", "advice.detachedHead=false", "checkout", "-q", commit],
        ):
            subprocess.run(["git", "-C", str(grammar), *args], check=True)
        subprocess.run(
            ["cargo", "build", "--quiet", "--release", "--manifest-path", str(CHECKER / "Cargo.toml")],
            env={**os.environ, "GRAMMAR_SRC": str(grammar / "src")},
            check=True,
        )
    exe = CHECKER / "target" / "release" / ("dart-queries-check" + (".exe" if sys.platform == "win32" else ""))
    print(f"== grammar {commit[:12]}")

    queries = sorted(LANGUAGES.glob("*/*.scm"))
    result = subprocess.run([exe, "compile", *map(str, queries)], capture_output=True, text=True)
    check(result.returncode == 0, f"the {len(queries)} queries compile {result.stdout.strip()}")

    fixture = CHECKER / "highlights.dart"
    result = subprocess.run([exe, "errors", str(fixture)], capture_output=True, text=True)
    check(result.returncode == 0, f"highlights.dart parses without errors {result.stdout.strip()}")

    out = subprocess.run([exe, "captures", str(LANGUAGE / "highlights.scm"), str(fixture)], capture_output=True, text=True, check=True).stdout
    captures = []
    for order, line in enumerate(out.splitlines()):
        position, name, text = line.split("\t", 2)
        row, column = map(int, position.split(":"))
        captures.append((row, column, len(text), order, name, text))

    print("== highlights")
    check_marks(fixture, [(0, *c) for c in captures])

    check_dartdoc_grammar()
    check_doc_comments(exe)

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


def check_marks(fixture, captures):
    """Checks each mark of `fixture` against `captures`: (depth, row, column, length, order, name)."""
    for row, column, expected in expectations(fixture.read_text()):
        # Of the captures over that place, those of the deepest layer, which Zed draws over the
        # others; of those, the innermost; of those, the last, as in Zed.
        over = [c for c in captures if c[1] == row and c[2] <= column < c[2] + c[3]]
        got = min(over, key=lambda c: (-c[0], c[3], -c[4]))[5] if over else None
        line = fixture.read_text().splitlines()[row - 1]
        token = re.match(r"[\w$@']+", line[column - 1:])
        label = token.group() if token else line[column - 1:column + 9]
        ok = got is not None and (got == expected or got.startswith(expected + "."))
        check(ok, f"{row}:{column} {label!r} is {expected}" + ("" if ok else f" (got {got})"))


def tree_sitter(*args, cwd):
    npx = shutil.which("npx")
    return subprocess.run(
        [npx, "--yes", f"tree-sitter-cli@{TREE_SITTER_CLI}", *args],
        cwd=cwd,
        capture_output=True,
        text=True,
    )


def check_dartdoc_grammar():
    print("== tree-sitter-dartdoc")
    with tempfile.TemporaryDirectory() as tmp:
        copy = Path(tmp) / "tree-sitter-dartdoc"
        shutil.copytree(DARTDOC, copy)
        result = tree_sitter("generate", cwd=copy)
        check(result.returncode == 0, f"grammar.js generates {result.stderr.strip()}")
        stale = [
            str(p.relative_to(copy))
            for p in sorted((copy / "src").rglob("*"))
            if p.is_file() and p.read_bytes() != (DARTDOC / p.relative_to(copy)).read_bytes()
        ]
        check(not stale, f"src/ is what grammar.js generates (regenerate: npx tree-sitter-cli@{TREE_SITTER_CLI} generate) {stale or ''}")
    result = tree_sitter("test", cwd=DARTDOC)
    print("\n".join("      " + l for l in result.stdout.splitlines() if "✓" in l or "✗" in l))
    check(result.returncode == 0, "its corpus passes")


def check_doc_comments(exe):
    print("== code blocks in documentation comments")
    fixture = CHECKER / "doc_comments.dart"
    out = subprocess.run([exe, "layers", str(LANGUAGES), str(fixture)], capture_output=True, text=True, check=True).stdout
    layers, errors, captures = [], [], []
    for order, line in enumerate(out.splitlines()):
        kind, *fields = line.split("\t")
        if kind == "layer":
            layers.append((int(fields[0]), fields[1], fields[2]))
        elif kind == "error":
            errors.append(fields)
        elif kind == "capture":
            row, column = map(int, fields[1].split(":"))
            captures.append((int(fields[0]), row, column, len(fields[3]), order, fields[2]))
    named = lambda name: [l for l in layers if l[1] == name]
    check([l[0] for l in named("Dartdoc")] == [1], "one Dartdoc layer, for all the /// comments")
    check([l[0] for l in named("Dartdoc Code")] == [2, 2, 2], "a Dartdoc Code layer for each Dart block (dart, no language, a member's), none for java or /** */")
    first = named("Dartdoc Code")[0][2].split(",")
    check(len(first) == 6, f"the lines of a block are one layer ({len(first)} lines)")
    check(not errors, f"the blocks parse without errors {errors or ''}")
    check_marks(fixture, captures)

    code = LANGUAGES / "dartdoc-code"
    check(
        (code / "highlights.scm").read_text() == (LANGUAGE / "highlights.scm").read_text(),
        "languages/dartdoc-code/highlights.scm is a copy of languages/dart/highlights.scm",
    )
    check(
        sorted(p.name for p in code.iterdir()) == ["config.toml", "highlights.scm"],
        "languages/dartdoc-code has no outline, runnables or tasks",
    )
    for name in ("dartdoc", "dartdoc-code"):
        check("hidden = true" in (LANGUAGES / name / "config.toml").read_text(), f"languages/{name} is hidden")


if __name__ == "__main__":
    main()
