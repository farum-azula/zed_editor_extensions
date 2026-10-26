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
- it runs `dart run build_runner watch --delete-conflicting-outputs` in the workspace's projects
  that have build_runner in their dev_dependencies (the root and the directories right under it),
  once each, forwards its output as log messages and its Succeeded / Failed lines as messages to
  show, stops it (and what it started) on shutdown, and does nothing when turned off by the
  `build_runner` initialization option or FLUTTER_TOOLS_BUILD_RUNNER=off; against a stand-in
  build_runner, a package that only prints;
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
    check("completionProvider" in capabilities and capabilities.get("textDocumentSync") == 1, "it offers completions, reading whole files")

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
  });
}
"""

PUBSPEC = "name: p\n\ndependencies:\n  flutter:\n    sdk: flutter\n\ndev_dependencies:\n  # generators\n  build_runner: ^2.4.0\n  test: ^1.0.0\n"


def check_build_runner_projects(tmp):
    print("== where build_runner is run")

    def package(directory, pubspec):
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "pubspec.yaml").write_bytes(pubspec.encode())

    def projects(root):
        run = subprocess.run([shutil.which("dart"), str(HERE / "build_runner_projects.dart"), str(root)], capture_output=True, text=True, timeout=120)
        return [Path(line).resolve() for line in run.stdout.splitlines()] if run.returncode == 0 else run.stderr

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

    def watch_args(root):
        probe = tmp / "probe_args.dart"
        probe.write_text(
            f"import 'dart:io';\nimport '{(HERE.parent / 'tool' / 'flutter_lsp.dart').as_uri()}';\n"
            "void main(List<String> a) { for (final p in buildRunnerProjects(Directory(a[0]))) "
            "print('${p.path} ${buildRunnerArgumentsFor(p).last}'); }\n"
        )
        run = subprocess.run([shutil.which("dart"), str(probe), str(root)], capture_output=True, text=True, timeout=120)
        return [(Path(line.rsplit(" ", 1)[0]).resolve(), line.rsplit(" ", 1)[1]) for line in run.stdout.splitlines()]

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
        on = serve(home, project("on"), env("on"))
        servers = [on, *off.values()]
        on.wait(lambda m: m.get("method") == "window/showMessage" and m["params"]["type"] == 1, 180, "the message that the build failed")

        def sent(method):
            return [(m["params"]["type"], m["params"]["message"]) for m in on.seen if m.get("method") == method]

        logs, shown = sent("window/logMessage"), sent("window/showMessage")
        check((3, "build_runner p_on: args: watch --delete-conflicting-outputs") in logs, "it runs `dart run build_runner watch --delete-conflicting-outputs` in the project")
        check(sum("args:" in message for _, message in logs) == 1, "once")
        check((2, "build_runner p_on: a warning") in logs, "stderr is logged as a warning")
        check((3, "build_runner p_on: [INFO] Succeeded after 0.1s with 0 outputs (0 actions)") in logs, "stdout lines are logged")
        check(shown == [
            (3, "build_runner p_on: [INFO] Succeeded after 0.1s with 0 outputs (0 actions)"),
            (1, "build_runner p_on: [SEVERE] Failed after 0.2s"),
        ], "Succeeded is shown as information, Failed as an error")
        deadline = time.monotonic() + 30
        while not (beats["on"].exists() and beats["on"].stat().st_size) and time.monotonic() < deadline:
            time.sleep(0.1)
        check(beats["on"].exists() and beats["on"].stat().st_size > 0, "build_runner is running")
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
        same = lambda: all((runner / n).read_text() == (TOOL / n).read_text() for n in ("flutter_run.dart", "dart_run.dart", "devices.dart", "flutter_devtools.dart", "devtools.dart"))
        check(same(), "the runner is copied to ~/.zed-flutter/")
        check(sorted(p.relative_to(app) for p in app.rglob("*")) == before, "nothing is written into the project")

        (runner / "flutter_run.dart").write_text("// an older version\n")
        start(home, app)
        check(same(), "a runner that differs is replaced")

        check_completions(home, app)
        check_build_runner_projects(tmp)
        check_build_runner(home, tmp)

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
