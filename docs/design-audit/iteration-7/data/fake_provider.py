#!/usr/bin/env python3
"""JSONL fixture only. Emits supplied observations; never runs tools or models."""
import json
from pathlib import Path
import sys
import time

root = Path(sys.argv[1])


def send(message):
    print(json.dumps(message), flush=True)


for line in sys.stdin:
    message = json.loads(line)
    with (root / "requests.jsonl").open("a") as log:
        log.write(json.dumps(message) + "\n")
    method = message.get("method")
    request_id = message.get("id")
    if method == "initialize":
        send({"id": request_id, "result": {"userAgent": "offline-data-audit/1"}})
    elif method == "thread/start":
        send({"id": request_id, "result": {"thread": {
            "id": "parent", "cwd": message["params"]["cwd"]}}})
    elif method == "turn/start":
        send({"id": request_id, "result": {"turn": {
            "id": "audit-turn", "status": "inProgress"}}})
        send({"method": "turn/started", "params": {
            "threadId": "parent", "turn": {"id": "audit-turn"}}})
        (root / "ready").write_text("ready\n")
        deadline = time.monotonic() + 20
        while not (root / "release").exists():
            if time.monotonic() > deadline:
                raise RuntimeError("audit runner did not release fixture")
            time.sleep(0.01)
        count = 0
        with (root / "scenario.jsonl").open() as source, (root / "emitted.jsonl").open("w") as log:
            for record in source:
                event = json.loads(record)
                send(event)
                log.write(json.dumps(event) + "\n")
                count += 1
        (root / "emitted-count").write_text(str(count))
    elif method == "initialized":
        pass
    elif request_id is not None:
        send({"id": request_id, "error": {
            "code": -32601, "message": "Fixture does not implement this request"}})
