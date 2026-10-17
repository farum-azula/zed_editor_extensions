"""Tests tool/flutter_lsp.dart, the language server next to the Dart one:

- it answers initialize, shutdown and exit, and refuses other requests;
- it offers the snippets of tool/snippets.json as completions, each with edits that add the
  imports it needs where `dart format` keeps imports (a `dart:` one among the `dart:` ones, or
  first with a blank line; others last; after a `library` directive), or none when the file has
  the import, or one that brings the same (`debugPrint` comes with any Flutter library, widgets
  with material); edits to the file count;
- it copies the programs of the run and DevTools tasks (flutter_run.dart, flutter_devtools.dart
  and what they import) to ~/.zed-flutter/, and copies them again when they differ;
- it writes nothing into the project.

    python3 tests/check_lsp.py        # needs `dart` in PATH
"""

import json
import os
import shutil
import sys
import tempfile
from pathlib import Path

from dap import Dap

HERE = Path(__file__).resolve().parent
TOOL = HERE.parent / "tool"
SNIPPETS = HERE.parent / "tool" / "snippets.json"
# The server, in a copy of the work directory the extension writes it into (set by main).
LSP = None

failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


def start(home, folder):
    """Starts the server on `folder`, as Zed does, and shuts it down; returns its answers."""
    lsp = Dap([shutil.which("dart"), str(LSP)], env=dict(os.environ, HOME=str(home), USERPROFILE=str(home)))
    lsp.send({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {"capabilities": {}, "workspaceFolders": [{"uri": folder.as_uri(), "name": folder.name}]},
    })
    initialize = lsp.wait(lambda m: m.get("id") == 1, 60, "the initialize response")
    lsp.send({"jsonrpc": "2.0", "id": 2, "method": "textDocument/hover", "params": {}})
    hover = lsp.wait(lambda m: m.get("id") == 2, 30, "the hover response")
    lsp.send({"jsonrpc": "2.0", "id": 3, "method": "shutdown"})
    lsp.wait(lambda m: m.get("id") == 3, 30, "the shutdown response")
    lsp.send({"jsonrpc": "2.0", "method": "exit"})
    lsp.close()
    return initialize, hover, lsp.process.returncode


def completions(lsp, uri):
    lsp.seq += 1
    lsp.send({
        "jsonrpc": "2.0",
        "id": lsp.seq,
        "method": "textDocument/completion",
        "params": {"textDocument": {"uri": uri}, "position": {"line": 0, "character": 0}},
    })
    seq = lsp.seq
    items = lsp.wait(lambda m: m.get("id") == seq, 30, "the completion response")["result"]["items"]
    return {i["label"]: i for i in items}


def import_edits(item):
    return [(e["range"]["start"]["line"], e["newText"]) for e in item.get("additionalTextEdits") or []]


def import_edit(item):
    edits = import_edits(item)
    return edits[0] if len(edits) == 1 else edits or None


def check_completions(home, app):
    lsp = Dap([shutil.which("dart"), str(LSP)], env=dict(os.environ, HOME=str(home), USERPROFILE=str(home)))
    lsp.seq = 100
    lsp.send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"capabilities": {}}})
    capabilities = lsp.wait(lambda m: m.get("id") == 1, 60, "the initialize response")["result"]["capabilities"]
    check("completionProvider" in capabilities and capabilities.get("textDocumentSync") == 1, "it offers completions, reading whole files")

    def open_file(name, text):
        uri = (app / "lib" / name).as_uri()
        lsp.send({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}}})
        return uri

    print("== a file without imports")
    items = completions(lsp, open_file("bare.dart", "void main() {\n}\n"))
    log = items.get("log", {})
    check(log.get("kind") == 15 and log.get("insertTextFormat") == 2 and log.get("insertText", "").startswith("log("), "`log` is a snippet")
    check(import_edit(log) == (0, "import 'dart:developer';\n"), "it adds import 'dart:developer' at the top")
    check(import_edit(items["tm"]) == (0, "import 'dart:async';\n"), "`tm` adds import 'dart:async'")
    check(import_edit(items["logd"]) == (0, "import 'package:flutter/foundation.dart';\n"), "`logd` adds flutter/foundation.dart")
    check(import_edit(items["scf"]) == (0, "import 'package:flutter/material.dart';\n"), "`scf` adds flutter/material.dart")
    check(import_edit(items["sfp"]) == (0, "import 'package:flutter/widgets.dart';\n"), "`sfp` adds flutter/widgets.dart")
    check(
        import_edits(items["tw"]) == [
            (0, "import 'package:flutter_test/flutter_test.dart';\n"),
            (0, "import 'package:flutter/material.dart';\n"),
        ],
        "`tw` adds flutter_test and flutter/material.dart",
    )
    check(import_edit(items["pr"]) is None, "`pr` needs no import")
    check(len(items) == len(json.loads(SNIPPETS.read_text())), "every snippet is offered")

    print("== a Flutter file")
    text = "import 'dart:async';\nimport 'dart:math';\n\nimport 'package:flutter/material.dart';\n\nvoid main() {}\n"
    uri = open_file("app.dart", text)
    items = completions(lsp, uri)
    check(import_edit(items["log"]) == (2, "import 'dart:developer';\n"), "a dart: import goes after the other dart: ones")
    check(import_edit(items["tm"]) is None, "no edit when the file has the import")
    check(import_edit(items["logd"]) is None, "debugPrint comes with material.dart")
    check(import_edit(items["sfp"]) is None, "widgets.dart comes with material.dart")

    print("== package imports only, a library directive")
    items = completions(lsp, open_file("pkg.dart", "import 'package:flutter/widgets.dart';\n\nvoid f() {}\n"))
    check(import_edit(items["log"]) == (0, "import 'dart:developer';\n\n"), "a dart: import goes first, a blank line apart")
    check(import_edit(items["scf"]) == (1, "import 'package:flutter/material.dart';\n"), "material.dart does not come with widgets.dart")
    check(import_edit(items["logd"]) is None, "debugPrint comes with widgets.dart")
    items = completions(lsp, open_file("lib.dart", "/// Docs.\nlibrary my_lib;\n\nvoid f() {}\n"))
    check(import_edit(items["log"]) == (2, "import 'dart:developer';\n"), "after the library directive")

    print("== edits to the file")
    lsp.send({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {"textDocument": {"uri": uri, "version": 2}, "contentChanges": [{"text": "import 'dart:developer';\n" + text}]}})
    check(import_edit(completions(lsp, uri)["log"]) is None, "an import added since is seen")

    lsp.send({"jsonrpc": "2.0", "id": 2, "method": "shutdown"})
    lsp.wait(lambda m: m.get("id") == 2, 30, "the shutdown response")
    lsp.send({"jsonrpc": "2.0", "method": "exit"})
    lsp.close()


def main():
    global LSP
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        home, app, work = tmp / "home", tmp / "app", tmp / "work"
        home.mkdir()
        # What the extension writes into its work directory (TOOLS of src/lib.rs).
        shutil.copytree(TOOL, work)
        LSP = work / "flutter_lsp.dart"
        (app / "lib").mkdir(parents=True)
        (app / "pubspec.yaml").write_text("name: app\ndependencies:\n  flutter:\n    sdk: flutter\n")
        before = sorted(p.relative_to(app) for p in app.rglob("*"))

        initialize, hover, code = start(home, app)
        check(initialize["result"]["serverInfo"]["name"] == "flutter-tools", "it answers initialize")
        check(hover.get("error", {}).get("code") == -32601, "it refuses other requests")
        check(code == 0, "it exits on exit")

        runner = home / ".zed-flutter"
        same = lambda: all((runner / n).read_text() == (TOOL / n).read_text() for n in ("flutter_run.dart", "devices.dart", "flutter_devtools.dart", "devtools.dart"))
        check(same(), "the runner is copied to ~/.zed-flutter/")
        check(sorted(p.relative_to(app) for p in app.rglob("*")) == before, "nothing is written into the project")

        (runner / "flutter_run.dart").write_text("// an older version\n")
        start(home, app)
        check(same(), "a runner that differs is replaced")

        check_completions(home, app)

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
