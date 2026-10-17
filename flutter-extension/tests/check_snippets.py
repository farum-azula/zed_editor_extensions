"""Checks snippets/dart.json:

1. no two snippets, and no snippet and a Dart language server snippet, share a prefix;
2. every body parses the way Zed parses snippets (a `$` not starting a tab stop breaks the whole
   snippet, so a Dart `$` must be written `\\$`);
3. every occurrence of a tab stop has the same default text, so that accepting a default fills all
   of them (an occurrence without one would stay empty);
4. every snippet, expanded with its defaults and put where it is meant to be typed (see
   snippet_contexts.json), is Dart that `dart analyze` finds no errors in.

    python3 tests/check_snippets.py            # needs `flutter` in PATH
"""

import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SNIPPETS = HERE.parent / "snippets" / "dart.json"
CONTEXTS = HERE / "snippet_contexts.json"

# Prefixes of the Dart language server's own snippets, which these must not repeat.
DART_LSP_PREFIXES = {
    "stless", "stful", "stanim", "class", "main", "fun", "for", "forin", "if", "ife",
    "switch", "try", "while", "do", "test", "group",
}

PREAMBLE = """\
import 'dart:async';
import 'dart:developer';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
"""

# Where each kind of snippet is typed. `@@` is replaced by the expanded snippet.
CONTEXTS_SOURCE = {
    "top": "@@\n",
    "member": """\
abstract class _Host extends State<StatefulWidget> with SingleTickerProviderStateMixin {
  String _name = '';

@@
}
""",
    "statement": """\
abstract class _Host extends State<StatefulWidget> {
  Future<void> run(BuildContext context, Object? value, List<String> items) async {
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
    "expr": """\
Object run(BuildContext context) => @@;
""",
    "test": """\
void main() {
@@
}
""",
    "tester": """\
void main() {
  testWidgets('host', (tester) async {
@@
  });
}
""",
}

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


def main():
    snippets = json.loads(SNIPPETS.read_text())
    contexts = json.loads(CONTEXTS.read_text())
    failures = []

    prefixes = {}
    for name, snip in snippets.items():
        prefixes.setdefault(snip["prefix"], []).append(name)
    for prefix, names in prefixes.items():
        if len(names) > 1:
            failures.append(f"prefix {prefix!r} is used by {names}")
        if prefix in DART_LSP_PREFIXES:
            failures.append(f"prefix {prefix!r} of {names} is a Dart language server snippet")
    for name in contexts.keys() - snippets.keys():
        failures.append(f"{name}: in snippet_contexts.json, but not a snippet")

    sources = {}
    for name, snip in snippets.items():
        context = contexts.get(name)
        if context not in CONTEXTS_SOURCE:
            failures.append(f"{name}: no context in snippet_contexts.json (one of {list(CONTEXTS_SOURCE)})")
            continue
        try:
            expanded, stops = parse("\n".join(snip["body"]))
        except SnippetError as e:
            failures.append(f"{name}: Zed cannot parse it: {e}")
            continue
        for index, defaults in stops.items():
            if index != 0 and len(set(defaults)) > 1:
                failures.append(f"{name}: tab stop ${index} has different defaults {defaults}")
        sources[name] = PREAMBLE + CONTEXTS_SOURCE[context].replace("@@", expanded)

    flutter = shutil.which("flutter")
    if not flutter:
        failures.append("`flutter` is not in PATH, the snippets were not analyzed")
    else:
        with tempfile.TemporaryDirectory() as project:
            project = Path(project)
            (project / "pubspec.yaml").write_text(PUBSPEC)
            lib = project / "lib"
            lib.mkdir()
            files = {}
            for n, (name, source) in enumerate(sources.items()):
                file = lib / f"snippet_{n}.dart"
                file.write_text(source)
                files[file.name] = name
            subprocess.run([flutter, "pub", "get"], cwd=project, check=True, capture_output=True)
            result = subprocess.run(
                [flutter, "analyze", "--no-pub", "--no-fatal-infos", "--no-fatal-warnings", "lib"],
                cwd=project,
                capture_output=True,
                text=True,
            )
            errors = [l for l in result.stdout.splitlines() if l.strip().startswith("error")]
            for line in errors:
                m = re.search(r"(snippet_\d+\.dart):(\d+)", line)
                name = files.get(m.group(1)) if m else None
                failures.append(f"{name}: {line.strip()}")
            if result.returncode != 0 and not errors:
                failures.append(f"flutter analyze failed:\n{result.stdout}\n{result.stderr}")

    for failure in failures:
        print(f"FAIL  {failure}")
    print(f"{len(snippets)} snippets, {len(failures)} problem(s)")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
