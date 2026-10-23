"""Checks that the run configurations hang together, since nothing at run time would say so:

- languages/dart/tasks.json has a `Flutter: run on <name>` task for each target of
  tool/devices.dart, in its order, running the runner on that target, tagged `flutter-main` (the
  run button next to a Flutter app's main()); then a `Flutter DevTools: <name>` task for each page
  of tool/devtools.dart, opening it for the project; the Dart extension's own tasks come first;
- src/lib.rs's debug locator knows the same targets and the same runner, so each task is offered
  in the debugger too;
- languages/dart/config.toml names the Flutter adapter, without which Zed asks no locator.

    python3 tests/check_tasks.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


def main():
    devices = (ROOT / "tool" / "devices.dart").read_text()
    block = devices[devices.index("const targets = {"):]
    block = block[: block.index("};")]
    targets = re.findall(r"'([\w-]+)': '([^']+)'", block)
    check(len(targets) == 6, f"devices.dart has the six targets: {[t for t, _ in targets]}")

    tools = (ROOT / "tool" / "devtools.dart").read_text()
    block = tools[tools.index("const pages = {"):]
    block = block[: block.index("};")]
    pages = re.findall(r"'([\w-]+)': '([^']+)'", block)
    check(len(pages) == 6, f"devtools.dart has the six pages: {[p for p, _ in pages]}")

    tasks = json.loads((ROOT / "languages" / "dart" / "tasks.json").read_text())
    flutter = [t for t in tasks if t["label"].startswith("Flutter: ")]
    devtools = [t for t in tasks if t["label"].startswith("Flutter DevTools: ")]
    check(tasks[-len(flutter) - len(devtools):] == flutter + devtools, "the Dart extension's tasks come first")
    check([t["label"] for t in flutter] == [f"Flutter: run on {name}" for _, name in targets], "a run task per target, in order")
    for task, (target, _) in zip(flutter, targets):
        check(
            task["command"] == f'dart "$HOME/.zed-flutter/flutter_run.dart" {target}'
            and task["cwd"] == "$ZED_WORKTREE_ROOT"
            and "flutter-main" in task.get("tags", []),
            f"{task['label']} runs the runner on {target}, from the run button too",
        )

    check([t["label"] for t in devtools] == [f"Flutter DevTools: {name}" for _, name in pages], "a DevTools task per page, in order")
    for task, (page, _) in zip(devtools, pages):
        check(
            task["command"] == f'dart "$HOME/.zed-flutter/flutter_devtools.dart" {page} "$ZED_WORKTREE_ROOT"'
            and not task.get("tags"),
            f"{task['label']} opens {page} for the project",
        )

    plain = {t["label"]: t for t in tasks}
    for label, file in (("dart run: $ZED_RELATIVE_FILE", "$ZED_FILE"), ("dart run: bin/main.dart", "$ZED_WORKTREE_ROOT/bin/main.dart")):
        task = plain.get(label)
        check(
            task is not None
            and task["command"] == f'dart "$HOME/.zed-flutter/dart_run.dart" "{file}"'
            and "args" not in task
            and task["cwd"] == "$ZED_WORKTREE_ROOT",
            f"{label} builds first (dart_run.dart) before running {file}",
        )
    check("dart-main" in plain["dart run: $ZED_RELATIVE_FILE"].get("tags", []), "the first is on the run button next to main()")

    lsp = (ROOT / "tool" / "flutter_lsp.dart").read_text()
    installed = re.findall(r"'([\w.]+\.dart)'", re.search(r"const runnerFiles = \[([^\]]*)\]", lsp).group(1))
    check({"flutter_run.dart", "flutter_devtools.dart", "dart_run.dart"} <= set(installed), "the language server installs the programs of the tasks")
    for name in installed:
        imports = re.findall(r"^import '([\w.]+\.dart)';", (ROOT / "tool" / name).read_text(), re.M)
        check(set(imports) <= set(installed), f"and what {name} imports")

    lib = (ROOT / "src" / "lib.rs").read_text()
    for name in installed:
        check(f'include_str!("../tool/{name}")' in lib, f"the extension writes {name} into its work directory")
    lib_targets = re.findall(r'"([\w-]+)"', re.search(r"const TARGETS: &\[&str\] = &\[([^\]]*)\]", lib).group(1))
    check(lib_targets == [t for t, _ in targets], "the debug locator knows the same targets")
    check('const RUNNER: &str = "flutter_run.dart";' in lib, "and the same runner")

    config = (ROOT / "languages" / "dart" / "config.toml").read_text()
    check(re.search(r'^debuggers = \["Flutter", "Dart"\]$', config, re.M) is not None, "the Dart language names the Flutter adapter")

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
