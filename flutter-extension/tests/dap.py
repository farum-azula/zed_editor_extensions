"""A minimal Debug Adapter Protocol client for the tests: frames messages, reads them on a thread,
and waits for the ones a test expects."""

import json
import queue
import subprocess
import threading
import time


class Dap:
    def __init__(self, command, cwd=None, env=None):
        self.process = subprocess.Popen(
            command,
            cwd=cwd,
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.messages = queue.Queue()
        self.seen = []
        self.seq = 0
        threading.Thread(target=self._read, daemon=True).start()
        threading.Thread(target=self._drain_stderr, daemon=True).start()
        self.stderr = []

    def _read(self):
        out = self.process.stdout
        while True:
            headers = {}
            while True:
                line = out.readline()
                if not line:
                    self.messages.put(None)
                    return
                line = line.decode().strip()
                if not line:
                    break
                key, value = line.split(":", 1)
                headers[key.strip().lower()] = value.strip()
            self.messages.put(json.loads(out.read(int(headers["content-length"]))))

    def _drain_stderr(self):
        for line in self.process.stderr:
            self.stderr.append(line.decode(errors="replace"))

    def send(self, message):
        body = json.dumps(message).encode()
        self.process.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
        self.process.stdin.flush()

    def request(self, command, arguments=None):
        self.seq += 1
        self.send({"seq": self.seq, "type": "request", "command": command, "arguments": arguments or {}})
        return self.seq

    def wait(self, accept, timeout, what):
        """The first message `accept` takes; every message read is kept in `seen`."""
        deadline = time.monotonic() + timeout
        while True:
            left = deadline - time.monotonic()
            if left <= 0:
                raise AssertionError(f"timed out waiting for {what}")
            try:
                message = self.messages.get(timeout=left)
            except queue.Empty:
                raise AssertionError(f"timed out waiting for {what}") from None
            if message is None:
                raise AssertionError(f"the adapter exited while waiting for {what}: {''.join(self.stderr)}")
            self.seen.append(message)
            if accept(message):
                return message

    def response(self, seq, timeout=30):
        return self.wait(
            lambda m: m.get("type") == "response" and m.get("request_seq") == seq,
            timeout,
            f"the response to request {seq}",
        )

    def event(self, name, timeout=30, where=lambda m: True):
        return self.wait(
            lambda m: m.get("type") == "event" and m.get("event") == name and where(m),
            timeout,
            f"a {name} event",
        )

    def output(self, text, timeout=30):
        return self.event("output", timeout, lambda m: text in m.get("body", {}).get("output", ""))

    def drain(self, seconds):
        """Every message that arrives within `seconds`."""
        got = []
        deadline = time.monotonic() + seconds
        while (left := deadline - time.monotonic()) > 0:
            try:
                message = self.messages.get(timeout=left)
            except queue.Empty:
                break
            if message is None:
                break
            self.seen.append(message)
            got.append(message)
        return got

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                self.process.kill()
