"""Tests Flutter DevTools support against stand-ins (fake_flutter.py for Flutter's debug adapter,
fake_devtools.py for DevTools, a browser and Zed's command line tool), so without Flutter:

- once the adapter reports the app's VM service, the proxy serves DevTools and writes a session
  file to ~/.zed-flutter/sessions/ with the project, the VM service and DevTools' address, and says
  where DevTools is in the console; `"devTools": false` turns that off;
- tool/flutter_devtools.dart opens the page it is given, connected to the app of the project, in
  a window; with no app running it says so and fails;
- a `navigate` tool event (a widget selected in the inspector) opens its file in Zed at its line
  and column, the resolved file when the event has one; a place that is not a file does nothing;
- when the app ends, DevTools stops and the session file goes; a session file whose DevTools no
  longer answers is deleted when the tasks look for an app;
- nothing is written into the project.

    python3 tests/check_devtools.py        # needs `dart` in PATH
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
from pathlib import Path

from dap import Dap

HERE = Path(__file__).resolve().parent
TOOL = HERE.parent / "tool"
FAKE_FLUTTER = HERE / "fake_flutter.py"
FAKE_DEVTOOLS = HERE / "fake_devtools.py"
VM = "ws://127.0.0.1:50000/abc=/ws"

failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


def wait_for(condition, timeout=15):
    deadline = time.time() + timeout
    while time.time() < deadline:
        if condition():
            return True
        time.sleep(0.1)
    return False


def records(path):
    return [json.loads(l) for l in path.read_text().splitlines()] if path.exists() else []


def answers(url):
    try:
        with urllib.request.urlopen(url, timeout=2):
            return True
    except OSError:
        return False


def main():
    dart = shutil.which("dart")
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        home, project = tmp / "home", tmp / "app"
        home.mkdir()
        (project / "lib").mkdir(parents=True)
        (project / "lib" / "main.dart").write_text("void main() {}\n")
        before = sorted(p.relative_to(project) for p in project.rglob("*"))
        browser, zed = tmp / "browser.txt", tmp / "zed.txt"
        sessions = home / ".zed-flutter" / "sessions"
        env = dict(
            os.environ,
            HOME=str(home),
            USERPROFILE=str(home),
            FLUTTER_TOOLS_DEVTOOLS=json.dumps([sys.executable, str(FAKE_DEVTOOLS), "server"]),
            FLUTTER_TOOLS_BROWSER=json.dumps([sys.executable, str(FAKE_DEVTOOLS), "record", str(browser)]),
            FLUTTER_TOOLS_ZED=json.dumps([sys.executable, str(FAKE_DEVTOOLS), "record", str(zed)]),
        )

        def devtools(*args):
            return subprocess.run(
                [dart, str(TOOL / "flutter_devtools.dart"), *args], env=env, capture_output=True, text=True, timeout=60
            )

        def session(**launch):
            dap = Dap([dart, str(TOOL / "flutter_proxy.dart"), sys.executable, str(FAKE_FLUTTER), "debug_adapter"], env=env)
            dap.response(dap.request("initialize", {"adapterID": "Flutter"}))
            dap.response(dap.request("launch", {"cwd": str(project), "program": "lib/main.dart", **launch}))
            dap.response(dap.request("startApp"))
            dap.response(dap.request("debuggerUris", {"vmServiceUri": VM}))
            return dap

        print("== the app starts")
        dap = session()
        dap.output("Flutter DevTools: ", timeout=60)
        check(True, "the console says where DevTools is")
        files = list(sessions.glob("*.json"))
        check(len(files) == 1, "a session file is written to ~/.zed-flutter/sessions/")
        info = json.loads(files[0].read_text()) if files else {}
        check(info.get("cwd") == str(project) and info.get("vmServiceUri") == VM, "with the project and the VM service")
        check(answers(info.get("devTools", "http://127.0.0.1:1/")), "DevTools is served")

        print("== a DevTools page")
        result = devtools("performance", str(project / "lib"))
        check(result.returncode == 0, "flutter_devtools.dart succeeds")
        expected = f"{info.get('devTools')}performance?uri={urllib.parse.quote(VM, safe='')}"
        check(wait_for(lambda: records(browser) == [[expected]]), "it opens the page, connected to the app, in a window")
        check(devtools("profiler").returncode == 64, "an unknown page is refused")

        print("== a widget selected")
        dap.request("navigate", {"fileUri": "package:app/main.dart", "resolvedFileUri": (project / "lib" / "main.dart").as_uri(), "line": 3, "column": 5})
        check(wait_for(lambda: records(zed) == [[f"{project / 'lib' / 'main.dart'}:3:5"]]), "its file opens in Zed at its line and column")
        dap.request("navigate", {"fileUri": "dart:core/object.dart", "line": 1, "column": 1})
        dap.drain(1)
        check(len(records(zed)) == 1, "a place that is not a file does nothing")

        print("== the app ends")
        dap.response(dap.request("disconnect"))
        dap.close()
        check(wait_for(lambda: not list(sessions.glob("*.json"))), "the session file goes")
        check(wait_for(lambda: not answers(info.get("devTools", "http://127.0.0.1:1/"))), "DevTools stops")
        result = devtools("inspector", str(project))
        check(result.returncode == 1 and "No Flutter app is running" in result.stderr, "with no app, the task says so")

        print("== a stale session file")
        sessions.mkdir(parents=True, exist_ok=True)
        stale = sessions / "1.json"
        stale.write_text(json.dumps({"cwd": str(project), "vmServiceUri": VM, "devTools": "http://127.0.0.1:9/"}))
        devtools("inspector", str(project))
        check(not stale.exists(), "it is deleted")

        print('== "devTools": false')
        dap = session(devTools=False)
        dap.drain(2)
        check(not list(sessions.glob("*.json")), "no DevTools, no session file")
        dap.response(dap.request("disconnect"))
        dap.close()

        check(sorted(p.relative_to(project) for p in project.rglob("*")) == before, "nothing is written into the project")

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
