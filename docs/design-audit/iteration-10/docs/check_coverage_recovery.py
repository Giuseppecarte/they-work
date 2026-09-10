#!/usr/bin/env python3
"""Current 80x24 coverage and storage-recovery routes; synthetic stores only."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile
import traceback

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location(
    "routes8", ROOT / "docs/design-audit/iteration-8/docs/check_routes.py"
)
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)
w = r.w


def activate(s, label):
    """Choose a visible semantic control using real Tab/Enter events."""
    for _ in range(40):
        s.action("Tab to " + label, r.TAB)
        if "> " + label + " ]" in s.text():
            s.action("Activate " + label, b"\r")
            return
    raise AssertionError("Could not focus " + label + "\n" + s.text())


def coverage(binary, folder, output):
    r.prepare(folder)
    s = r.Session(binary, folder, r.arguments(folder))
    cases = {}
    try:
        s.wait(lambda: "Connections / Sources" in s.text(), "source chooser")
        s.action("Connect isolated sources", r.F5)
        s.wait(lambda: "alpha-shop" in s.text(), "source discovery")
        s.action("Open notebook", b"b")
        for key, label in [(b"1", "attention"), (b"2", "deliveries"),
                           (b"3", "changes"), (b"4", "team")]:
            s.action("Choose " + label, key)
            s.action("Select next retained record", b"\x1b[B")
            before = [line[:36] for line in s.text().splitlines() if line.lstrip().startswith(">")]
            markers = r.state_hashes(r.config(folder))
            s.action("Read coverage with h", b"h")
            s.wait(lambda: "read · h back" in s.text(), "coverage report")
            s.action("Read coverage from its start", b"\x1b[H")
            assert "COVERAGE" in s.text(), s.text()
            assert "Back" in s.text(), s.text()
            s.capture(output, label + "-coverage")
            s.action("Page through coverage", b"\x1b[6~")
            s.action("Enter must not inspect an obscured record", b"\r")
            s.action("r must not mark an obscured record", b"r")
            assert "read · h back" in s.text(), s.text()
            assert markers == r.state_hashes(r.config(folder)), "Coverage changed reading markers"
            s.action("Esc returns to retained record", r.ESC)
            assert "COVERAGE" not in s.text(), s.text()
            after = [line[:36] for line in s.text().splitlines() if line.lstrip().startswith(">")]
            assert before == after, (before, after)
            s.capture(output, label + "-returned")
            cases[label] = {"coverage_readable": True, "back_visible": True,
                            "esc_restores_selection": True, "hidden_record_actions_blocked": True,
                            "settings_unchanged_by_coverage": True}
        cases["terminal_exit"] = r.close(s, output, "coverage")
        return cases
    finally:
        r.cleanup(s, folder)


def recovery(binary, folder, output):
    w.control.prepare(folder)
    s = None
    state_path = None
    saved = None
    try:
        s = w.Session(binary, folder, columns=80, rows=24, mouse=False)
        s.wait(lambda: len(w.control.records(folder / "invocations.jsonl")) >= 3, "fake probes")
        s.action("Create fixture task", b"n")
        s.paste(str(folder / "vertical-project"))
        s.action("Focus task instruction", b"\t")
        s.paste("working")
        s.action("Send initial task", r.F5)
        calls = lambda: w.control.records(folder / "provider.jsonl")
        starts = lambda: [x for x in calls() if x.get("method") == "turn/start"]
        steers = lambda: [x for x in calls() if x.get("method") == "turn/steer"]
        s.wait(lambda: len(starts()) == 1, "initial managed turn")
        s.wait(lambda: "Sending to" not in s.text(), "initial acknowledgement")
        s.action("Close task form", r.ESC)
        w.find(s, "working")
        s.action("Compose for exact managed worker", b"m")
        s.paste("Keep recipient and draft during canonical recovery.")
        state_path, = list((folder / "settings/control").glob("*/state.json"))
        saved = state_path.read_bytes()
        # This fixture intentionally removes its canonical state. The saved
        # bytes stay private and are restored only to test the restart latch.
        state_path.unlink()
        s.action("Explicit send after canonical loss", r.F5)
        s.wait(lambda: "Not sent:" in s.text(), "pre-send rejection")
        draft_visible_during_fault = "Keep recipient and draft" in s.text()
        assert "INSTRUCTION TO vertical-project / working" in s.text(), s.text()
        assert "Observation only" in s.text(), s.text()
        assert not steers(), "Missing canonical state dispatched work"
        s.capture(output, "canonical-loss-draft")
        s.action("Leave preserved composer", r.ESC)
        s.action("Close worker brief", r.ESC)
        s.action("Inspect Connections storage error", b"c")
        s.wait(lambda: "CONNECTIONS" in s.text(), "Connections")
        s.pump(2)
        observed = [s.text()]
        for _ in range(5):
            if "recovery" in " ".join(observed).lower():
                break
            s.action("Scroll Connections details", b"\x1b[6~")
            observed.append(s.text())
        assert "recovery" in " ".join(observed).lower(), "Recovery missing from Connections"
        s.capture(output, "canonical-loss-connections")
        state_path.write_bytes(saved)
        state_path.chmod(0o600)
        s.pump(2)
        assert len(starts()) == 1 and not steers(), "Restore resent a task"
        s.action("Leave Connections", r.ESC)
        w.find(s, "working")
        s.capture(output, "canonical-restored-live-host-blocked")
        # Keep this office process alive: drafts are view-local. Stop only the
        # fixture supervisor/provider, then deliberately reconnect its task.
        w.control.stop_owned_fixture_hosts(folder)
        s.pump(3)
        assert len(starts()) == 1 and not steers(), "Stopping the host replayed work"
        activate(s, "Task actions")
        activate(s, "Reconnect")
        resumes = lambda: [x for x in calls() if x.get("method") == "thread/resume"]
        s.wait(lambda: len(resumes()) == 1, "explicit resume without sending a turn")
        s.action("Reopen the recipient-bound draft", b"m")
        s.wait(lambda: "Keep recipient and draft" in s.text(), "draft restored after checked reconnect")
        assert "INSTRUCTION TO vertical-project / working" in s.text(), s.text()
        assert len(starts()) == 1 and not steers(), "Reconnect replayed the preserved draft"
        assert resumes()[0]["params"]["threadId"] == "managed-1", resumes()
        s.capture(output, "canonical-reconnected-draft-preserved")
        exit_result = r.close(s, output, "recovery")
        s = None
        w.control.stop_owned_fixture_hosts(folder)
        s = w.Session(binary, folder, columns=80, rows=24, mouse=False)
        s.pump(3)
        assert len(starts()) == 1 and not steers(), "Reopening resent a task"
        s.action("Inspect restored saved-state connection", b"c")
        s.wait(lambda: "CONNECTIONS" in s.text(), "restored Connections")
        s.capture(output, "canonical-restored-reopened")
        reopened_exit = r.close(s, output, "recovery-reopened")
        for name in ["provider.jsonl", "invocations.jsonl"]:
            shutil.copy2(folder / name, output / name)
        return {"draft_visible_during_fault": draft_visible_during_fault,
                "same_draft_visible_after_checked_reconnect": True,
                "exact_recipient_after_reconnect": "managed-1", "explicit_reconnects": len(resumes()),
                "recovery_error_discoverable": True,
                "rejected_provider_calls": 0, "automatic_resends": 0,
                "initial_turn_starts": len(starts()), "terminal_exit": exit_result,
                "reopened_exit": reopened_exit}
    finally:
        if state_path and saved is not None and not state_path.exists():
            state_path.write_bytes(saved)
            state_path.chmod(0o600)
        r.cleanup(s, folder)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / "target/audit/iteration-10/docs/fixtures"
    scratch.mkdir(parents=True, exist_ok=True)
    folder = Path(tempfile.mkdtemp(prefix="coverage-recovery-", dir=scratch))
    binary = folder / "they-work"
    shutil.copy2(args.binary.resolve(), binary)
    result = {"binary_sha256": w.digest(binary), "cells": [80, 24],
              "keyboard_only": True, "authenticated_provider": False,
              "participant_sessions": 0, "physical_terminal": "not tested", "cases": {}}
    try:
        result["cases"]["coverage"] = coverage(binary, folder / "coverage", output)
        result["cases"]["recovery"] = recovery(binary, folder / "recovery", output)
        result["status"] = "pass"
    except Exception:
        result["status"] = "fail"
        result["failure"] = traceback.format_exc()
        raise
    finally:
        (output / "results.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"status": result["status"], "cases": list(result["cases"])}))


if __name__ == "__main__":
    main()
