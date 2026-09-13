#!/usr/bin/env python3
"""Offline native-console fixture, including read-only capability probes."""
import json
import os
import pathlib
import sys
import time

home = pathlib.Path(os.environ.get("CLAUDE_CONFIG_DIR", os.environ.get("CODEX_HOME", ".")))
args = sys.argv[1:]
if args == ["--version"]:
    print("native-fixture 2.1.248")
elif args == ["--help"]:
    print("--help --version" if (home / "legacy").exists() else "--help --version --bg")
elif args == ["agents", "--json"]:
    print((home / "roster.json").read_text() if (home / "roster.json").exists() else "[]")
elif args == ["wait"]:
    (home / "console.ready").write_text(str(os.getpid()))
    time.sleep(15)
else:
    (home / "received.json").write_text(json.dumps({"args": args, "cwd": os.getcwd()}))
