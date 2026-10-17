"""Checks that the run configurations hang together, since nothing at run time would say so:

- languages/dart/tasks.json has a `Flutter: run on <name>` task for each target of
  tool/devices.dart, in its order, running the runner on that target, tagged `flutter-main` (the
  run button next to a Flutter app's main()); the Dart extension's own tasks come first;
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

    tasks = json.loads((ROOT / "languages" / "dart" / "tasks.json").read_text())
    flutter = [t for t in tasks if t["label"].startswith("Flutter: ")]
    check(tasks[-len(flutter):] == flutter, "the Dart extension's tasks come first")
    check([t["label"] for t in flutter] == [f"Flutter: run on {name}" for _, name in targets], "a run task per target, in order")
    for task, (target, _) in zip(flutter, targets):
        check(
            task["command"] == f'dart "$HOME/.zed-flutter/flutter_run.dart" {target}'
            and task["cwd"] == "$ZED_WORKTREE_ROOT"
            and "flutter-main" in task.get("tags", []),
            f"{task['label']} runs the runner on {target}, from the run button too",
        )

    lib = (ROOT / "src" / "lib.rs").read_text()
    lib_targets = re.findall(r'"([\w-]+)"', re.search(r"const TARGETS: &\[&str\] = &\[([^\]]*)\]", lib).group(1))
    check(lib_targets == [t for t, _ in targets], "the debug locator knows the same targets")
    check('const RUNNER: &str = "flutter_run.dart";' in lib, "and the same runner")

    config = (ROOT / "languages" / "dart" / "config.toml").read_text()
    check(re.search(r'^debuggers = \["Flutter", "Dart"\]$', config, re.M) is not None, "the Dart language names the Flutter adapter")

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
