"""Runs a real Flutter app through proxy/flutter_proxy.dart and Flutter's own debug adapter, the way
Zed runs it, and checks that:

- saving a change under lib/ hot reloads the app: its new code runs, without restarting it;
- Restart (the DAP `restart` request Zed sends) hot restarts it: `main` runs again;
- the proxy serves Flutter DevTools for the app (the Dart SDK's own) and its pages load;
- selecting a widget in the inspector (select_widget.dart, as a click in DevTools' tree) opens its
  code in Zed, at the line and column that create it (a stand-in records what Zed is asked);
- Stop (a `disconnect`, as Zed sends) quits the app: its VM service goes away.

    python3 tests/check_hot_reload.py [--device <target or device id>]

Needs `dart` and `flutter` in PATH and a device to run on: by default the desktop of this OS. A run
target (ios-simulator, android, web, macos, windows, linux) is looked up, or started, by the proxy
as the generated debug scenarios are; anything else is taken as a device id.
The first build takes a few minutes.
"""

import argparse
import json
import os
import platform
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
from pathlib import Path

from dap import Dap

HERE = Path(__file__).resolve().parent
PROXY = HERE.parent / "tool" / "flutter_proxy.dart"
FAKE_DEVTOOLS = HERE / "fake_devtools.py"

MAIN = """\
import 'package:flutter/material.dart';

void main() {
  debugPrint('main ran');
  runApp(const App());
}

class App extends StatelessWidget {
  const App({super.key});

  @override
  Widget build(BuildContext context) {
    debugPrint('build: v1');
    return const MaterialApp(home: SizedBox());
  }
}
"""

failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what, flush=True)
    if not ok:
        failures.append(what)


def devtools_url(dap, timeout=120):
    """The address of DevTools' inspector the proxy reports in the console."""
    prefix = "Flutter DevTools: "
    for m in dap.seen:
        if m.get("event") == "output" and m["body"].get("output", "").startswith(prefix):
            return m["body"]["output"][len(prefix):].strip()
    return dap.output(prefix, timeout)["body"]["output"][len(prefix):].strip()


def loads(url):
    try:
        with urllib.request.urlopen(url, timeout=10) as response:
            return response.status == 200
    except OSError:
        return False


def listening(uri):
    """Whether something accepts connections at the host and port of [uri]."""
    parsed = urllib.parse.urlparse(uri)
    try:
        socket.create_connection((parsed.hostname, parsed.port), timeout=2).close()
        return True
    except OSError:
        return False


def outputs(dap, text):
    return sum(text in m.get("body", {}).get("output", "") for m in dap.seen if m.get("event") == "output")


def launch_device(device, desktop):
    """The launch arguments for --device: a run target goes through the proxy's device lookup, as
    the generated debug scenarios do; anything else is a device id."""
    if device in ("ios-simulator", "android", "web", desktop):
        return {"target": device}
    return {"toolArgs": ["-d", device]}


def main():
    desktop = {"Darwin": "macos", "Linux": "linux", "Windows": "windows"}[platform.system()]
    parser = argparse.ArgumentParser()
    parser.add_argument("--device", default=desktop)
    device = parser.parse_args().device
    dart, flutter = shutil.which("dart"), shutil.which("flutter")

    tmp = tempfile.mkdtemp()
    try:
        app = Path(tmp) / "reload_check"
        create = [flutter, "create", "--empty", "--project-name", "reload_check", str(app)]
        platforms = {"ios-simulator": "ios", "android": "android", "web": "web", desktop: desktop}
        if device in platforms:
            create[2:2] = [f"--platforms={platforms[device]}"]
        subprocess.run(create, check=True, capture_output=True)
        main_dart = app / "lib" / "main.dart"
        main_dart.write_text(MAIN)

        zed = Path(tmp) / "zed.txt"
        env = dict(os.environ, FLUTTER_TOOLS_ZED=json.dumps([sys.executable, str(FAKE_DEVTOOLS), "record", str(zed)]))
        dap = Dap([dart, str(PROXY), flutter, "debug_adapter"], cwd=app, env=env)
        stopped = False
        try:
            dap.response(dap.request("initialize", {"adapterID": "Flutter", "linesStartAt1": True}))
            launch = dap.request(
                "launch",
                {"program": "lib/main.dart", "cwd": str(app), **launch_device(device, desktop)},
            )
            dap.event("initialized", timeout=120)
            # Both wait while the proxy boots a simulator or an emulator for the target.
            dap.response(dap.request("configurationDone"), timeout=600)
            dap.response(launch, timeout=600)

            print(f"== building and starting on {device}", flush=True)
            dap.event("flutter.appStarted", timeout=1800)
            dap.output("build: v1", timeout=60)
            check(True, "the app runs under the proxy")

            print("== DevTools", flush=True)
            url = devtools_url(dap)
            check(loads(url), f"DevTools is served for the app: {url}")

            print("== a widget selected", flush=True)
            vm = urllib.parse.unquote(url.split("uri=", 1)[1])
            selected = subprocess.run(
                [dart, str(HERE / "select_widget.dart"), vm, "SizedBox"], capture_output=True, text=True, timeout=120
            )
            check(selected.returncode == 0, f"the inspector selects a widget {selected.stderr.strip()}")
            source = MAIN.splitlines()
            line = next(i for i, l in enumerate(source) if "SizedBox()" in l)
            expected = (os.path.realpath(main_dart), line + 1, source[line].index("SizedBox") + 1)
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline and not zed.exists():
                time.sleep(0.2)
            opened = json.loads(zed.read_text().splitlines()[0])[0] if zed.exists() else "nothing:0:0"
            path, at_line, at_column = opened.rsplit(":", 2)
            check(
                (os.path.realpath(path), int(at_line), int(at_column)) == expected,
                f"its code opens in Zed at the line and column that create it: {opened}",
            )

            print("== saving a change", flush=True)
            main_dart.write_text(MAIN.replace("build: v1", "build: v2"))
            dap.output("build: v2", timeout=120)
            check(True, "saving a change runs the new code (hot reload)")
            check(outputs(dap, "main ran") == 1, "the app was not restarted for it")

            print("== Restart", flush=True)
            restart = dap.response(dap.request("restart"), timeout=120)
            check(restart["success"], "restart succeeds")
            dap.wait(lambda m: outputs(dap, "main ran") >= 2, 120, "main to run again")
            check(True, "Restart runs main again (hot restart)")

            print("== Stop", flush=True)
            dap.response(dap.request("disconnect", {"terminateDebuggee": True}), timeout=60)
            stopped = True
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline and listening(vm):
                time.sleep(0.5)
            check(not listening(vm), "Stop quits the app")
        except AssertionError as error:
            check(False, str(error))
        finally:
            try:
                if not stopped:
                    dap.request("disconnect", {"terminateDebuggee": True})
                    dap.drain(15)
            finally:
                dap.close()
    finally:
        # On Windows the app may still be quitting when the checks end, and a running app's files
        # cannot be deleted: leave those to the OS rather than fail checks that passed.
        shutil.rmtree(tmp, ignore_errors=True)

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
