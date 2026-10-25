"""Tests tool/dart_run.dart, the runner of the `dart run: <file>` tasks, with a fake `dart` first in
PATH that records the commands it gets:

- in a package with `build_runner` in `dev_dependencies` it builds first
  (`dart run build_runner build --delete-conflicting-outputs`), in the package directory, then runs
  the file with the arguments after it; a failed build stops with its exit code;
- without `build_runner` (or with it only in `dependencies`) it just runs the file;
- it exits with the program's exit code;
- in a Flutter package (`flutter: sdk: flutter` in `dependencies`) it runs the file with
  `flutter run -t <file> <arguments>` (after the build, if any), since the Dart VM cannot run it.

    python3 tests/check_dart_run.py        # needs `dart` in PATH
"""

import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUN = HERE.parent / "tool" / "dart_run.dart"

FAKE_DART = f"""#!{sys.executable}
import json, os, sys
with open(os.environ["FAKE_DART_LOG"], "a") as log:
    log.write(json.dumps({{"args": sys.argv[1:], "cwd": os.getcwd()}}) + "\\n")
build = sys.argv[1:3] == ["run", "build_runner"]
sys.exit(int(os.environ["FAKE_DART_BUILD_EXIT" if build else "FAKE_DART_RUN_EXIT"]))
"""

FAKE_FLUTTER = f"""#!{sys.executable}
import json, os, sys
with open(os.environ["FAKE_DART_LOG"], "a") as log:
    log.write(json.dumps({{"args": ["flutter", *sys.argv[1:]], "cwd": os.getcwd()}}) + "\\n")
sys.exit(int(os.environ["FAKE_DART_RUN_EXIT"]))
"""

failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


def package(root, name, pubspec):
    (root / name / "bin").mkdir(parents=True)
    (root / name / "pubspec.yaml").write_text(pubspec)
    main = root / name / "bin" / "main.dart"
    main.write_text("void main() {}\n")
    return main


def run(tmp, file, *args, build_exit=0, run_exit=0):
    log = tmp / "log.jsonl"
    log.write_text("")
    env = dict(
        os.environ,
        PATH=str(tmp / "fake") + os.pathsep + os.environ["PATH"],
        FAKE_DART_LOG=str(log),
        FAKE_DART_BUILD_EXIT=str(build_exit),
        FAKE_DART_RUN_EXIT=str(run_exit),
    )
    result = subprocess.run(
        [shutil.which("dart"), str(RUN), str(file), *args],
        cwd=tmp, env=env, capture_output=True, text=True, timeout=120,
    )
    calls = [json.loads(line) for line in log.read_text().splitlines()]
    return result, [(c["args"], Path(c["cwd"]).resolve()) for c in calls]


def main():
    build = ["run", "build_runner", "build", "--delete-conflicting-outputs"]
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp).resolve()
        (tmp / "fake").mkdir()
        fake = tmp / "fake" / "dart"
        fake.write_text(FAKE_DART)
        fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
        fake_flutter = tmp / "fake" / "flutter"
        fake_flutter.write_text(FAKE_FLUTTER)
        fake_flutter.chmod(fake_flutter.stat().st_mode | stat.S_IEXEC)

        generated = package(
            tmp, "generated",
            "name: generated\ndependencies:\n  json_annotation: ^4.0.0\n"
            "dev_dependencies:\n  # the generator\n  build_runner: ^2.4.0\n  test: ^1.0.0\n",
        )
        plain = package(tmp, "plain", "name: plain\ndev_dependencies:\n  test: ^1.0.0\n")
        runtime = package(tmp, "runtime", "name: runtime\ndependencies:\n  build_runner: ^2.4.0\n")
        flutter_app = package(
            tmp, "flutter_app",
            "name: flutter_app\ndependencies:\n  flutter:\n    sdk: flutter\n  spring_boot_starter_flutter: ^0.2.0\n"
            "dev_dependencies:\n  build_runner: ^2.4.0\n",
        )
        flutter_dev = package(
            tmp, "flutter_dev",
            "name: flutter_dev\ndependencies:\n  path: ^1.0.0\ndev_dependencies:\n  flutter_test:\n    sdk: flutter\n",
        )
        nested = tmp / "generated" / "bin" / "src"
        nested.mkdir()
        (nested / "tool.dart").write_text("void main() {}\n")

        print("== build_runner in dev_dependencies")
        result, calls = run(tmp, generated, "--verbose", "x")
        package_dir = tmp / "generated"
        check(result.returncode == 0, "it succeeds")
        check(calls == [(build, package_dir), (["run", str(generated), "--verbose", "x"], package_dir)],
              "it builds first, then runs the file with its arguments, both in the package directory")

        print("== a file below the package root")
        result, calls = run(tmp, nested / "tool.dart")
        check([c[0] for c in calls] == [build, ["run", str(nested / "tool.dart")]] and calls[0][1] == package_dir,
              "the nearest pubspec.yaml above the file is the package")

        print("== a failed build")
        result, calls = run(tmp, generated, build_exit=7)
        check(result.returncode == 7, "it exits with the build's exit code")
        check([c[0] for c in calls] == [build], "and the program is not run")

        print("== without build_runner")
        for name, main_file in (("plain", plain), ("runtime", runtime)):
            result, calls = run(tmp, main_file, "a")
            check(calls == [(["run", str(main_file), "a"], tmp / name)],
                  f"{name}: it just runs the file, in the package directory")

        print("== a Flutter package")
        result, calls = run(tmp, flutter_app, "--release", run_exit=5)
        app_dir = tmp / "flutter_app"
        check(calls == [(build, app_dir), (["flutter", "run", "-t", str(flutter_app), "--release"], app_dir)],
              "it builds first, then runs the file with flutter run -t, in the package directory")
        check(result.returncode == 5, "it exits with flutter's exit code")
        result, calls = run(tmp, flutter_dev)
        check(calls == [(["run", str(flutter_dev)], tmp / "flutter_dev")],
              "the Flutter SDK only in dev_dependencies: dart run")

        print("== the program's exit code")
        result, calls = run(tmp, generated, run_exit=3)
        check(result.returncode == 3, "after a build, it exits with the program's code")
        result, calls = run(tmp, plain, run_exit=4)
        check(result.returncode == 4, "without a build too")

        print("== no file")
        result = subprocess.run([shutil.which("dart"), str(RUN)], capture_output=True, text=True)
        check(result.returncode == 64 and "usage" in result.stderr, "no file: usage")

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
