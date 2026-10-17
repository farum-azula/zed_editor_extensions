"""Checks tool/snippets.json, the snippets that the flutter-tools language server offers, each with
an edit that adds the imports it needs when the file does not have them:

1. no two snippets, and no snippet and a Dart language server snippet, share a prefix;
2. every body parses the way Zed parses snippets (a `$` not starting a tab stop breaks the whole
   snippet, so a Dart `$` must be written `\\$`);
3. every occurrence of a tab stop has the same default text, so that accepting a default fills all
   of them (an occurrence without one would stay empty);
4. every snippet, expanded with its defaults and put where it is meant to be typed (its `context`),
   is Dart that `flutter analyze` finds no errors in with its `imports` and no others, and with no
   fewer: the imports are found by trying, in order, none, one Dart library, one Flutter library,
   and a Flutter library with a Dart one, first in a file with no Flutter in it, then in a Flutter
   widget's State.

    python3 tests/check_snippets.py            # needs `flutter` in PATH
    python3 tests/check_snippets.py --update   # writes the imports found into tool/snippets.json
"""

import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SNIPPETS = HERE.parent / "tool" / "snippets.json"

# Prefixes of the Dart language server's own snippets, which these must not repeat.
DART_LSP_PREFIXES = {
    "stless", "stful", "stanim", "class", "main", "fun", "for", "forin", "if", "ife",
    "switch", "try", "while", "do", "test", "group",
}

ASYNC = "dart:async"
DEVELOPER = "dart:developer"
FOUNDATION = "package:flutter/foundation.dart"
WIDGETS = "package:flutter/widgets.dart"
MATERIAL = "package:flutter/material.dart"
CUPERTINO = "package:flutter/cupertino.dart"
TEST = "package:flutter_test/flutter_test.dart"

# Where each kind of snippet is typed, in a file with no Flutter in it and in a Flutter one. `@@` is
# replaced by the expanded snippet.
PLAIN = {
    "top": "@@\n",
    "member": """\
abstract class _Host {
  String _name = '';

@@
}
""",
    "statement": """\
Future<void> run(Object? value, List<String> items, Object error, StackTrace stackTrace) async {
@@
}
""",
    "expr": "Object run() => @@;\n",
}

FLUTTER = {
    "member": """\
abstract class _Host extends State<StatefulWidget> with SingleTickerProviderStateMixin {
  String _name = '';

@@
}
""",
    "statement": """\
abstract class _Host extends State<StatefulWidget> {
  Future<void> run(
    BuildContext context,
    Object? value,
    List<String> items,
    Object error,
    StackTrace stackTrace,
  ) async {
@@
  }
}
""",
    "widget": """\
abstract class _Host extends State<StatefulWidget> with SingleTickerProviderStateMixin {
  final items = <String>[];
  final notifier = ValueNotifier<int>(0);
  final _controller = TextEditingController();
  final _formKey = GlobalKey<FormState>();
  late final AnimationController _animationController = AnimationController(vsync: this);
  late final Future<String> future;
  late final Stream<String> stream;

  Widget run(BuildContext context) {
    return @@;
  }
}
""",
    "expr": "Object run(BuildContext context) => @@;\n",
}

TESTS = {
    "test": "void main() {\n@@\n}\n",
    "tester": """\
void main() {
  testWidgets('host', (tester) async {
@@
  });
}
""",
}

PLAIN_IMPORTS = [[], [ASYNC], [DEVELOPER], [FOUNDATION], [WIDGETS], [MATERIAL], [CUPERTINO]]
FLUTTER_IMPORTS = [
    [WIDGETS], [MATERIAL], [CUPERTINO], [WIDGETS, ASYNC], [MATERIAL, ASYNC], [WIDGETS, DEVELOPER],
]
TEST_IMPORTS = [[TEST], [TEST, WIDGETS], [TEST, MATERIAL]]

CONTEXTS = {*PLAIN, *FLUTTER, *TESTS}

PUBSPEC = """\
name: snippet_check
publish_to: none
environment:
  sdk: ^3.5.0
dependencies:
  flutter:
    sdk: flutter
  flutter_test:
    sdk: flutter
"""


class SnippetError(Exception):
    pass


def parse(source):
    """Zed's snippet grammar (crates/snippet): returns the text with the defaults filled in and,
    per tab stop, the default text of each occurrence."""
    text = []
    stops = {}

    def snippet(i, nested):
        while i < len(source):
            c = source[i]
            if c == "$":
                i = tabstop(i + 1)
            elif c == "\\":
                if i + 1 < len(source) and source[i + 1] in "$\\}":
                    text.append(source[i + 1])
                    i += 2
                else:
                    text.append("\\")
                    i += 1
            elif c == "}":
                if nested:
                    return i
                text.append("}")
                i += 1
            else:
                text.append(c)
                i += 1
        if nested:
            raise SnippetError("expected a closing brace")
        return i

    def number(i):
        m = re.match(r"\d+", source[i:])
        if not m:
            raise SnippetError(f"`$` not followed by a tab stop number at {source[i - 1:i + 15]!r}")
        return int(m.group()), i + len(m.group())

    def tabstop(i):
        start = len("".join(text))
        if source.startswith("{", i):
            index, i = number(i + 1)
            if source.startswith("|", i):
                end = source.find("|", i + 1)
                if end < 0:
                    raise SnippetError("choice without a closing `|`")
                choices = source[i + 1:end].split(",")
                text.append(choices[0])
                i = end + 1
            if source.startswith(":", i):
                i = snippet(i + 1, True)
            if not source.startswith("}", i):
                raise SnippetError("expected a closing brace")
            i += 1
        else:
            index, i = number(i)
        stops.setdefault(index, []).append("".join(text)[start:])
        return i

    snippet(0, False)
    return "".join(text), stops


def candidates(context, expanded):
    """The files a snippet typed in `context` may compile in, in the order of preference, with
    the imports of each."""
    result = []
    if context in TESTS:
        for imports in TEST_IMPORTS:
            result.append((imports, TESTS[context]))
    else:
        if context in PLAIN:
            result += [(imports, PLAIN[context]) for imports in PLAIN_IMPORTS]
        if context in FLUTTER:
            result += [(imports, FLUTTER[context]) for imports in FLUTTER_IMPORTS]
    return [
        (imports, "".join(f"import '{i}';\n" for i in imports) + "\n" + host.replace("@@", expanded))
        for imports, host in result
    ]


def analyze(flutter, sources):
    """Analyzes the files `sources` (a list of texts) in one project; returns, per file, its
    errors."""
    errors = [[] for _ in sources]
    with tempfile.TemporaryDirectory() as project:
        project = Path(project)
        (project / "pubspec.yaml").write_text(PUBSPEC)
        lib = project / "lib"
        lib.mkdir()
        for n, source in enumerate(sources):
            (lib / f"snippet_{n}.dart").write_text(source)
        subprocess.run([flutter, "pub", "get"], cwd=project, check=True, capture_output=True)
        result = subprocess.run(
            [flutter, "analyze", "--no-pub", "--no-fatal-infos", "--no-fatal-warnings", "lib"],
            cwd=project,
            capture_output=True,
            text=True,
        )
        lines = [l.strip() for l in result.stdout.splitlines() if l.strip().startswith("error")]
        for line in lines:
            m = re.search(r"snippet_(\d+)\.dart:\d+", line)
            if not m:
                raise SnippetError(f"flutter analyze: {line}")
            errors[int(m.group(1))].append(line)
        if result.returncode != 0 and not lines:
            raise SnippetError(f"flutter analyze failed:\n{result.stdout}\n{result.stderr}")
    return errors


def main():
    update = "--update" in sys.argv[1:]
    snippets = json.loads(SNIPPETS.read_text())
    failures = []

    prefixes = {}
    for name, snip in snippets.items():
        prefixes.setdefault(snip["prefix"], []).append(name)
    for prefix, names in prefixes.items():
        if len(names) > 1:
            failures.append(f"prefix {prefix!r} is used by {names}")
        if prefix in DART_LSP_PREFIXES:
            failures.append(f"prefix {prefix!r} of {names} is a Dart language server snippet")

    tried = {}
    for name, snip in snippets.items():
        if snip.get("context") not in CONTEXTS:
            failures.append(f"{name}: its context must be one of {sorted(CONTEXTS)}")
            continue
        try:
            expanded, stops = parse("\n".join(snip["body"]))
        except SnippetError as e:
            failures.append(f"{name}: Zed cannot parse it: {e}")
            continue
        for index, defaults in stops.items():
            if index != 0 and len(set(defaults)) > 1:
                failures.append(f"{name}: tab stop ${index} has different defaults {defaults}")
        tried[name] = candidates(snip["context"], expanded)

    flutter = shutil.which("flutter")
    if not flutter:
        failures.append("`flutter` is not in PATH, the snippets were not analyzed")
        tried = {}
    files = [(name, imports, source) for name, tries in tried.items() for imports, source in tries]
    errors = analyze(flutter, [source for _, _, source in files]) if files else []

    found = {}
    for (name, imports, _), errs in zip(files, errors):
        if not errs and name not in found:
            found[name] = imports
    for name in tried:
        if name not in found:
            first = next(errs for (n, _, _), errs in zip(files, errors) if n == name)
            failures.append(f"{name}: compiles with none of the imports tried, e.g. {first[0]}")
        elif update:
            snip = snippets[name]
            snip.pop("imports", None)
            if found[name]:
                # `imports` after `context`, before the body.
                body = snip.pop("body")
                snip["imports"] = found[name]
                snip["body"] = body
        elif snippets[name].get("imports", []) != found[name]:
            failures.append(
                f"{name}: needs the imports {found[name]}, not {snippets[name].get('imports', [])}"
                " (run with --update)"
            )

    if update and not failures:
        SNIPPETS.write_text(json.dumps(snippets, indent=2, ensure_ascii=False) + "\n")
        print(f"wrote the imports into {SNIPPETS.name}")
    for failure in failures:
        print(f"FAIL  {failure}")
    with_imports = sum(1 for s in snippets.values() if s.get("imports"))
    print(f"{len(snippets)} snippets, {with_imports} with imports, {len(failures)} problem(s)")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
