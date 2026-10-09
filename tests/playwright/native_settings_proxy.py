#!/usr/bin/env python3
"""Delay real status responses during local browser acceptance; never invent replies.

Use only with a private native acceptance instance. A `hold-next` control file
contains a unique identifier. Its next status read is fetched from the actual
backend, then held until `release-<identifier>` exists. Logs contain timing,
revision observations and response digests, never bearer headers or request data.
"""

import argparse
import hashlib
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import re
import threading
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--instance", type=Path, required=True)
    parser.add_argument("--control", type=Path, required=True)
    args = parser.parse_args()
    instance = json.loads(args.instance.read_text())
    home = Path(instance["home"])
    if not home.name.startswith("brassclaw-browser-live-instance-"):
        raise ValueError("requires an isolated native browser acceptance home")
    for key in ("http_port", "proxy_port"):
        if type(instance[key]) is not int or not 0 < instance[key] < 65536:
            raise ValueError("invalid local listener port")
    args.control.mkdir(mode=0o700, parents=True, exist_ok=True)
    lock = threading.Lock()
    next_id = 0

    def event(kind, request_id, **fields):
        with lock:
            with (args.control / "events.jsonl").open("a") as output:
                output.write(json.dumps(dict(kind=kind, request_id=request_id,
                                             time=time.monotonic(), **fields)) + "\n")

    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, pattern, *values):
            # Keep HTTP diagnostics in the same fixture log, without headers.
            event("http_log", 0, message=pattern % values)

        def forward(self):
            nonlocal next_id
            with lock:
                next_id += 1
                request_id = next_id
                hold = None
                candidate = args.control / "hold-next"
                if self.path == "/api/settings/monty-vm/status" and candidate.exists():
                    hold = candidate.read_text().strip()
                    if not re.fullmatch(r"[a-zA-Z0-9_-]+", hold):
                        raise ValueError("invalid fixture hold identity")
                    candidate.rename(args.control / ("claimed-" + hold))
            event("started", request_id, method=self.command, path=self.path, hold=hold)
            backend = http.client.HTTPConnection("127.0.0.1", instance["http_port"], timeout=30)
            try:
                body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
                ignored = {"connection", "keep-alive", "proxy-connection", "transfer-encoding"}
                headers = {key: value for key, value in self.headers.items()
                           if key.lower() not in ignored}
                headers["Host"] = f"127.0.0.1:{instance['http_port']}"
                backend.request(self.command, self.path, body=body, headers=headers)
                response = backend.getresponse()
                data = response.read()
                fields = dict(status=response.status, sha256=hashlib.sha256(data).hexdigest())
                if self.path == "/api/settings/monty-vm/status" and response.status == 200:
                    value = json.loads(data)
                    execution = value.get("execution_limits") or {}
                    fields.update(state=value.get("state"),
                                  desired_revision=execution.get("desired_revision"),
                                  effective_revision=execution.get("effective_revision"),
                                  interval=(execution.get("limits") or {}).get("status_poll_interval_millis"))
                event("captured", request_id, hold=hold, **fields)
                if hold:
                    deadline = time.monotonic() + 30
                    while not (args.control / ("release-" + hold)).exists():
                        if time.monotonic() >= deadline:
                            event("hold_timeout", request_id, hold=hold)
                            raise TimeoutError("browser acceptance did not release its response")
                        time.sleep(0.01)
                self.send_response_only(response.status, response.reason)
                for key, value in response.getheaders():
                    if key.lower() not in ignored | {"content-length"}:
                        self.send_header(key, value)
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                if self.command != "HEAD":
                    self.wfile.write(data)
                    self.wfile.flush()
                event("sent", request_id, hold=hold, **fields)
            except (BrokenPipeError, ConnectionResetError):
                event("client_closed", request_id, hold=hold)
                self.close_connection = True
            except Exception as error:
                event("error", request_id, error=type(error).__name__)
                self.close_connection = True
                raise
            finally:
                backend.close()

        do_GET = forward
        do_HEAD = forward
        do_PUT = forward
        do_POST = forward
        do_PATCH = forward
        do_DELETE = forward
        do_OPTIONS = forward

    server = ThreadingHTTPServer(("127.0.0.1", instance["proxy_port"]), Handler)
    event("ready", 0, port=instance["proxy_port"])
    try:
        server.serve_forever(poll_interval=0.1)
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
