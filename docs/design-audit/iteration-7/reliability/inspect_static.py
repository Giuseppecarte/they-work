#!/usr/bin/env python3
"""Record source ordering; this does not execute Windows or publish a release."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent


def source(path, first, second):
    text = (ROOT / path).read_text()
    a, b = text.index(first), text.index(second)
    return {"path": path, "sha256": hashlib.sha256(text.encode()).hexdigest(),
            "first_line": text[:a].count("\n") + 1, "second_line": text[:b].count("\n") + 1,
            "first_precedes_second": a < b}


result = {
    "windows_state_replace": {
        "code_observation": source("crates/theywork-control/src/security.rs", "fs::remove_file(path)?", "fs::rename(&tmp, path)?"),
        "runtime_status": "not-tested",
        "known": "The Windows branch removes the previous state file before renaming the replacement.",
        "hypothesis": "A process or machine failure in that interval can leave no state file and lose saved ownership/receipts.",
        "next_experiment": "On a Windows VM, stop at that exact interval and restart; inspect ownership and same-ID behavior without provider tasks.",
    },
    "publish_before_verify": {
        "code_observation": source(".github/workflows/release.yml", "push: true", "Verify the published image through a capable PTY"),
        "runtime_status": "not-tested",
        "known": "The workflow pushes the version and latest container tags before the published-image smoke check; native release assets depend on that check.",
        "hypothesis": "A failed post-push check leaves newly advanced container tags without a completed native release.",
        "next_experiment": "Rehearse failure after push in a disposable registry/workflow stub; inspect tags and release state. Do not publish public artifacts.",
    },
    "remaining_execution_gaps": [
        {"case": name, "status": "not-tested"} for name in (
            "Windows installer, user PATH and running executable replacement",
            "Windows control DACL, console and detached host runtime",
            "Real HTTP redirects, release download, signing/notarization and OS launch prompts",
            "Cross-version preference/control-state migration",
            "Actual ENOSPC disk exhaustion and physical power loss",
            "Authenticated providers and provider task side effects",
        )
    ],
}
output = HERE / "evidence/static-boundaries.json"
output.parent.mkdir(exist_ok=True)
output.write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps({key: value["runtime_status"] for key, value in result.items() if isinstance(value, dict)}))
