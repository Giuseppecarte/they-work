#!/usr/bin/env python3
"""Offline bounded-stream fixture: logs requests and emits no real tool work."""
import json
import pathlib
import sys
import time

root = pathlib.Path(sys.argv[1])

def send(value):
    print(json.dumps(value), flush=True)

for line in sys.stdin:
    message = json.loads(line)
    with (root / 'requests.jsonl').open('a') as output:
        output.write(json.dumps(message) + '\n')
    method = message.get('method')
    params = message.get('params', {})
    if method == 'initialize':
        send({'id': message['id'], 'result': {'userAgent': 'offline-stream-fixture/1'}})
    elif method == 'thread/start':
        send({'id': message['id'], 'result': {'thread': {'id': 'managed-1', 'cwd': params['cwd']}}})
    elif method == 'turn/start':
        count = int(params['input'][0]['text'])
        send({'id': message['id'], 'result': {'turn': {'id': 'turn-1', 'status': 'inProgress'}}})
        send({'method': 'turn/started', 'params': {'threadId': 'managed-1', 'turn': {'id': 'turn-1'}}})
        deadline = time.monotonic() + 10
        while not (root / 'burst.go').exists():
            if time.monotonic() > deadline:
                raise RuntimeError('Fixture gate not released')
            time.sleep(0.01)
        for index in range(count):
            if index == count - 1:
                send({'id': 'approval-1', 'method': 'item/commandExecution/requestApproval', 'params': {'threadId': 'managed-1', 'turnId': 'turn-1', 'itemId': 'exact-command', 'command': 'simulated command only', 'availableDecisions': ['accept', 'decline']}})
                continue
            actor = 'child' if index == 1 else 'managed-1'
            if index == 0:
                item = {'id': 'spawn-child', 'type': 'collabToolCall', 'tool': 'spawnAgent', 'senderThreadId': 'managed-1', 'newThreadId': 'child', 'status': 'completed'}
            elif index in (1, 2):
                item = {'id': f'result-{index}', 'type': 'agentMessage', 'phase': 'final_answer', 'text': f'Synthetic result {index}'}
            else:
                item = {'id': f'noise-{index}', 'type': 'reasoning'}
            send({'method': 'item/completed', 'params': {'threadId': actor, 'turnId': 'turn-1', 'item': item}})
    elif 'id' in message:
        send({'id': message['id'], 'error': {'code': -32601, 'message': 'Fixture rejects unintended operation'}})
