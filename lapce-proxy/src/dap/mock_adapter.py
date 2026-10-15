#!/usr/bin/env python3
"""Tiny DAP adapter used by lapce-proxy tests. Not a real debugger."""
import json
import sys

seq = 0
launch_seq = None
out = sys.stdout.buffer
inp = sys.stdin.buffer


def send(msg):
    global seq
    seq += 1
    msg["seq"] = seq
    body = json.dumps(msg).encode()
    out.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
    out.flush()


def respond(req, body=None, success=True, message=None):
    msg = {"type": "response", "request_seq": req["seq"], "command": req["command"], "success": success}
    if body is not None:
        msg["body"] = body
    if message:
        msg["message"] = message
    send(msg)


def event(name, body=None):
    msg = {"type": "event", "event": name}
    if body is not None:
        msg["body"] = body
    send(msg)


def read():
    length = None
    while True:
        line = inp.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            break
        k, v = line.decode().split(":", 1)
        if k.lower() == "content-length":
            length = int(v)
    return json.loads(inp.read(length))


while True:
    req = read()
    if req is None:
        break
    cmd = req["command"]
    args = req.get("arguments") or {}
    if cmd == "initialize":
        respond(req, {"supportsConfigurationDoneRequest": True, "supportsTerminateRequest": True})
    elif cmd == "launch":
        if args.get("program") == "fail":
            respond(req, success=False, message="boom")
            continue
        launch_seq = req
        if args.get("program") == "early":
            respond(req)
            launch_seq = None
        event("custom/unknownEvent", {"x": 1})
        event("initialized")
    elif cmd == "setBreakpoints":
        bps = args.get("breakpoints") or []
        respond(req, {"breakpoints": [{"verified": True, "line": b["line"]} for b in bps]})
    elif cmd == "configurationDone":
        respond(req)
        if launch_seq is not None:
            respond(launch_seq)
        event("output", {"category": "stdout", "output": "hello\n"})
        event("stopped", {"reason": "breakpoint", "threadId": 1, "allThreadsStopped": True})
    elif cmd == "threads":
        respond(req, {"threads": [{"id": 1, "name": "main"}]})
    elif cmd == "stackTrace":
        respond(req, {"stackFrames": [{"id": 10, "name": "f", "line": 3, "column": 1, "source": {"path": "/tmp/a.php"}}], "totalFrames": 1})
    elif cmd == "scopes":
        respond(req, {"scopes": [{"name": "Locals", "variablesReference": 100, "expensive": False}]})
    elif cmd == "variables":
        respond(req, {"variables": [{"name": "$a", "value": "1", "type": "int", "variablesReference": 0}]})
    elif cmd in ("continue", "next", "stepIn", "stepOut", "pause"):
        respond(req, {"allThreadsContinued": True} if cmd == "continue" else None)
        if cmd == "continue":
            event("continued", {"threadId": 1, "allThreadsContinued": True})
    elif cmd == "terminate":
        respond(req)
        event("terminated")
    elif cmd == "disconnect":
        respond(req)
        break
    else:
        respond(req, success=False, message="unsupported " + cmd)