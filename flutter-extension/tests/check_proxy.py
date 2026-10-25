"""Tests tool/flutter_proxy.dart against a stand-in for Flutter (fake_flutter.py), so without
Flutter or a device:

- the editor's requests reach the adapter, and the adapter's messages reach the editor, unchanged;
- once the app has started, saving a .dart file under lib/ sends the adapter one `hotReload`;
  saving several at once sends one, a change to another kind of file none;
- the response to that request does not reach the editor, which never sent it;
- nothing is reloaded before the app starts, or with `"hotReloadOnSave": false`;
- a launch with a `target` runs on a device of that kind: a running one, or an emulator it starts
  first, while what the editor sends meanwhile waits; a device named in `toolArgs` wins; a target
  that cannot run here fails the launch with the reason;
- the editor's Stop ends a launched app: a `disconnect` goes to Flutter after a `terminate`
  (Flutter's own disconnect would leave a desktop app running), unless the editor sent one, the
  app was attached to, or `terminateDebuggee` is false;
- a launch whose `cwd` is a monorepo or a pub workspace goes to Flutter with the app's folder as
  `cwd`, and saving under that app's lib/ reloads it; of several apps, none: the launch fails.

    python3 tests/check_proxy.py        # needs `dart` in PATH
"""

import json
import os
import platform
import shutil
import sys
import tempfile
import time
from pathlib import Path

from dap import Dap

HERE = Path(__file__).resolve().parent
PROXY = HERE.parent / "tool" / "flutter_proxy.dart"
FAKE = HERE / "fake_flutter.py"

failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


def received(dap):
    """The commands the fake adapter reports having received, in order."""
    return [
        m["body"]["output"].split(" ", 1)[1].strip()
        for m in dap.seen
        if m.get("event") == "output" and m["body"]["output"].startswith("received ")
    ]


def start(state=None):
    env = dict(os.environ, FLUTTER_TOOLS_POLL_MS="100")
    if state is not None:
        env["FAKE_FLUTTER_STATE"] = str(state)
    return Dap([shutil.which("dart"), str(PROXY), sys.executable, str(FAKE), "debug_adapter"], env=env)


def session(project, **launch):
    dap = start()
    seq = dap.request("initialize", {"adapterID": "Flutter"})
    response = dap.response(seq)
    check(response["success"] and response["body"]["fake"], "initialize reaches the adapter and its response the editor")
    seq = dap.request("launch", {"cwd": str(project), "program": "lib/main.dart", **launch})
    dap.response(seq)
    return dap


def save(path, text="// changed\n"):
    path.write_text(path.read_text() + text if path.exists() else text)


def main():
    with tempfile.TemporaryDirectory() as tmp:
        project = Path(tmp)
        (project / "lib" / "src").mkdir(parents=True)
        main_dart = project / "lib" / "main.dart"
        main_dart.write_text("void main() {}\n")

        print("== before the app starts")
        dap = session(project)
        save(main_dart)
        dap.drain(1)
        check("hotReload" not in received(dap), "no reload before flutter.appStarted")

        print("== after it starts")
        dap.request("startApp")  # the fake adapter answers with flutter.appStarted
        dap.event("flutter.appStarted")
        time.sleep(0.5)  # the watcher starts on that event
        save(main_dart)
        dap.output("received hotReload")
        own = [m for m in dap.seen if m.get("type") == "response" and m.get("command") == "hotReload"]
        dap.drain(1)
        own += [m for m in dap.seen if m.get("type") == "response" and m.get("command") == "hotReload"]
        check(not own, "the response to the proxy's hotReload does not reach the editor")

        print("== several files at once")
        before = received(dap).count("hotReload")
        for name in ("a.dart", "b.dart", "src/c.dart"):
            save(project / "lib" / name)
        dap.drain(1.5)
        check(received(dap).count("hotReload") - before == 1, "saving three files reloads once")

        print("== another kind of file")
        before = received(dap).count("hotReload")
        save(project / "lib" / "notes.txt")
        save(project / "README.md")
        dap.drain(1)
        check(received(dap).count("hotReload") == before, "a .txt or a file outside lib/ does not reload")

        print("== the editor's own requests")
        seq = dap.request("restart")
        response = dap.response(seq)
        check(response["success"] and response["command"] == "restart", "restart passes through to the adapter")
        check(received(dap)[-1] == "restart", "the adapter gets restart itself")

        print("== a failed reload")
        dap.response(dap.request("failNextReload"))
        save(project / "lib" / "fail.dart")
        dap.output("Hot reload failed: no app")
        check(True, "a failed reload is reported in the console")

        seq = dap.request("disconnect")
        dap.response(seq)
        dap.close()
        check(dap.process.returncode == 0, "the proxy exits with the adapter")

        print("== hotReloadOnSave: false")
        dap = session(project, hotReloadOnSave=False)
        dap.request("startApp")
        dap.event("flutter.appStarted")
        time.sleep(0.5)
        save(main_dart)
        dap.drain(1)
        check("hotReload" not in received(dap), "no reload when turned off")
        dap.close()

        check_stop(project)
        check_targets(Path(tmp))
    check_workspace()

    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


def check_stop(project):
    print("== Stop: disconnect alone")
    dap = session(project)
    dap.request("startApp")
    dap.event("flutter.appStarted")
    seq = dap.request("disconnect", {"terminateDebuggee": True})
    response = dap.response(seq)
    order = received(dap)
    check(response["success"], "the disconnect is answered")
    check("terminate" in order and order.index("terminate") < order.index("disconnect"),
          "the app is terminated first (Flutter's disconnect leaves a desktop app running)")
    check(not any(m.get("type") == "response" and m.get("command") == "terminate" for m in dap.seen),
          "the editor does not see the response to that terminate")
    dap.close()

    print("== Stop: terminate, then disconnect")
    dap = session(project)
    dap.response(dap.request("terminate"))
    dap.response(dap.request("disconnect"))
    check(received(dap).count("terminate") == 1, "no second terminate")
    dap.close()

    print("== disconnect without ending the app")
    dap = session(project)
    dap.response(dap.request("disconnect", {"terminateDebuggee": False}))
    check("terminate" not in received(dap), "terminateDebuggee: false leaves the app alone")
    dap.close()
    dap = start()
    dap.response(dap.request("initialize", {"adapterID": "Flutter"}))
    dap.response(dap.request("attach", {"cwd": str(project)}))
    dap.response(dap.request("disconnect"))
    check("terminate" not in received(dap), "so does an attached session")
    dap.close()


def launch_on(tmp, target, devices=(), emulators=(), tool_args=None):
    """Launches with `target`; returns the session, the launch response and the fake's state."""
    state = tmp / "state.json"
    state.write_text(json.dumps({"devices": list(devices), "emulators": list(emulators), "calls": []}))
    dap = start(state)
    dap.response(dap.request("initialize", {"adapterID": "Flutter"}))
    arguments = {"cwd": str(tmp), "program": "lib/main.dart", "target": target}
    if tool_args is not None:
        arguments["toolArgs"] = tool_args
    launch = dap.request("launch", arguments)
    # Sent while the device is being found: it must reach Flutter after the launch.
    dap.request("configurationDone")
    response = dap.response(launch, timeout=60)
    dap.drain(0.5)
    return dap, response, json.loads(state.read_text())


def tool_args(dap):
    for m in dap.seen:
        output = m.get("body", {}).get("output", "") if m.get("event") == "output" else ""
        if output.startswith("launch "):
            return json.loads(output[len("launch "):]).get("toolArgs")
    return None


def launched(dap):
    """The arguments of the launch the fake adapter received."""
    for m in dap.seen:
        output = m.get("body", {}).get("output", "") if m.get("event") == "output" else ""
        if output.startswith("launch "):
            return json.loads(output[len("launch "):])
    return None


def check_workspace():
    flutter_pubspec = "name: {}\ndependencies:\n  flutter:\n    sdk: flutter\n"
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp).resolve()
        (root / "pubspec.yaml").write_text("name: root\nworkspace:\n  - app\n  - core/ui\n")
        for path, main in (("app", True), ("core/ui", False)):
            (root / path / "lib").mkdir(parents=True)
            (root / path / "pubspec.yaml").write_text(flutter_pubspec.format(Path(path).name))
            if main:
                (root / path / "lib" / "main.dart").write_text("void main() {}\n")

        print("== a pub workspace")
        dap = session(root)
        check(launched(dap)["cwd"] == str(root / "app"), "Flutter launches in the app's folder")
        dap.request("startApp")
        dap.event("flutter.appStarted")
        time.sleep(0.5)
        save(root / "app" / "lib" / "main.dart")
        dap.drain(1)
        check("hotReload" in received(dap), "saving under the app's lib/ reloads")
        dap.close()

        print("== several apps")
        (root / "core" / "ui" / "lib" / "main.dart").write_text("void main() {}\n")
        dap = start()
        dap.response(dap.request("initialize", {"adapterID": "Flutter"}))
        response = dap.response(dap.request("launch", {"cwd": str(root), "program": "lib/main.dart"}))
        check(not response["success"] and "several Flutter apps" in response["message"], "the launch fails, naming them")
        dap.close()


def check_targets(tmp):
    pixel = {"id": "Pixel_10", "name": "Pixel 10", "device": "emulator-5554"}
    galaxy = {"id": "R5CX", "name": "Galaxy", "targetPlatform": "android-arm64", "emulator": False}

    print("== target: android, nothing running")
    dap, response, state = launch_on(tmp, "android", emulators=[pixel])
    check(response["success"], "the launch succeeds")
    check(["emulators", "--launch", "Pixel_10"] in state["calls"], "the emulator is started")
    check(tool_args(dap) == ["-d", "emulator-5554"], "Flutter launches on it")
    order = received(dap)
    check(order.index("launch") < order.index("configurationDone"), "what the editor sent meanwhile follows the launch")
    check(any("Starting the Android emulator Pixel 10" in m.get("body", {}).get("output", "") for m in dap.seen),
          "the console says what is starting")
    dap.close()

    print("== target: android, a phone connected")
    dap, response, state = launch_on(tmp, "android", devices=[galaxy], emulators=[pixel])
    check(tool_args(dap) == ["-d", "R5CX"], "the running device is used")
    check(not any(c[:1] == ["emulators"] for c in state["calls"]), "no emulator is started")
    dap.close()

    print("== a device in toolArgs")
    dap, response, state = launch_on(tmp, "android", tool_args=["-d", "chrome"])
    check(tool_args(dap) == ["-d", "chrome"], "the named device wins")
    check(not any(c[:1] == ["devices"] for c in state["calls"]), "no device is looked for")
    dap.close()

    print("== target: web")
    chrome = {"id": "chrome", "name": "Chrome", "targetPlatform": "web-javascript", "emulator": False}
    dap, response, state = launch_on(tmp, "web", devices=[chrome])
    check(tool_args(dap) == ["-d", "chrome"], "Chrome when it is there")
    dap.close()
    dap, response, state = launch_on(tmp, "web")
    check(tool_args(dap) == ["-d", "web-server"], "web-server without a browser")
    dap.close()

    print("== desktop targets")
    host = {"Darwin": "macos", "Linux": "linux", "Windows": "windows"}[platform.system()]
    other = "linux" if host != "linux" else "windows"
    dap, response, state = launch_on(tmp, host)
    check(tool_args(dap) == ["-d", host], f"{host} runs on {host}")
    dap.close()
    dap, response, state = launch_on(tmp, other)
    check(not response["success"] and f"only on {other}" in response["message"], f"{other} fails here, saying why")
    dap.close()

    print("== targets that cannot run")
    dap, response, state = launch_on(tmp, "android")
    check(not response["success"] and "no Android device" in response["message"], "no Android device or emulator")
    dap.close()
    dap, response, state = launch_on(tmp, "playstation")
    check(not response["success"] and "unknown target" in response["message"], "an unknown target")
    dap.close()


if __name__ == "__main__":
    main()
