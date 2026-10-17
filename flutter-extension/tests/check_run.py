"""Tests tool/flutter_run.dart, the runner of the run tasks, against a stand-in for Flutter
(fake_flutter.py):

- it runs `flutter run -d <device>` on the device its target resolves to (starting an emulator
  first if need be), with the arguments after the target; an iOS simulator is shown in
  Simulator.app, even one that was booted without a window;
- it exits with Flutter's exit code; an unknown or impossible target exits with 1 and the reason.

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


def run(tmp, args, state):
    path = tmp / "state.json"
    path.write_text(json.dumps({"calls": [], "devices": [], "emulators": [], **state}))
    env = dict(
        os.environ,
        FAKE_FLUTTER_STATE=str(path),
        FLUTTER_TOOLS_FLUTTER=json.dumps([sys.executable, str(FAKE)]),
        FLUTTER_TOOLS_POLL_MS="100",
        FLUTTER_TOOLS_OPEN_SIMULATOR=json.dumps([sys.executable, str(FAKE), "open-simulator"]),
    )
    result = subprocess.run(
        [shutil.which("dart"), str(RUN), *args], cwd=tmp, env=env, capture_output=True, text=True, timeout=120
    )
    return result, json.loads(path.read_text())


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

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
