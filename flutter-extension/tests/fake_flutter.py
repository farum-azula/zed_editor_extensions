"""A stand-in for the `flutter` command in the tests, without Flutter or devices. What it knows is
in the JSON file named by FAKE_FLUTTER_STATE: `devices` (as `flutter devices --machine` prints
them), `emulators` (ids and names), `run_exit` (the exit code of `run`); it appends what it is asked
to `calls`.

- `debug_adapter`: answers every request with success and reports each one as an output event
  (`received <command>`, `launch <arguments>` for a launch). Two requests of its own drive it:
  `startApp` makes it send the `flutter.appStarted` event the real adapter sends once the app runs,
  `failNextReload` makes it answer the next `hotReload` with an error.
- `devices --machine`, `emulators`, `emulators --launch <id>` (the emulator becomes a device),
  `run ...` (prints its arguments), `open-simulator <udid>` (stands in for opening Simulator.app).
"""

import json
import os
import sys

STATE = os.environ.get("FAKE_FLUTTER_STATE")


def load():
    if not STATE or not os.path.exists(STATE):
        return {"devices": [], "emulators": [], "calls": []}
    with open(STATE) as f:
        return json.load(f)


def save(state):
    if STATE:
        with open(STATE, "w") as f:
            json.dump(state, f)


args = sys.argv[1:]
state = load()
state.setdefault("calls", []).append(args)
save(state)

if args[:1] == ["open-simulator"]:
    sys.exit(0)
if args[:1] == ["devices"]:
    print("Checking for devices...")
    print(json.dumps(state["devices"]))
    sys.exit(0)
if args == ["emulators"]:
    for e in state["emulators"]:
        print(f"{e['id']} • {e['name']} • Google • android")
    sys.exit(0)
if args[:2] == ["emulators", "--launch"]:
    for e in state["emulators"]:
        if e["id"] == args[2]:
            state["devices"].append(
                {"id": e["device"], "name": e["name"], "targetPlatform": "android-arm64", "emulator": True}
            )
            save(state)
            sys.exit(0)
    sys.exit(1)
if args[:1] == ["run"]:
    print("run " + " ".join(args[1:]))
    sys.exit(state.get("run_exit", 0))
if args != ["debug_adapter"]:
    sys.exit(f"fake flutter: unknown command {args}")

seq = 0
fail_next_reload = False


def send(message):
    global seq
    seq += 1
    message["seq"] = seq
    body = json.dumps(message).encode()
    sys.stdout.buffer.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
    sys.stdout.buffer.flush()


def event(name, body=None):
    send({"type": "event", "event": name, "body": body or {}})


def read():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        line = line.decode().strip()
        if not line:
            break
        key, value = line.split(":", 1)
        headers[key.strip().lower()] = value.strip()
    return json.loads(sys.stdin.buffer.read(int(headers["content-length"])))


while (message := read()) is not None:
    command = message.get("command")
    event("output", {"category": "console", "output": f"received {command}\n"})
    if command == "launch":
        event("output", {"category": "console", "output": "launch " + json.dumps(message["arguments"]) + "\n"})
    response = {"type": "response", "request_seq": message["seq"], "command": command, "success": True}
    if command == "initialize":
        response["body"] = {"fake": True}
    if command == "failNextReload":
        fail_next_reload = True
    if command == "hotReload" and fail_next_reload:
        fail_next_reload = False
        response.update(success=False, message="no app")
    send(response)
    if command == "startApp":
        event("flutter.appStarted")
    if command == "disconnect":
        event("terminated")
        break
