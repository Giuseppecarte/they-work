#!/usr/bin/env python3
"""Offline JSONL provider fixture. Never invokes a model or executes a tool."""
import json
import os
import sys

log = sys.argv[1]
count = 0
turn = 0
if os.path.exists(log):
    with open(log) as previous:
        for record in previous:
            method = json.loads(record).get("method")
            count += method == "thread/start"
            turn += method == "turn/start"


def send(value):
    print(json.dumps(value), flush=True)


def event(method, params):
    send({"method": method, "params": params})


for line in sys.stdin:
    message = json.loads(line)
    with open(log, "a") as output:
        output.write(json.dumps(message) + "\n")
    method = message.get("method")
    params = message.get("params", {})
    request_id = message.get("id")
    if method == "initialize":
        send({"id": request_id, "result": {"userAgent": "offline-fixture/1", "home": os.environ.get("CODEX_HOME")}})
    elif method == "thread/start":
        count += 1
        send({"id": request_id, "result": {"thread": {"id": f"managed-{count}", "cwd": params["cwd"]}}})
    elif method == "thread/resume":
        send({"id": request_id, "result": {"thread": {"id": params["threadId"]}}})
    elif method == "turn/start":
        turn += 1
        native_turn = f"turn-{turn}"
        thread = params["threadId"]
        prompt = params["input"][0]["text"]
        if prompt != "uncertain":
            send({"id": request_id, "result": {"turn": {"id": native_turn, "status": "inProgress"}}})
        event("turn/started", {"threadId": thread, "turn": {"id": native_turn}})
        event("item/agentMessage/delta", {"threadId": thread, "turnId": native_turn, "itemId": "message-1", "delta": "Fixture is working"})
        if prompt == "approval":
            send({"id": "approve-1", "method": "item/commandExecution/requestApproval", "params": {"threadId": thread, "turnId": native_turn, "itemId": "command-1", "command": "test-only-command", "availableDecisions": ["accept", "decline", "cancel"]}})
        if prompt == "team":
            for index in range(2):
                child = f"child-{index}"
                event("item/completed", {"threadId": thread, "turnId": native_turn, "item": {"id": f"spawn-{index}", "type": "collabToolCall", "tool": "spawnAgent", "status": "completed", "senderThreadId": thread, "newThreadId": child, "prompt": f"Task {index}"}})
                event("turn/started", {"threadId": child, "turn": {"id": f"child-turn-{index}"}})
                event("item/completed", {"threadId": child, "turnId": f"child-turn-{index}", "item": {"type": "agentMessage", "id": f"result-{index}", "phase": "final_answer", "text": f"Delivered result {index}"}})
                event("turn/completed", {"threadId": child, "turn": {"id": f"child-turn-{index}", "status": "completed"}})
        if prompt == "complete":
            event("turn/completed", {"threadId": thread, "turn": {"id": native_turn, "status": "completed"}})
        if prompt == "disconnect":
            break
    elif method == "turn/steer":
        send({"id": request_id, "result": {"turnId": params["expectedTurnId"]}})
    elif method == "turn/interrupt":
        send({"id": request_id, "result": {}})
        event("turn/completed", {"threadId": params["threadId"], "turn": {"id": params["turnId"], "status": "interrupted"}})
    elif method is None and request_id == "approve-1":
        event("serverRequest/resolved", {"threadId": "managed-1", "requestId": "approve-1"})
    elif method == "initialized":
        pass
    elif request_id is not None:
        send({"id": request_id, "error": {"code": -32601, "message": "Unsupported fixture request"}})
