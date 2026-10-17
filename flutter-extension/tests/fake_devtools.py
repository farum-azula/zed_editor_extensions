"""Stand-ins for the programs DevTools support starts, in the tests:

    python3 fake_devtools.py server           # `dart devtools --machine`: serves 200 on any path
                                              # and prints its server.started event
    python3 fake_devtools.py record <file> ...  # a browser or Zed's command line tool: appends
                                              # its arguments to <file>, a JSON list a line
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

if sys.argv[1] == "record":
    with open(sys.argv[2], "a") as f:
        f.write(json.dumps(sys.argv[3:]) + "\n")
    sys.exit(0)


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"devtools")

    def log_message(self, *args):
        pass


server = HTTPServer(("127.0.0.1", 0), Handler)
host, port = server.server_address
print(json.dumps({"event": "server.dtdStarted", "params": {"uri": "ws://127.0.0.1:1/x"}}), flush=True)
print(json.dumps({"event": "server.started", "params": {"host": host, "port": port, "pid": 0}}), flush=True)
server.serve_forever()
