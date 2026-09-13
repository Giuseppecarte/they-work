#!/usr/bin/env python3
"""Rehearse actual release installation and fake-provider faults, without accounts.

Production code is unmodified. Installer downloads are replaced at the curl
boundary with local files; the archive contains the actual native executable.
Control cases start that exact release's --control-host and use its JSON IPC.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import shutil
import signal
import socket
import subprocess
import sys
import tarfile
import time
import traceback

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
EXPECTED = {
    "install": "The real archived binary installs and runs; replacement is atomic; failed downloads, checksum or extraction preserve the prior binary.",
    "concurrent": "Eight concurrent copies of one operation execute exactly one native turn; changing its payload is rejected.",
    "control-actions": "Below the history bound, the same fixture approval can be declined and its owned active turn interrupted exactly once.",
    "uncertain": "A missing acknowledgement remains uncertain; restart and duplicate operation IDs never replay work.",
    "crash": "A crash after the provider receives a turn but before its acknowledgement recovers an uncertain receipt without replay.",
    "permission": "A pre-send storage failure sends nothing; after storage recovers the operation must not remain indefinitely Sending.",
    "quota": "A file-size quota failure sends nothing and does not falsely confirm execution; restart can recover the empty durable state.",
    "after-send": "A post-send durable-write failure is not replayed; restart exposes uncertainty for the already executed operation.",
    "ledger": "At the operation-history bound, new work may be refused, but an owned active turn must remain interruptible and its current request answerable.",
}


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def wait(check, label, seconds=5):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        value = check()
        if value:
            return value
        time.sleep(.02)
    raise RuntimeError("Timed out: " + label)


def check(label, condition, actual):
    return {"check": label, "status": "pass" if condition else "defect", "actual": actual}


def clean_env(folder):
    for name in ("home", "tmp", "xdg"):
        (folder / name).mkdir(parents=True, exist_ok=True)
    return {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": str(folder / "home"),
            "TMPDIR": str(folder / "tmp"), "XDG_CONFIG_HOME": str(folder / "xdg"),
            "LANG": "en_US.UTF-8", "TERM": "xterm-256color"}


class Host:
    def __init__(self, binary, folder, evidence, quota=None):
        self.binary, self.folder, self.evidence, self.quota = binary, folder, evidence, quota
        self.state_dir = folder / "control"
        self.state_dir.mkdir(parents=True, exist_ok=True)
        self.state_dir.chmod(0o700)
        self.env = clean_env(folder)
        self.source = folder / "codex"
        self.source.mkdir(exist_ok=True)
        self.project = folder / "project with spaces"
        self.project.mkdir(exist_ok=True)
        self.config_path = self.state_dir / "config.json"
        save(self.config_path, {"state_dir": str(self.state_dir), "codex_home": str(self.source),
             "codex_program": sys.executable, "codex_args": [str(HERE / "fake_provider.py"), str(folder)],
             "rpc_timeout_ms": 1500})
        self.config_path.chmod(0o600)
        self.process = None
        self.group = None
        self.attempts = []

    def start(self):
        def limits():
            if self.quota:
                signal.signal(signal.SIGXFSZ, signal.SIG_IGN)
                resource.setrlimit(resource.RLIMIT_FSIZE, (self.quota, self.quota))
        log = (self.folder / "host.log").open("ab")
        self.process = subprocess.Popen([str(self.binary), "--control-host", str(self.config_path)],
            cwd=self.folder, env=self.env, stdin=subprocess.DEVNULL, stdout=log, stderr=log,
            start_new_session=True, preexec_fn=limits if self.quota else None)
        log.close()
        self.group = self.process.pid
        def ready():
            if self.process.poll() is not None:
                raise RuntimeError("Host startup failed: " + (self.folder / "host.log").read_text())
            try:
                return self.rpc("Snapshot", record=False).get("value")
            except (OSError, ValueError):
                return None
        wait(ready, "host endpoint")

    def rpc(self, request, record=True):
        endpoint = json.loads((self.state_dir / "endpoint.json").read_text())
        address, port = endpoint["address"].rsplit(":", 1)
        with socket.create_connection((address, int(port)), timeout=5) as connection:
            connection.settimeout(6)
            connection.sendall((json.dumps({"token": endpoint["token"], "request": request}) + "\n").encode())
            response = connection.makefile("rb").readline(17 * 1024 * 1024)
        value = json.loads(response)
        if record:
            self.attempts.append({"request": request, "response": value})
        return value

    def snapshot(self):
        result = self.rpc("Snapshot", record=False)
        if result["error"]:
            raise RuntimeError(result["error"])
        return result["value"]

    def start_request(self, operation, prompt="working"):
        return {"Start": {"project": str(self.project), "prompt": prompt, "operation_id": operation}}

    def counts(self):
        rows = self.records()
        return {name: sum(row.get("method") == name for row in rows)
                for name in ("thread/start", "thread/resume", "turn/start", "turn/steer", "turn/interrupt")}

    def records(self):
        path = self.folder / "provider.jsonl"
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def state_summary(self):
        state = self.snapshot()
        return {key: state[key] for key in ("connected", "threads", "pending_requests", "last_error")} | {
            "operations_count": len(state["operations"]),
            "operations": {key: value for key, value in state["operations"].items() if not key.startswith("seed-")}}

    def crash(self):
        if self.process and self.process.poll() is None:
            self.process.kill()
            self.process.wait(timeout=5)

    def stop(self):
        self.state_dir.chmod(0o700)
        if self.group:
            try:
                os.killpg(self.group, signal.SIGKILL)
            except ProcessLookupError:
                pass
        if self.process:
            self.process.wait(timeout=5)
        self.group = None

    def export(self):
        # Never export endpoint.json or its IPC credential.
        save(self.evidence / (self.folder.name + "-calls.json"), self.attempts)
        for name in ("provider.jsonl", "host.log"):
            source = self.folder / name
            if source.exists():
                shutil.copyfile(source, self.evidence / (self.folder.name + "-" + name))


def install_case(binary, folder, evidence):
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise RuntimeError("This archive rehearsal requires an actual macOS arm64 host; other targets are not tested by relabeling an archive")
    env = clean_env(folder)
    dist = folder / "dist"
    command = [sys.executable, str(ROOT / "scripts/package-release.py"), "--target", "aarch64-apple-darwin",
               "--binary", str(binary), "--out-dir", str(dist)]
    packaged = subprocess.run(command, env=env, cwd=ROOT, capture_output=True, text=True, timeout=30)
    (evidence / "package.log").write_text(packaged.stdout + packaged.stderr)
    assert packaged.returncode == 0
    archive = dist / "they-work-aarch64-apple-darwin.tar.gz"
    shutil.copyfile(dist / (archive.name + ".sha256"), dist / "SHA256SUMS")
    original_archive = archive.read_bytes()
    original_sums = (dist / "SHA256SUMS").read_bytes()
    tools = folder / "tools"
    tools.mkdir()
    curl = tools / "curl"
    curl.write_text(f'''#!{sys.executable}
import json,os,pathlib,shutil,sys
base=pathlib.Path(os.environ['RELIABILITY_DIST'])
url=next(a for a in sys.argv[1:] if a.startswith('https://'))
with (base/'requests.jsonl').open('a') as f:f.write(json.dumps({{'url':url}})+'\\n')
if os.environ.get('RELIABILITY_FAILURE')=='download':sys.exit(22)
assert url.startswith('https://github.com/Giuseppecarte/they-work/releases/download/v0.7.0-audit/')
shutil.copyfile(base/url.rsplit('/',1)[1],sys.argv[sys.argv.index('-o')+1])
''')
    curl.chmod(0o755)
    env.update(PATH=str(tools) + ":" + env["PATH"], RELIABILITY_DIST=str(dist))
    destination = folder / "installed path with spaces"
    target = destination / "they-work"
    checks, runs = [], []
    def run(label):
        result = subprocess.run(["sh", str(ROOT / "scripts/install.sh"), "--version", "v0.7.0-audit",
             "--install-dir", str(destination)], cwd=folder, env=env, capture_output=True, text=True, timeout=20)
        runs.append({"case": label, "exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr})
        return result
    result = run("fresh real archive install")
    checks.append(check("fresh installation matches the archived release", result.returncode == 0 and sha(target) == sha(binary), result.returncode))
    smoke = subprocess.run([str(target), "--demo", "--once"], cwd=folder, env=env, capture_output=True, text=True, timeout=15)
    (evidence / "installed-demo-once.log").write_text(smoke.stdout + smoke.stderr)
    checks.append(check("installed executable runs without source data", smoke.returncode == 0, smoke.returncode))
    inode = target.stat().st_ino
    live = subprocess.Popen([str(target), "--demo", "--headless", "--exit-after", "1500ms"], cwd=folder,
                            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    result = run("replace same-build installation while it is running")
    _, error = live.communicate(timeout=5)
    checks.append(check("atomic same-build replacement preserves running process", result.returncode == 0 and target.stat().st_ino != inode and live.returncode == 0,
                        {"installer_exit": result.returncode, "running_exit": live.returncode, "stderr": error.decode()}))
    for mode in ("download", "checksum", "truncated", "unwritable"):
        archive.write_bytes(original_archive)
        (dist / "SHA256SUMS").write_bytes(original_sums)
        env.pop("RELIABILITY_FAILURE", None)
        if mode == "download": env["RELIABILITY_FAILURE"] = "download"
        if mode == "checksum": (dist / "SHA256SUMS").write_text("0" * 64 + "  " + archive.name + "\n")
        if mode == "truncated":
            archive.write_bytes(original_archive[:100])
            (dist / "SHA256SUMS").write_text(sha(archive) + "  " + archive.name + "\n")
        before = sha(target)
        if mode == "unwritable": destination.chmod(0o500)
        try: result = run(mode)
        finally: destination.chmod(0o700)
        checks.append(check(mode + " preserves previous executable", result.returncode != 0 and sha(target) == before,
                            {"exit": result.returncode, "preserved_sha256": sha(target)}))
    save(evidence / "installer-runs.json", runs)
    shutil.copyfile(dist / "requests.jsonl", evidence / "installer-download-boundary.jsonl")
    return checks, {"archive_sha256": hashlib.sha256(original_archive).hexdigest(),
                    "transport": "curl substituted with local archive/checksum files; no HTTP or GitHub request",
                    "update_scope": "same-build executable replacement, not a cross-version migration"}


def control_case(name, binary, folder, evidence):
    host = Host(binary, folder, evidence, quota=4096 if name == "quota" else None)
    checks, details = [], {}
    try:
        host.start()
        if name == "concurrent":
            request = host.start_request("shared")
            with ThreadPoolExecutor(max_workers=8) as pool:
                replies = list(pool.map(lambda _: host.rpc(request), range(8)))
            counts = host.counts()
            checks.append(check("eight same-ID clients execute once", all((r.get("value") or {}).get("status") == "Confirmed" for r in replies)
                                and counts["thread/start"] == counts["turn/start"] == 1, counts))
            conflict = host.rpc(host.start_request("shared", "different"))
            checks.append(check("same ID with changed payload is rejected", bool(conflict["error"]) and host.counts() == counts, conflict))
        elif name == "control-actions":
            started = host.rpc(host.start_request("baseline-approval", "approval"))
            state = wait(lambda: (s if (s := host.snapshot())["pending_requests"] else None), "baseline approval")
            pending = state["pending_requests"][0]
            reply = host.rpc({"Reply": {"request_id": pending["id"], "response": {"decision": "decline"},
                                       "operation_id": "baseline-decline"}})
            wait(lambda: not host.snapshot()["pending_requests"], "baseline request resolution")
            interrupt = host.rpc({"Interrupt": {"thread_id": "managed-1", "turn_id": state["threads"]["managed-1"]["active_turn_id"],
                                               "operation_id": "baseline-stop"}})
            checks.append(check("baseline approval belongs to an accepted actual fixture turn", (started.get("value") or {}).get("status") == "Confirmed", started))
            checks.append(check("decline format is accepted below the bound", (reply.get("value") or {}).get("status") == "Confirmed"
                                and sum(r.get("id") == "approval-1" and "result" in r for r in host.records()) == 1, reply))
            checks.append(check("interrupt format is accepted below the bound", (interrupt.get("value") or {}).get("status") == "Confirmed"
                                and host.counts()["turn/interrupt"] == 1, interrupt))
        elif name in ("uncertain", "crash"):
            request = host.start_request("lost-ack", "uncertain")
            if name == "crash":
                with ThreadPoolExecutor(max_workers=1) as pool:
                    pending = pool.submit(host.rpc, request)
                    wait(lambda: host.counts()["turn/start"] == 1, "provider received turn")
                    host.crash()
                    try: details["interrupted_call"] = pending.result()
                    except (OSError, ValueError) as error: details["interrupted_call"] = type(error).__name__
            else:
                first = host.rpc(request)
                checks.append(check("missing acknowledgement reports uncertainty", (first.get("value") or {}).get("status") == "Uncertain", first))
            before = host.counts()
            host.stop(); host.start()
            checks.append(check("restart does not launch provider or replay", host.counts() == before and not host.snapshot()["connected"], host.counts()))
            duplicate = host.rpc(request)
            checks.append(check("duplicate remains uncertain without execution", (duplicate.get("value") or {}).get("status") == "Uncertain" and host.counts() == before, duplicate))
            reconnect = host.rpc({"Reconnect": {"thread_id": "managed-1", "operation_id": "recover"}})
            checks.append(check("explicit reconnect does not start another turn", (reconnect.get("value") or {}).get("status") == "Confirmed"
                                and host.counts()["turn/start"] == 1 and host.counts()["thread/resume"] == 1, host.counts()))
        elif name in ("permission", "quota"):
            request = host.start_request("write-failed", "x" * 5000 if name == "quota" else "working")
            if name == "permission": host.state_dir.chmod(0o500)
            first = host.rpc(request)
            host.state_dir.chmod(0o700)
            checks.append(check("failed durable intent executes no provider command", bool(first["error"]) and host.counts()["thread/start"] == 0, first))
            if name == "permission":
                time.sleep(2)
                details["restored_write_access_wait_ms"] = 2000
                retry = host.rpc(request)
                checks.append(check("storage recovery does not leave permanent Sending", (retry.get("value") or {}).get("status") != "Sending", retry))
                details["retry_provider_counts"] = host.counts()
            host.stop(); host.quota = None; host.start()
            recovered = host.rpc(request)
            checks.append(check("restart after pre-send failure can execute once", (recovered.get("value") or {}).get("status") == "Confirmed"
                                and host.counts()["turn/start"] == 1, recovered))
        elif name == "after-send":
            request = host.start_request("after-write", "gate-ack")
            with ThreadPoolExecutor(max_workers=1) as pool:
                pending = pool.submit(host.rpc, request)
                wait(lambda: (folder / "gate-ready").exists(), "provider acknowledgement gate")
                host.state_dir.chmod(0o500)
                (folder / "gate-release").write_text("release fixture acknowledgement\n")
                first = pending.result()
            host.state_dir.chmod(0o700)
            checks.append(check("post-send persistence error does not claim durable confirmation", bool(first["error"]) and host.counts()["turn/start"] == 1, first))
            host.stop(); host.start()
            duplicate = host.rpc(request)
            checks.append(check("post-send restart preserves uncertainty and does not replay", (duplicate.get("value") or {}).get("status") == "Uncertain"
                                and host.counts()["turn/start"] == 1, duplicate))
        elif name == "ledger":
            host.rpc(host.start_request("initial"))
            wait(lambda: host.snapshot()["threads"]["managed-1"]["active_turn_id"], "initial turn")
            host.stop()
            state_path = host.state_dir / "state.json"
            state = json.loads(state_path.read_text())
            for index in range(9998 - len(state["operations"])):
                key = f"seed-{index:05}"
                state["operations"][key] = {"id": key, "status": "Confirmed", "thread_id": None,
                    "turn_id": None, "detail": "synthetic prior receipt", "request_key": "synthetic prior request"}
            save(state_path, state); state_path.chmod(0o600)
            host.start()
            host.rpc({"Reconnect": {"thread_id": "managed-1", "operation_id": "reconnect-boundary"}})
            started = host.rpc({"Send": {"thread_id": "managed-1", "prompt": "approval", "expected_turn_id": None,
                                         "operation_id": "boundary-turn"}})
            state = wait(lambda: (s if (s := host.snapshot())["pending_requests"] else None), "boundary approval")
            request = state["pending_requests"][0]
            interrupt = host.rpc({"Interrupt": {"thread_id": "managed-1", "turn_id": state["threads"]["managed-1"]["active_turn_id"],
                                               "operation_id": "stop-at-limit"}})
            reply = host.rpc({"Reply": {"request_id": request["id"], "response": {"decision": "decline"},
                                       "operation_id": "decline-at-limit"}})
            checks.append(check("rehearsal reaches exactly 10000 receipts with one current request", len(state["operations"]) == 10000
                                and (started.get("value") or {}).get("status") == "Confirmed", len(state["operations"])))
            checks.append(check("owned active turn remains interruptible at history limit", (interrupt.get("value") or {}).get("status") == "Confirmed", interrupt))
            checks.append(check("current request can be declined at history limit", (reply.get("value") or {}).get("status") == "Confirmed", reply))
            details["seed_scope"] = "9997 synthetic historical receipts plus an actual initial receipt; only boundary operations execute"
        details["final_state"] = host.state_summary()
        details["provider_counts"] = host.counts()
        return checks, details
    finally:
        host.stop()
        host.export()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--scratch", type=Path, default=ROOT / "docs/design-audit/tmp/iteration-7-reliability")
    parser.add_argument("--evidence", type=Path, default=HERE / "evidence")
    parser.add_argument("--cases", nargs="*", choices=list(EXPECTED), default=list(EXPECTED))
    args = parser.parse_args()
    binary = args.binary.resolve()
    scratch = args.scratch.resolve() / time.strftime("run-%Y%m%d-%H%M%S")
    scratch.mkdir(parents=True)
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    result = {"schema": 1, "binary": str(binary), "binary_sha256": sha(binary), "platform": platform.platform(),
              "real_provider_calls": False, "public_release_calls": False, "scratch": str(scratch),
              "expected_outcomes": EXPECTED, "cases": {}}
    for name in args.cases:
        folder = scratch / name
        folder.mkdir()
        print("START", name, flush=True)
        try:
            checks, details = install_case(binary, folder, evidence) if name == "install" else control_case(name, binary, folder, evidence)
            status = "defect" if any(row["status"] == "defect" for row in checks) else "pass"
            result["cases"][name] = {"status": status, "checks": checks, "details": details}
        except Exception as error:
            result["cases"][name] = {"status": "harness-error", "error": str(error), "traceback": traceback.format_exc()}
        print("END", name, result["cases"][name]["status"], flush=True)
        result["summary"] = {"scenarios": len(result["cases"]),
            **{status: sum(row["status"] == status for case in result["cases"].values() for row in case.get("checks", []))
               for status in ("pass", "defect")},
            "harness_errors": sum(case["status"] == "harness-error" for case in result["cases"].values())}
        save(evidence / "results.json", result)
    print(json.dumps({name: value["status"] for name, value in result["cases"].items()}), flush=True)


if __name__ == "__main__":
    main()
