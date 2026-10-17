"""Checks the Dart language's tree-sitter queries against the grammar extension.toml pins:

- every query of languages/dart compiles: Zed drops a query that does not, and with highlights.scm
  that is all of the file's highlighting;
- tests/queries/highlights.dart parses without errors (it uses the newest Dart syntax the grammar
  is pinned for), and highlights.scm gives each place marked in it the capture expected.

In highlights.dart, a comment line with `^` (or `<-`, for its own first column) names the capture
expected at that column of the code line above it; a capture it falls back from counts too
(`keyword.definition` is a `keyword`).

    python3 tests/check_queries.py        # needs cargo and git
"""

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHECKER = ROOT / "tests" / "queries"
LANGUAGE = ROOT / "languages" / "dart"

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
        elif not line.lstrip().startswith("//"):
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

    queries = sorted(LANGUAGE.glob("*.scm"))
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
    for row, column, expected in expectations(fixture.read_text()):
        # Of the captures over that place, the innermost; of those, the last, as in Zed.
        over = [c for c in captures if c[0] == row and c[1] <= column < c[1] + c[2]]
        got = min(over, key=lambda c: (c[2], -c[3]))[4] if over else None
        line = fixture.read_text().splitlines()[row - 1]
        token = re.match(r"[\w$@']+", line[column - 1:])
        label = token.group() if token else line[column - 1:column + 9]
        ok = got is not None and (got == expected or got.startswith(expected + "."))
        check(ok, f"{row}:{column} {label!r} is {expected}" + ("" if ok else f" (got {got})"))

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
