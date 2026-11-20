"""Tests tool/flutter_lsp.dart, the language server next to the Dart one:

- it answers initialize, shutdown and exit, and refuses other requests;
- it offers the snippets of tool/snippets.json as completions for the word being typed (those whose
  prefix starts with it, in any case), none at an empty word, after `{`, `(`, `.`, `,`, a space or
  at the start of a line, and none outside the text;
- each with edits that add the
  imports it needs where `dart format` keeps imports (a `dart:` one among the `dart:` ones, or
  first with a blank line; others last; after a `library` directive), or none when the file has
  the import, or one that brings the same (`debugPrint` comes with any Flutter library, widgets
  with material); edits to the file count;
- it runs `dart run build_runner watch --delete-conflicting-outputs` (with `--workspace` for a pub
  workspace) for the package of the file saved last, before any save of the first file opened: one
  watch at a time, moving when a file of another package is saved; with the `build_runner` option
  "open", one for each package with an open file, "all", for every package; again when a
  pubspec.yaml changes, once the start has settled, and when a watch ends because another
  build_runner command took its lock; forwards its output as log messages and its Succeeded /
  Failed lines as one message to show, stops it (and what it started) on shutdown, and does
  nothing when turned off by the `build_runner` initialization option or
  FLUTTER_TOOLS_BUILD_RUNNER=off; against a stand-in build_runner, a package that only prints;
- it copies the programs of the run and DevTools tasks (flutter_run.dart, flutter_devtools.dart
  and what they import) to ~/.zed-flutter/, and copies them again when they differ;
- it writes nothing into the project.

    python3 tests/check_lsp.py        # needs `dart` in PATH
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
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


def completions(lsp, uri, line, character, kind=2):
    """The items offered with the cursor at `character` of `line`, by label; `kind` is the LSP
    triggerKind: 2 as Zed sends after a trigger character such as `{`, 1 for typed letters and
    ctrl-space (editor::ShowCompletions)."""
    lsp.seq += 1
    lsp.send({
        "jsonrpc": "2.0",
        "id": lsp.seq,
        "method": "textDocument/completion",
        "params": {
            "textDocument": {"uri": uri},
            "position": {"line": line, "character": character},
            "context": {"triggerKind": kind},
        },
    })
    seq = lsp.seq
    items = lsp.wait(lambda m: m.get("id") == seq, 30, "the completion response")["result"]["items"]
    return {i["label"]: i for i in items}


def change(lsp, uri, text):
    lsp.send({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {"textDocument": {"uri": uri, "version": 2}, "contentChanges": [{"text": text}]}})


def offered(lsp, uri, text, *words):
    """What is offered, by label, as each of `words` is typed on a new last line of `text` (which
    ends with a newline); the file is left as `text`."""
    items = {}
    for word in words:
        change(lsp, uri, text + word)
        items.update(completions(lsp, uri, text.count("\n"), len(word)))
    change(lsp, uri, text)
    return items


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
    check("completionProvider" in capabilities and capabilities.get("textDocumentSync") == {"openClose": True, "change": 2, "save": {"includeText": False}}, "it offers completions, reading the changes of files (and told of saves)")

    def open_file(name, text):
        uri = (app / "lib" / name).as_uri()
        lsp.send({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}}})
        return uri

    print("== a file without imports")
    prefixes = [snippet["prefix"] for snippet in json.loads(SNIPPETS.read_text()).values()]
    items = offered(lsp, open_file("bare.dart", "void main() {\n}\n"), "void main() {\n}\n", *prefixes)
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
    check(sorted(items) == sorted(prefixes), "every snippet is offered, when its prefix is typed")

    print("== a Flutter file")
    text = "import 'dart:async';\nimport 'dart:math';\n\nimport 'package:flutter/material.dart';\n\nvoid main() {}\n"
    uri = open_file("app.dart", text)
    items = offered(lsp, uri, text, "log", "tm", "logd", "sfp")
    check(import_edit(items["log"]) == (2, "import 'dart:developer';\n"), "a dart: import goes after the other dart: ones")
    check(import_edit(items["tm"]) is None, "no edit when the file has the import")
    check(import_edit(items["logd"]) is None, "debugPrint comes with material.dart")
    check(import_edit(items["sfp"]) is None, "widgets.dart comes with material.dart")

    print("== package imports only, a library directive")
    text = "import 'package:flutter/widgets.dart';\n\nvoid f() {}\n"
    items = offered(lsp, open_file("pkg.dart", text), text, "log", "scf", "logd")
    check(import_edit(items["log"]) == (0, "import 'dart:developer';\n\n"), "a dart: import goes first, a blank line apart")
    check(import_edit(items["scf"]) == (1, "import 'package:flutter/material.dart';\n"), "material.dart does not come with widgets.dart")
    check(import_edit(items["logd"]) is None, "debugPrint comes with widgets.dart")
    text = "/// Docs.\nlibrary my_lib;\n\nvoid f() {}\n"
    items = offered(lsp, open_file("lib.dart", text), text, "log")
    check(import_edit(items["log"]) == (2, "import 'dart:developer';\n"), "after the library directive")

    print("== edits to the file")
    text = "import 'dart:developer';\n" + text
    change(lsp, uri, text)
    check(import_edit(offered(lsp, uri, text, "log")["log"]) is None, "an import added since is seen")
    # Incremental changes: the import's line removed, then `log` typed on the last line.
    change(lsp, uri, text)
    edit = lambda start, end, new: lsp.send({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {"textDocument": {"uri": uri, "version": 3}, "contentChanges": [{"range": {"start": {"line": start[0], "character": start[1]}, "end": {"line": end[0], "character": end[1]}}, "text": new}]}})
    edit((0, 0), (1, 0), "")
    last = text.count("\n") - 1
    edit((last, 0), (last, 0), "log")
    items = completions(lsp, uri, last, 3)
    check(import_edit(items["log"]) == (2, "import 'dart:developer';\n"), "changes of a range are applied: the removed import is missing again")

    print("== the word at the cursor")
    snippets = json.loads(SNIPPETS.read_text()).values()
    starting = lambda word: {s["prefix"] for s in snippets if s["prefix"].startswith(word)}
    text = "void main() {}\n\n  sc\nSCF\na(sc, b)\nx.\n\tsfp"
    uri = open_file("word.dart", text)
    at = lambda line, character, kind=2: completions(lsp, uri, line, character, kind)
    check(at(0, 13) == {}, "nothing after {")
    check(at(0, 10) == {}, "nothing after (")
    check(at(0, 14) == {}, "nothing after }")
    check(at(1, 0) == {}, "nothing on an empty line")
    check(at(2, 0) == {} and at(2, 2) == {}, "nothing before a word, after the spaces")
    check(set(at(2, 4)) == starting("sc") and len(starting("sc")) > 1, "after `sc`, the snippets that start with sc")
    check(set(at(2, 3)) == starting("s") and len(starting("s")) > 4, "after `s`, those that start with s")
    check(set(at(3, 3)) == {"scf"}, "after `SCF`, in any case")
    check(set(at(4, 4)) == starting("sc") and at(4, 2) == {} and at(4, 5) == {}, "in the middle of a line, only the word before the cursor counts")
    check(at(5, 2) == {}, "nothing after .")
    check(set(at(2, 99)) == starting("sc"), "a column past the end of the line is its end")
    check(at(99, 0) == {} and at(-1, 0) == {}, "nothing outside the text")
    everything = {s["prefix"] for s in snippets}
    check(set(at(0, 13, kind=1)) == everything and set(at(1, 0, kind=1)) == everything, "ctrl-space (Invoked) after { or on an empty line: every snippet")
    check(set(at(2, 4, kind=1)) == starting("sc"), "ctrl-space after `sc`: only those that start with sc")
    check(set(at(6, 4)) == {"sfp"}, "after a tab and `sfp`, itself only")
    check(import_edit(at(2, 4)["scf"]) == (0, "import 'package:flutter/material.dart';\n"), "the matching snippets still add their imports")
    check(offered(lsp, uri, "void f() {}\n", "zzz") == {}, "nothing for a word no snippet starts with")
    check(completions(lsp, "file:///no/such.dart", 0, 0) == {}, "nothing for a file that was never opened")
    text = "a\r\nsc\r\n"
    uri = open_file("crlf.dart", text)
    check(set(completions(lsp, uri, 1, 2)) == starting("sc"), "lines end with CRLF too")

    lsp.send({"jsonrpc": "2.0", "id": 2, "method": "shutdown"})
    lsp.wait(lambda m: m.get("id") == 2, 30, "the shutdown response")
    lsp.send({"jsonrpc": "2.0", "method": "exit"})
    lsp.close()


# What stands in for build_runner: a package of that name that prints what build_runner prints and
# then runs on, beating into a file, which shows whether it is still running.
FAKE_BUILD_RUNNER = """\
import 'dart:async';
import 'dart:io';

Future<void> main(List<String> args) async {
  print('args: ${args.join(' ')}');
  stderr.writeln('a warning');
  await Future<void>.delayed(const Duration(milliseconds: 300));
  print('[INFO] Succeeded after 0.1s with 0 outputs (0 actions)');
  print('[SEVERE] Failed after 0.2s');
  final beats = File(Platform.environment['FAKE_BUILD_RUNNER_BEATS']!);
  Timer.periodic(const Duration(milliseconds: 100), (_) {
    beats.writeAsStringSync('.', mode: FileMode.append);
    // Another build_runner command asked for the lock: build_runner 2.17 stops the watch.
    final request = File('yield');
    if (request.existsSync()) {
      request.deleteSync();
      print('Exiting as requested by another build_runner process.');
      exit(0);
    }
  });
}
"""

PUBSPEC = "name: p\n\ndependencies:\n  flutter:\n    sdk: flutter\n\ndev_dependencies:\n  # generators\n  build_runner: ^2.4.0\n  test: ^1.0.0\n"


def check_build_runner_projects(tmp):
    print("== where build_runner is run")

    def package(directory, pubspec):
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "pubspec.yaml").write_bytes(pubspec.encode())

    def targets(root):
        run = subprocess.run([shutil.which("dart"), str(HERE / "build_runner_projects.dart"), str(root)], capture_output=True, text=True, timeout=120)
        if run.returncode != 0:
            return run.stderr
        return [(Path(line.rsplit(" ", 1)[0]).resolve(), line.rsplit(" ", 1)[1]) for line in run.stdout.splitlines()]

    def projects(root):
        found = targets(root)
        return found if isinstance(found, str) else [path for path, _ in found]

    ws = tmp / "ws"
    package(ws, PUBSPEC)
    package(ws / "a", "name: a\ndev_dependencies:\n    build_runner:\n      path: ../x\n")
    package(ws / "b", "name: b\ndependencies:\n  build_runner: ^2.0.0\n")
    package(ws / "c", "name: c\ndev_dependencies:\n  build_runner_cli: ^1.0.0\n  # build_runner: ^2.0.0\n  test: ^1.0.0\n")
    package(ws / ".hidden", PUBSPEC)
    package(ws / "d" / "inner", PUBSPEC)
    (ws / "e").mkdir()
    package(ws / "f", PUBSPEC.replace("\n", "\r\n"))
    package(ws / "g", "name: g\ndev_dependencies:\n  test: ^1.0.0\nflutter:\n  build_runner: true\n")
    package(ws / "h", 'name: h\ndev_dependencies:\n  "build_runner": ^2.0.0\n')
    package(ws / "i", "name: i\ndev_dependencies:\n  foo:\n    git:\n      build_runner: x\n")
    package(ws / "build" / "out", PUBSPEC)
    package(ws / "node_modules" / "m", PUBSPEC)
    package(ws / "j" / "k" / "l" / "m", PUBSPEC)
    package(ws / "j" / "k" / "l" / "m" / "n", PUBSPEC)
    check(
        projects(ws) == [(ws / n).resolve() for n in ("", "a", "d/inner", "f", "h", "j/k/l/m")],
        "the root and the packages below it that have build_runner in dev_dependencies, not hidden ones, "
        "build output, other sections, or deeper than 4 levels",
    )
    mono = tmp / "mono"
    package(mono / "x", PUBSPEC)
    package(mono / "y", "name: y\n")
    check(projects(mono) == [(mono / "x").resolve()], "a root without a pubspec, a package of it")
    check(projects(tmp / "nowhere") == [], "a root that is not there")

    def lock(directory, version):
        (directory / "pubspec.lock").write_text(
            f'packages:\n  build_runner:\n    dependency: "direct dev"\n    source: hosted\n    version: "{version}"\n'
        )

    watch_args = targets

    workspace = tmp / "workspace"
    package(workspace, "name: root\nworkspace:\n  - app\n  - core\n" + PUBSPEC.split("\n", 1)[1])
    package(workspace / "app", "name: app\nresolution: workspace\n" + PUBSPEC.split("\n", 1)[1])
    package(workspace / "core", "name: core\nresolution: workspace\n")
    lock(workspace, "2.17.1")
    check(
        watch_args(workspace) == [(workspace.resolve(), "--workspace")],
        "a pub workspace: one watch with --workspace in its root, none per member",
    )
    lock(workspace, "2.13.0")
    check(
        [path for path, _ in watch_args(workspace)] == [workspace.resolve(), (workspace / "app").resolve()]
        and all(arg != "--workspace" for _, arg in watch_args(workspace)),
        "build_runner before 2.14: a watch per package, as before",
    )
    members_only = tmp / "members_only"
    package(members_only, "name: root\nworkspace:\n  - app\n")
    package(members_only / "app", "name: app\nresolution: workspace\n" + PUBSPEC.split("\n", 1)[1])
    lock(members_only, "2.17.1")
    check(
        watch_args(members_only) == [((members_only / "app").resolve(), "--workspace")],
        "a root without build_runner: the member's watch builds the workspace",
    )
    monorepo = tmp / "monorepo"
    package(monorepo, "name: root\nworkspace:\n  - core\n  - example/cli\n")
    package(monorepo / "core", "name: core\nresolution: workspace\n" + PUBSPEC.split("\n", 1)[1])
    package(monorepo / "example" / "cli", "name: cli\nresolution: workspace\n" + PUBSPEC.split("\n", 1)[1])
    package(monorepo / "example" / "flutter_app", PUBSPEC)
    lock(monorepo, "2.17.1")
    check(
        watch_args(monorepo)
        == [((monorepo / "core").resolve(), "--workspace"), ((monorepo / "example" / "flutter_app").resolve(), "--delete-conflicting-outputs")],
        "a member two levels down is built by the workspace's watch; an app outside the workspace gets its own",
    )
    globbed = tmp / "globbed"
    package(globbed, "name: root\nworkspace:\n  - packages/*\n  - 'apps/**'\n")
    for member in ("packages/core", "packages/ui", "apps/shop/mobile"):
        package(globbed / member, f"name: {member.rsplit('/', 1)[1]}\nresolution: workspace\n" + PUBSPEC.split("\n", 1)[1])
    lock(globbed, "2.17.1")
    check(
        watch_args(globbed) == [((globbed / "apps" / "shop" / "mobile").resolve(), "--workspace")],
        "members listed by globs (`packages/*`, `apps/**`) are the workspace's: one watch builds them all",
    )


def serve(home, folder, env, options=None):
    """Starts the server on `folder` as Zed does: initialize, then initialized."""
    lsp = Dap([shutil.which("dart"), str(LSP)], env=dict(os.environ, HOME=str(home), USERPROFILE=str(home), **env))
    params = {"capabilities": {}, "workspaceFolders": [{"uri": folder.as_uri(), "name": folder.name}]}
    if options is not None:
        params["initializationOptions"] = options
    lsp.send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": params})
    lsp.wait(lambda m: m.get("id") == 1, 60, "the initialize response")
    lsp.send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
    return lsp


def open_document(lsp, path, text="void main() {}\n"):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    lsp.send({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": path.as_uri(), "languageId": "dart", "version": 1, "text": text}}})


def save_document(lsp, path):
    lsp.send({"jsonrpc": "2.0", "method": "textDocument/didSave", "params": {"textDocument": {"uri": path.as_uri()}}})


def close_document(lsp, path):
    lsp.send({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {"uri": path.as_uri()}}})


def stop(lsp):
    lsp.send({"jsonrpc": "2.0", "id": 2, "method": "shutdown"})
    lsp.wait(lambda m: m.get("id") == 2, 30, "the shutdown response")
    lsp.send({"jsonrpc": "2.0", "method": "exit"})
    lsp.close()
    return lsp.process.returncode


def check_build_runner(home, tmp):
    print("== build_runner")
    fake = tmp / "build_runner"
    (fake / "bin").mkdir(parents=True)
    (fake / "pubspec.yaml").write_text("name: build_runner\nenvironment:\n  sdk: ^3.0.0\n")
    (fake / "bin" / "build_runner.dart").write_text(FAKE_BUILD_RUNNER)

    def project(name):
        name = f"p_{name}"  # `on` and `env` are no package names
        directory = tmp / name
        directory.mkdir()
        (directory / "pubspec.yaml").write_text(
            f"name: {name}\nenvironment:\n  sdk: ^3.0.0\ndev_dependencies:\n  build_runner:\n    path: '{fake.as_posix()}'\n"
        )
        return directory

    beats = {name: tmp / f"{name}.beats" for name in ("on", "env", "option")}
    env = lambda name, **more: dict(FAKE_BUILD_RUNNER_BEATS=str(beats[name]), **more)
    servers = []
    try:
        # The two that are turned off start first: whatever they would run is running by the time the other has output.
        off = {
            "env": serve(home, project("env"), env("env", FLUTTER_TOOLS_BUILD_RUNNER="off")),
            "option": serve(home, project("option"), env("option"), {"build_runner": False}),
        }
        on_project = project("on")
        on = serve(
            home,
            on_project,
            env(
                "on",
                FLUTTER_TOOLS_BUILD_RUNNER_POLL_MS="200",
                FLUTTER_TOOLS_BUILD_RUNNER_SETTLE_MS="3000",
                FLUTTER_TOOLS_BUILD_RUNNER_YIELD_MS="1500",
            ),
        )
        servers = [on, *off.values()]
        # The default scope: the watch of the package of an open file.
        open_document(on, on_project / "lib" / "main.dart")
        on.wait(lambda m: m.get("method") == "window/showMessage" and m["params"]["type"] == 1, 180, "the message that the build failed")

        def sent(method):
            return [(m["params"]["type"], m["params"]["message"]) for m in on.seen if m.get("method") == method]

        logs, shown = sent("window/logMessage"), sent("window/showMessage")
        check((3, "build_runner p_on: args: watch --delete-conflicting-outputs") in logs, "it runs `dart run build_runner watch --delete-conflicting-outputs` in the project")
        check(sum("args:" in message for _, message in logs) == 1, "once")
        check((2, "build_runner p_on: a warning") in logs, "stderr is logged as a warning")
        check((3, "build_runner p_on: [INFO] Succeeded after 0.1s with 0 outputs (0 actions)") in logs, "stdout lines are logged")
        check(shown == [(1, "build_runner p_on: [SEVERE] Failed after 0.2s")], "of the lines of builds that end together, the last is shown (Failed as an error)")
        deadline = time.monotonic() + 30
        while not (beats["on"].exists() and beats["on"].stat().st_size) and time.monotonic() < deadline:
            time.sleep(0.1)
        check(beats["on"].exists() and beats["on"].stat().st_size > 0, "build_runner is running")

        # After the settling time, a changed pubspec.yaml restarts the watch.
        time.sleep(3.5)
        check(sum("args:" in message for _, message in sent("window/logMessage")) == 1, "no restart while nothing changes")
        with (on_project / "pubspec.yaml").open("a") as pubspec:
            pubspec.write("# changed\n")
        on.wait(
            lambda m: m.get("method") == "window/logMessage" and "args:" in m["params"]["message"]
            and sum("args:" in x["params"]["message"] for x in on.seen if x.get("method") == "window/logMessage") >= 2,
            60,
            "the restarted watch",
        )
        check(sum("args:" in message for _, message in sent("window/logMessage")) == 2, "a changed pubspec.yaml restarts the watch once")

        # A `build_runner build` by hand takes the lock: the watch exits, and starts again.
        (on_project / "yield").write_text("")
        on.wait(
            lambda m: m.get("method") == "window/logMessage" and "args:" in m["params"]["message"]
            and sum("args:" in x["params"]["message"] for x in on.seen if x.get("method") == "window/logMessage") >= 3,
            60,
            "the watch started again after giving way",
        )
        logs = sent("window/logMessage")
        check((3, "build_runner p_on: restarting") in logs, "a watch that gave its lock away starts again")

        # Another build_runner asks for the lock and the watch keeps it (a stuck build_runner):
        # it is stopped after FLUTTER_TOOLS_BUILD_RUNNER_YIELD_MS and started again.
        requested = on_project / ".dart_tool" / "build" / "lock" / "build_runner.lock.requested"
        requested.parent.mkdir(parents=True, exist_ok=True)
        requested.write_text("")
        on.wait(
            lambda m: m.get("method") == "window/logMessage" and "args:" in m["params"]["message"]
            and sum("args:" in x["params"]["message"] for x in on.seen if x.get("method") == "window/logMessage") >= 4,
            60,
            "the watch started again after keeping its lock",
        )
        requested.unlink()
        check(
            any(t == 2 and "kept its lock" in message for t, message in sent("window/logMessage")),
            "a watch that keeps its lock after a request is stopped and started again",
        )
        for name, lsp in off.items():
            lsp.drain(1)
            seen = [m for m in lsp.seen if "build_runner" in json.dumps(m)]
            check(not beats[name].exists() and not seen, f"nothing runs when it is turned off by the {name}")

        check(stop(on) == 0, "the server exits on exit")
        time.sleep(1.5)
        size = beats["on"].stat().st_size
        time.sleep(1)
        check(beats["on"].stat().st_size == size, "build_runner, and what it started, stop with the server")
    finally:
        for lsp in servers:
            lsp.close()


def check_build_runner_workspace(home, tmp):
    print("== build_runner in a pub workspace")
    # A build_runner that builds workspaces (2.14+), the watch in a member, since the root does
    # not depend on build_runner itself.
    fake = tmp / "ws_build_runner"
    (fake / "bin").mkdir(parents=True)
    (fake / "pubspec.yaml").write_text("name: build_runner\nversion: 2.17.1\nenvironment:\n  sdk: ^3.6.0\n")
    (fake / "bin" / "build_runner.dart").write_text(FAKE_BUILD_RUNNER)
    root = tmp / "ws"
    member = root / "m"
    member.mkdir(parents=True)
    (root / "pubspec.yaml").write_text("name: ws\nenvironment:\n  sdk: ^3.6.0\nworkspace:\n  - m\n")
    (member / "pubspec.yaml").write_text(
        f"name: m\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\ndev_dependencies:\n  build_runner:\n    path: '{fake.as_posix()}'\n"
    )
    subprocess.run([shutil.which("dart"), "pub", "get"], cwd=root, capture_output=True, check=True, timeout=120)
    beats = tmp / "ws.beats"
    lsp = serve(
        home,
        root,
        dict(
            FAKE_BUILD_RUNNER_BEATS=str(beats),
            FLUTTER_TOOLS_BUILD_RUNNER_POLL_MS="200",
            FLUTTER_TOOLS_BUILD_RUNNER_SETTLE_MS="3000",
        ),
    )
    try:
        def starts():
            return [m["params"]["message"] for m in lsp.seen if m.get("method") == "window/logMessage" and "args:" in m["params"]["message"]]

        # A file of the workspace's root, which does not use build_runner itself: the root's
        # package is built by the workspace's watch.
        open_document(lsp, root / "bin" / "tool.dart")
        lsp.wait(lambda m: m.get("method") == "window/logMessage" and "args:" in m["params"]["message"], 180, "the workspace watch")
        check(starts() == ["build_runner m: args: watch --delete-conflicting-outputs --workspace"], "a file of any package of a workspace: one `--workspace` watch, in the member that uses build_runner")
        time.sleep(3.5)
        # A member added and `pub get` in the root: only the root's files change.
        with (root / "pubspec.yaml").open("a") as pubspec:
            pubspec.write("# a member added\n")
        (root / ".dart_tool" / "package_config.json").touch()
        lsp.wait(lambda m: len(starts()) >= 2, 60, "the watch restarted for the workspace's root")
        check(len(starts()) == 2, "a change of the workspace's root restarts the `--workspace` watch")
        stop(lsp)
    finally:
        lsp.close()


def check_build_runner_scopes(home, tmp):
    print("== which packages get a watch")
    fake = tmp / "scopes_build_runner"
    (fake / "bin").mkdir(parents=True)
    (fake / "pubspec.yaml").write_text("name: build_runner\nenvironment:\n  sdk: ^3.0.0\n")
    (fake / "bin" / "build_runner.dart").write_text(FAKE_BUILD_RUNNER)
    folder = tmp / "scopes"
    for name in ("p_a", "p_b"):
        (folder / name).mkdir(parents=True)
        (folder / name / "pubspec.yaml").write_text(
            f"name: {name}\nenvironment:\n  sdk: ^3.0.0\ndev_dependencies:\n  build_runner:\n    path: '{fake.as_posix()}'\n"
        )
        subprocess.run([shutil.which("dart"), "pub", "get"], cwd=folder / name, capture_output=True, check=True, timeout=120)

    def starts(lsp):
        return [m["params"]["message"].split(":")[0] for m in lsp.seen if m.get("method") == "window/logMessage" and "args:" in m["params"]["message"]]

    def logs(lsp):
        return [m["params"]["message"] for m in lsp.seen if m.get("method") == "window/logMessage"]

    def watching(lsp):
        """The packages with a watch running, from the starts and stops logged."""
        running = []
        for message in logs(lsp):
            name = message.split(":")[0].removeprefix("build_runner ")
            if "args:" in message:
                running.append(name)
            elif message.endswith(": stopped: no longer needed") and name in running:
                running.remove(name)
        return running

    beats = tmp / "scopes.beats"
    timings = dict(FLUTTER_TOOLS_BUILD_RUNNER_POLL_MS="200", FLUTTER_TOOLS_BUILD_RUNNER_LINGER_MS="1500", FLUTTER_TOOLS_BUILD_RUNNER_STAGGER_MS="200")
    a = folder / "p_a" / "lib" / "a.dart"
    b = folder / "p_b" / "lib" / "b.dart"
    lsp = serve(home, folder, dict(FAKE_BUILD_RUNNER_BEATS=str(beats), **timings))
    try:
        lsp.drain(2)
        check(starts(lsp) == [], "by default, nothing runs before a file is open")
        open_document(lsp, a)
        lsp.wait(lambda m: starts(lsp), 120, "the watch of the open file's package")
        open_document(lsp, b)
        lsp.drain(2)
        check(starts(lsp) == ["build_runner p_a"], "by default one watch, of the first open file's package; a file of another package opened starts none")
        save_document(lsp, b)
        lsp.wait(lambda m: watching(lsp) == ["p_b"], 60, "the watch of the saved file's package")
        check(
            logs(lsp).index("build_runner p_a: stopped: no longer needed") < max(i for i, m in enumerate(logs(lsp)) if m.startswith("build_runner p_b: args:")),
            "a file of another package saved: the one watch moves there (p_a's stops before p_b's starts)",
        )
        close_document(lsp, b)
        lsp.wait(lambda m: watching(lsp) == ["p_a"], 30, "the watch back on the package of the file still open")
        check(True, "a while after the last file of the watched package closed, the watch moves to the package of a file still open")
        close_document(lsp, a)
        lsp.wait(lambda m: watching(lsp) == [] and logs(lsp).count("build_runner p_a: stopped: no longer needed") == 2, 30, "no watch once no file is open")
        check(True, "a while after the last file closed, no watch runs")
        stop(lsp)
    finally:
        lsp.close()

    lsp = serve(home, folder, dict(FAKE_BUILD_RUNNER_BEATS=str(beats), **timings), {"build_runner": "open"})
    try:
        open_document(lsp, a)
        open_document(lsp, b)
        lsp.wait(lambda m: len(starts(lsp)) == 2, 120, "a watch for each package with an open file")
        check(sorted(starts(lsp)) == ["build_runner p_a", "build_runner p_b"], '"open": a watch for each package with an open file')
        close_document(lsp, a)
        lsp.wait(lambda m: watching(lsp) == ["p_b"], 30, "the watch of p_a stopped")
        check(True, '"open": a watch stops a while after the last file of its package closed')
        stop(lsp)
    finally:
        lsp.close()

    lsp = serve(
        home,
        folder,
        dict(
            FAKE_BUILD_RUNNER_BEATS=str(beats),
            FLUTTER_TOOLS_BUILD_RUNNER_POLL_MS="200",
            FLUTTER_TOOLS_BUILD_RUNNER_STAGGER_MS="1500",
            FLUTTER_TOOLS_BUILD_RUNNER_SUMMARY_MS="6000",
        ),
        {"build_runner": "all"},
    )
    try:
        lsp.wait(lambda m: m.get("method") == "window/showMessage", 180, "the message of both builds")
        lsp.drain(1)
        check(starts(lsp) == ["build_runner p_a", "build_runner p_b"], '"all": every package that uses build_runner gets a watch')
        shown = [(m["params"]["type"], m["params"]["message"]) for m in lsp.seen if m.get("method") == "window/showMessage"]
        check(shown == [(1, "build_runner: failed in p_a, p_b")], f"builds that end together make one message: {shown}")
        stop(lsp)
    finally:
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
        same = lambda: all((runner / n).read_text() == (TOOL / n).read_text() for n in ("flutter_run.dart", "dart_run.dart", "devices.dart", "flutter_devtools.dart", "devtools.dart", "pubspec.dart", "build_runner.dart", "build_runner_run.dart"))
        check(same(), "the runner is copied to ~/.zed-flutter/")
        check(sorted(p.relative_to(app) for p in app.rglob("*")) == before, "nothing is written into the project")

        (runner / "flutter_run.dart").write_text("// an older version\n")
        start(home, app)
        check(same(), "a runner that differs is replaced")

        check_completions(home, app)
        check_build_runner_projects(tmp)
        check_build_runner(home, tmp)
        check_build_runner_workspace(home, tmp)
        check_build_runner_scopes(home, tmp)

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
