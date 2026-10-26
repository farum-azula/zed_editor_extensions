"""Runs a real Dart program through tool/dart_run.dart (the runner of the `dart run: <file>` tasks)
and checks that saving a change hot reloads it: a small HTTP server answers with the new code,
without a restart (its start counter stays at 1), and Ctrl+C (SIGINT) stops it.

    python3 tests/check_dart_hot_reload.py        # needs `dart` in PATH
"""

import os
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUN = HERE.parent / "tool" / "dart_run.dart"

PROGRAM = """\
import 'dart:io';

var starts = 0;

String answer() => 'v1';

Future<void> main(List<String> args) async {
  starts++;
  final server = await HttpServer.bind('127.0.0.1', int.parse(args.first));
  await for (final request in server) {
    request.response.write('${answer()} $starts');
    await request.response.close();
  }
}
"""

failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def get(port, timeout=30.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=2) as r:
                return r.read().decode()
        except OSError:
            time.sleep(0.2)
    return None


def main():
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        (tmp / "bin").mkdir()
        (tmp / "pubspec.yaml").write_text("name: reload_check\nenvironment:\n  sdk: ^3.0.0\n")
        main_file = tmp / "bin" / "main.dart"
        main_file.write_text(PROGRAM)
        subprocess.run(["dart", "pub", "get", "--offline"], cwd=tmp, capture_output=True)
        port = free_port()
        process = subprocess.Popen(
            ["dart", str(RUN), str(main_file), str(port)],
            cwd=tmp, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
            env=dict(os.environ),
        )
        try:
            check(get(port) == "v1 1", "the program runs")
            time.sleep(0.5)
            main_file.write_text(PROGRAM.replace("'v1'", "'v2'"))
            deadline = time.time() + 20
            answer = None
            while time.time() < deadline:
                answer = get(port)
                if answer == "v2 1":
                    break
                time.sleep(0.3)
            check(answer == "v2 1", f"a save hot reloads it, without a restart (got {answer!r})")
            process.send_signal(signal.SIGINT)
            try:
                process.wait(timeout=20)
                stopped = True
            except subprocess.TimeoutExpired:
                stopped = False
            check(stopped, "SIGINT stops it")
            check(get(port, timeout=2) is None, "and its server is gone")
            output = process.stdout.read() if process.stdout else ""
            check("Reloaded in" in output, "the terminal shows the reload")
        finally:
            if process.poll() is None:
                process.kill()
    print(f"{len(failures)} check(s) failed" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
