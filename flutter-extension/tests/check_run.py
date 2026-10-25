"""Tests tool/flutter_run.dart, the runner of the run tasks, against a stand-in for Flutter
(fake_flutter.py):

- it runs `flutter run -d <device>` on the device its target resolves to (starting an emulator
  first if need be), with the arguments after the target; an iOS simulator is shown in
  Simulator.app, even one that was booted without a window;
- it exits with Flutter's exit code; an unknown or impossible target exits with 1 and the reason;
- in a monorepo or a pub workspace it runs in the app's folder: a `workspace:` member, else the
  app under the current directory; of several apps the one holding ZED_FILE, else it says which;
- saving a `.dart` file under the app's lib/ hot reloads it (SIGUSR1 to `flutter run`, given a
  --pid-file): one reload for a burst of saves, none for other files, none when switched off,
  the user's own --pid-file kept (not on Windows).

    python3 tests/check_run.py        # needs `dart` in PATH
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUN = HERE.parent / "tool" / "flutter_run.dart"
FAKE = HERE / "fake_flutter.py"

failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


def run(tmp, args, state, cwd=None, extra_env=None):
    path = tmp / "state.json"
    path.write_text(json.dumps({"calls": [], "devices": [], "emulators": [], **state}))
    env = dict(
        os.environ,
        FAKE_FLUTTER_STATE=str(path),
        FLUTTER_TOOLS_FLUTTER=json.dumps([sys.executable, str(FAKE)]),
        FLUTTER_TOOLS_POLL_MS="100",
        FLUTTER_TOOLS_OPEN_SIMULATOR=json.dumps([sys.executable, str(FAKE), "open-simulator"]),
        # The checks of the arguments; hot reload on save has its own below.
        FLUTTER_TOOLS_HOT_RELOAD_ON_SAVE="off",
    )
    # Zed's own ZED_FILE, when the tests run in its terminal, would pick an app.
    env.pop("ZED_FILE", None)
    env.update(extra_env or {})
    result = subprocess.run(
        [shutil.which("dart"), str(RUN), *args], cwd=cwd or tmp, env=env, capture_output=True, text=True, timeout=120
    )
    return result, json.loads(path.read_text())


def check_hot_reload_on_save(package):
    import time

    print("== hot reload on save")

    def start(root, app, extra_env=None, args=()):
        path = root / "state.json"
        path.write_text(json.dumps({"calls": [], "devices": [], "emulators": [], "run_seconds": 4}))
        env = dict(
            os.environ,
            FAKE_FLUTTER_STATE=str(path),
            FLUTTER_TOOLS_FLUTTER=json.dumps([sys.executable, str(FAKE)]),
            FLUTTER_TOOLS_POLL_MS="100",
        )
        env.pop("ZED_FILE", None)
        env.pop("FLUTTER_TOOLS_HOT_RELOAD_ON_SAVE", None)
        env.update(extra_env or {})
        process = subprocess.Popen(
            [shutil.which("dart"), str(RUN), "web", *args], cwd=app, env=env,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        # Until flutter run (the fake) is up.
        deadline = time.time() + 60
        while time.time() < deadline and not any(c[:1] == ["run"] for c in json.loads(path.read_text())["calls"]):
            time.sleep(0.1)
        time.sleep(0.5)
        return process, path

    def finish(process, path):
        process.communicate(timeout=60)
        return json.loads(path.read_text())

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        app = package(root, "app")
        (app / "lib" / "src").mkdir()

        process, path = start(root, app)
        for _ in range(3):  # a burst of saves
            (app / "lib" / "src" / "home.dart").write_text(f"// {time.time()}\n")
            time.sleep(0.03)
        time.sleep(1)
        (app / "lib" / "notes.txt").write_text("not Dart\n")
        time.sleep(1)
        state = finish(process, path)
        run_call = next(c for c in state["calls"] if c[:1] == ["run"])
        check("--pid-file" in run_call, "flutter run gets a --pid-file")
        check(state.get("signals") == ["SIGUSR1"], "one hot reload for a burst of saves of Dart files, none for others")
        check(process.returncode == 0, "it still exits with Flutter's code")

        process, path = start(root, app, extra_env={"FLUTTER_TOOLS_HOT_RELOAD_ON_SAVE": "off"})
        (app / "lib" / "main.dart").write_text("void main() {}\n")
        time.sleep(1)
        state = finish(process, path)
        run_call = next(c for c in state["calls"] if c[:1] == ["run"])
        check("--pid-file" not in run_call and not state.get("signals"), "switched off: no pid file, no reload")

        own = root / "my.pid"
        process, path = start(root, app, args=("--pid-file", str(own)))
        (app / "lib" / "main.dart").write_text("void main() { }\n")
        time.sleep(1)
        state = finish(process, path)
        run_call = next(c for c in state["calls"] if c[:1] == ["run"])
        check(run_call.count("--pid-file") == 1 and str(own) in run_call, "the user's own --pid-file is kept")
        check(state.get("signals") == ["SIGUSR1"], "and used for the reload")


def main():
    pixel = {"id": "Pixel_10", "name": "Pixel 10", "device": "emulator-5554"}
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)

        print("== android, nothing running")
        result, state = run(tmp, ["android", "--flavor", "dev"], {"emulators": [pixel]})
        check(result.returncode == 0, "it succeeds")
        check(["emulators", "--launch", "Pixel_10"] in state["calls"], "the emulator is started")
        check(state["calls"][-1] == ["run", "-d", "emulator-5554", "--flavor", "dev"], "flutter run gets the device and the arguments")
        check("Starting the Android emulator Pixel 10" in result.stdout, "it says what is starting")

        print("== ios-simulator, one booted without a window")
        iphone = {"id": "D1677B21", "name": "iPhone 17 Pro", "targetPlatform": "ios", "emulator": True}
        result, state = run(tmp, ["ios-simulator"], {"devices": [iphone]})
        check(["open-simulator", "D1677B21"] in state["calls"], "Simulator.app is opened on it, so it shows")
        check(state["calls"][-1] == ["run", "-d", "D1677B21"], "flutter run runs on it")

        print("== Flutter's exit code")
        result, state = run(tmp, ["web"], {"run_exit": 3})
        check(state["calls"][-1] == ["run", "-d", "web-server"], "web without a browser runs on web-server")
        check(result.returncode == 3, "it exits with Flutter's code")

        print("== targets that cannot run")
        result, state = run(tmp, ["playstation"], {})
        check(result.returncode == 1 and "unknown target" in result.stderr, "an unknown target")
        result, state = run(tmp, ["android"], {})
        check(result.returncode == 1 and "no Android device" in result.stderr, "no Android device or emulator")
        check(not any(c[:1] == ["run"] for c in state["calls"]), "flutter run is not started")
        result = subprocess.run([shutil.which("dart"), str(RUN)], capture_output=True, text=True)
        check(result.returncode == 64 and "usage" in result.stderr, "no target: usage")

    print("== monorepos and pub workspaces")
    flutter_pubspec = "name: {}\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  flutter:\n    sdk: flutter\n"

    def package(root, path, flutter=True, main=True):
        directory = root / path
        (directory / "lib").mkdir(parents=True)
        name = Path(path).name
        (directory / "pubspec.yaml").write_text(flutter_pubspec.format(name) if flutter else f"name: {name}\n")
        if main:
            (directory / "lib" / "main.dart").write_text("void main() {}\n")
        return directory.resolve()

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "pubspec.yaml").write_text(
            "name: root\nworkspace:\n  - app\n  - core/ui_kit  # widgets\n  - 'core/common'\n"
        )
        app = package(root, "app")
        package(root, "core/ui_kit", main=False)
        package(root, "core/common", flutter=False)
        result, state = run(root, ["web"], {})
        check(result.returncode == 0, "a pub workspace runs")
        check(state.get("run_cwds") == [str(app)], "flutter run runs in the workspace member that is an app")
        result, state = run(root, ["web"], {}, cwd=app)
        check(state.get("run_cwds") == [str(app)], "from the app's own folder it runs there")

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        app = package(root, "apps/shop")
        package(root, "apps/shop/build/stale")  # tool output is not an app
        package(root, "packages/widgets", main=False)
        result, state = run(root, ["web"], {})
        check(state.get("run_cwds") == [str(app)], "without a workspace, the app under the folder")

        admin = package(root, "apps/admin")
        result, state = run(root, ["web"], {})
        check(result.returncode == 1 and "apps/admin" in result.stderr and "apps/shop" in result.stderr,
              "several apps: it names them")
        check(not state.get("run_cwds"), "and does not run")
        result, state = run(root, ["web"], {}, extra_env={"ZED_FILE": str(admin / "lib" / "main.dart")})
        check(state.get("run_cwds") == [str(admin)], "several apps: the one with the active file")

    if sys.platform != "win32":
        check_hot_reload_on_save(package)

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
