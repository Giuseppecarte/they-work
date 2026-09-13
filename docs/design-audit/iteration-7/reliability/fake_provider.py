#!/usr/bin/env python3
"""Offline protocol peer for fault timing. It never executes tools or models."""
import json
import os
from pathlib import Path
import sys
import time

folder = Path(sys.argv[1])
log = folder / "provider.jsonl"
prior = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
threads = sum(row.get("method") == "thread/start" for row in prior)
turns = sum(row.get("method") == "turn/start" for row in prior)


def send(value):
    print(json.dumps(value), flush=True)


def event(method, params):
    send({"method": method, "params": params})


for line in sys.stdin:
    message = json.loads(line)
    with log.open("a") as output:
        output.write(json.dumps(message) + "\n")
        output.flush()
        os.fsync(output.fileno())
    method, params, request_id = message.get("method"), message.get("params", {}), message.get("id")
    if method == "initialize":
        send({"id": request_id, "result": {"userAgent": "offline-reliability-fixture/1"}})
    elif method == "thread/start":
        threads += 1
        send({"id": request_id, "result": {"thread": {"id": f"managed-{threads}", "cwd": params["cwd"]}}})
    elif method == "thread/resume":
        send({"id": request_id, "result": {"thread": {"id": params["threadId"]}}})
    elif method == "turn/start":
        turns += 1
        thread, turn = params["threadId"], f"turn-{turns}"
        prompt = params["input"][0]["text"]
        if prompt == "gate-ack":
            (folder / "gate-ready").write_text("provider received the turn\n")
            deadline = time.monotonic() + 15
            while not (folder / "gate-release").exists() and time.monotonic() < deadline:
                time.sleep(.01)
            send({"id": request_id, "result": {"turn": {"id": turn, "status": "inProgress"}}})
            # No notification follows: this isolates the final receipt write.
            continue
        if prompt != "uncertain":
            send({"id": request_id, "result": {"turn": {"id": turn, "status": "inProgress"}}})
        event("turn/started", {"threadId": thread, "turn": {"id": turn}})
        if prompt == "approval":
            send({"id": "approval-1", "method": "item/commandExecution/requestApproval", "params": {
                "threadId": thread, "turnId": turn, "itemId": "fake-command", "command": "not executed",
                "availableDecisions": ["accept", "decline", "cancel"]}})
    elif method == "turn/steer":
        send({"id": request_id, "result": {"turnId": params["expectedTurnId"]}})
    elif method == "turn/interrupt":
        send({"id": request_id, "result": {}})
        event("turn/completed", {"threadId": params["threadId"], "turn": {"id": params["turnId"], "status": "interrupted"}})
    elif method is None and request_id == "approval-1":
        event("serverRequest/resolved", {"threadId": "managed-1", "requestId": request_id})
    elif method != "initialized" and request_id is not None:
        send({"id": request_id, "error": {"code": -32601, "message": "Unknown fixture method"}})
