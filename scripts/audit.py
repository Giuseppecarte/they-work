#!/usr/bin/env python3
"""Bootstrap and reproduce one complete Ui screen and one synthetic collector test."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import uuid

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "target" / "audit"
ASSETS = ROOT / "scripts" / "audit_assets"
CASE = "tower-image-80x24"
FIXTURE = "codex_updates_same_timestamp_items_and_retains_removed_deliveries_without_writing_stores"


class AuditError(Exception):
    pass


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def run(command, *, env=None, log=None, cwd=ROOT):
    try:
        process = subprocess.run([str(x) for x in command], cwd=cwd, env=env,
                                 text=True, encoding="utf-8", errors="replace", capture_output=True)
    except OSError as error:
        raise AuditError(f"Cannot execute {command[0]}: {error}") from error
    if log:
        log.write_text("$ " + json.dumps([str(x) for x in command]) + "\n" + process.stdout + process.stderr,
                       encoding="utf-8")
    if process.returncode:
        detail = (process.stderr or process.stdout)[-4000:]
        raise AuditError(f"Command failed ({process.returncode}): {command[0]}\n{detail}" +
                         (f"\nFull log: {log}" if log else ""))
    return process.stdout


def python_info(executable):
    info = json.loads(run([executable, "-c", "import json,sys; print(json.dumps({'version':list(sys.version_info[:3]),'executable':sys.executable}))"]))
    if info["version"][:2] != [3, 12]:
        raise AuditError(f"Python 3.12 is required; {executable} is {'.'.join(map(str, info['version']))}. "
                         "Install Python 3.12 and run bootstrap --python /path/to/python3.12. No global packages are installed.")
    return info


def find_python(explicit=None):
    if explicit:
        python_info(explicit)
        return explicit
    candidates = [sys.executable, shutil.which("python3.12")]
    for candidate in candidates:
        if candidate:
            try:
                python_info(candidate)
                return candidate
            except AuditError:
                pass
    raise AuditError("Python 3.12 was not found. Install Python 3.12 and run: python3 scripts/audit.py bootstrap --python /path/to/python3.12")


def cargo_info(explicit=None, *, offline=False):
    cargo = explicit or shutil.which("cargo")
    if not cargo:
        raise AuditError("Cargo was not found. Install Rust with rustup, then: rustup toolchain install 1.90.0 --profile minimal")
    expected = re.search(r'channel\s*=\s*"([^"]+)"', (ROOT / "rust-toolchain.toml").read_text())[1]
    version = run([cargo, "--version"], env=environment() if offline else None).strip()
    if version.split()[1] != expected:
        raise AuditError(f"Expected Cargo {expected} from rust-toolchain.toml, got {version}. Install the pinned rustup toolchain.")
    return cargo, version


def rustc_info(cargo):
    located = shutil.which(str(cargo)) or str(cargo)
    compiler = Path(located).absolute().parent / ("rustc.exe" if os.name == "nt" else "rustc")
    return run([compiler, "--version", "--verbose"], env=environment()).strip()


def environment():
    env = os.environ.copy()
    for name in list(env):
        if name.startswith("THEYWORK_") or name in {"NO_COLOR", "PYTHONPATH", "PYTHONHOME"}:
            env.pop(name, None)
    env.update(TERM="xterm-256color", COLORTERM="truecolor", THEYWORK_ENCODING="quadrants", PYTHONNOUSERSITE="1",
               PYTHONDONTWRITEBYTECODE="1", CARGO_TERM_COLOR="never", CARGO_NET_OFFLINE="true",
               RUSTUP_AUTO_INSTALL="0")
    # Keep selected toolchain/cache overrides, but default compiler output and
    # synthetic database fixtures to this checkout's ignored audit directory.
    env.setdefault("CARGO_TARGET_DIR", str(BASE / "cargo-target"))
    return env


def venv_python():
    return BASE / "venv" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")


def bootstrap(args):
    python = find_python(args.python)
    cargo, version = cargo_info(args.cargo)
    BASE.mkdir(parents=True, exist_ok=True)
    from audit_replay import verify_fonts
    verify_fonts()
    env = environment()
    env.pop("CARGO_NET_OFFLINE", None)
    run([python, "-m", "venv", str(BASE / "venv")], env=env, log=BASE / "bootstrap-venv.log")
    run([venv_python(), "-m", "pip", "--isolated", "install", "--disable-pip-version-check",
         "--require-hashes", "--only-binary=:all:", "--no-deps", "-r", ASSETS / "requirements.lock"],
        env=env, log=BASE / "bootstrap-pillow.log")
    # Fetch the workspace lock, not just Python dependencies: smoke must not
    # depend on an already warm Cargo cache or contact a registry.
    run([cargo, "fetch", "--locked"], env=env, log=BASE / "bootstrap-cargo.log")
    write_json(BASE / "bootstrap.json", {"python": python_info(venv_python()), "cargo": version,
               "cargo_executable": str(Path(shutil.which(str(cargo)) or cargo).absolute()),
               "requirements_sha256": digest(ASSETS / "requirements.lock"),
               "cargo_lock_sha256": digest(ROOT / "Cargo.lock")})
    print(f"Bootstrap ready. Next: python3 scripts/audit.py smoke\nLocal dependencies: {BASE}")


def compiler_executable(output):
    artifacts = []
    for line in output.splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        target = record.get("target", {})
        if record.get("reason") == "compiler-artifact" and target.get("name") == "ui_inventory" and "example" in target.get("kind", []) and record.get("executable"):
            artifacts.append(record["executable"])
    if len(set(artifacts)) != 1:
        raise AuditError(f"Expected one ui_inventory executable in Cargo JSON; found {len(set(artifacts))}.")
    return Path(artifacts[0])


def validate_fixture(output):
    runs = re.findall(r"^running (\d+) tests?$", output, re.MULTILINE)
    exact = re.search(r"^test " + re.escape(FIXTURE) + r" \.\.\. ok$", output, re.MULTILINE)
    if runs != ["1"] or not exact or "test result: ok. 1 passed; 0 failed; 0 ignored;" not in output:
        raise AuditError("The exact collector fixture did not run once and pass. A zero-test filter is not a successful smoke.")


def validate_screen(raw):
    inventory = json.loads((raw / "inventory.json").read_text())
    cases = inventory["cases"] if isinstance(inventory, dict) else inventory
    if len(cases) != 1 or cases[0].get("id") != CASE:
        raise AuditError("Expected exactly one tower-image-80x24 export; the example filter changed.")
    record = cases[0]
    if record.get("automatic") != {"route": True, "hit_bounds": True}:
        raise AuditError("The real Ui route or hit-region bounds failed.")
    # Diagnostics names the character fallback (e.g. Quadrants), even while
    # image density is active. Actual image presence/size is checked by replay.
    if record["configuration"]["cell_pixels"] != [8, 16] or record.get("mode") != "image":
        raise AuditError("The exporter did not use the required image-mode 8x16 geometry.")
    hits = json.loads((raw / f"{CASE}.hits.json").read_text())
    if not hits or any(min(h["x"], h["y"]) < 0 or min(h["width"], h["height"]) <= 0 or h["x"] + h["width"] > 80 or h["y"] + h["height"] > 24 for h in hits):
        raise AuditError("Missing or out-of-bounds actual Ui hit regions.")
    return record, len(hits)


def output_directory(requested):
    base = BASE.resolve()
    path = Path(requested).resolve() if requested else base / "smoke" / (datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ-") + uuid.uuid4().hex[:8])
    if not path.is_relative_to(base) or path == base:
        raise AuditError("Smoke output must be a new directory under target/audit; historical audit artifacts cannot be overwritten.")
    if path.exists():
        raise AuditError(f"Output already exists: {path}. Choose a new --output directory.")
    return path


def smoke(args):
    marker = BASE / "bootstrap.json"
    if not marker.is_file() or not venv_python().is_file():
        raise AuditError("Audit dependencies are absent. Run: python3 scripts/audit.py bootstrap --python /path/to/python3.12")
    stamp = json.loads(marker.read_text())
    if stamp["requirements_sha256"] != digest(ASSETS / "requirements.lock") or stamp["cargo_lock_sha256"] != digest(ROOT / "Cargo.lock"):
        raise AuditError("Dependency locks changed since bootstrap. Run scripts/audit.py bootstrap again.")
    python = python_info(venv_python())
    cargo, version = cargo_info(args.cargo or stamp.get("cargo_executable"), offline=True)
    try:
        pillow = run([venv_python(), "-c", "import PIL; print(PIL.__version__)"]).strip()
    except AuditError as error:
        raise AuditError("Pillow is unavailable in the local audit venv. Run scripts/audit.py bootstrap again.") from error
    if pillow != json.loads((ASSETS / "dependencies.json").read_text())["pillow"]:
        raise AuditError("Pillow differs from the lock. Run scripts/audit.py bootstrap again.")
    from audit_replay import verify_fonts
    verify_fonts()
    out = output_directory(args.output)
    raw = out / "raw"
    raw.mkdir(parents=True)
    logs, scratch = out / "logs", out / "fixture-tmp"
    logs.mkdir()
    scratch.mkdir()
    env = environment()
    env.update(TMPDIR=str(scratch), TMP=str(scratch), TEMP=str(scratch))
    failure = {"status": "failed", "visual_review": "not-tested", "terminal_transport": "not-tested"}
    write_json(out / "metadata.json", failure)
    try:
        built = run([cargo, "build", "--locked", "--offline", "-p", "theywork-render", "--example", "ui_inventory", "--message-format=json"], env=env, log=logs / "cargo-build.log")
        executable = compiler_executable(built)
        run([executable, raw, CASE, "8x16"], env=env, log=logs / "capture.log")
        record, hit_count = validate_screen(raw)
        replay_info = json.loads(run([venv_python(), ROOT / "scripts" / "audit_replay.py", raw / f"{CASE}.cells.json", out / f"{CASE}.png"], env=env, log=logs / "replay.log"))
        fixture_output = run([cargo, "test", "--locked", "--offline", "-p", "theywork-collect", "--test", "collaboration", FIXTURE, "--", "--exact", "--nocapture"], env=env, log=logs / "fixture.log")
        validate_fixture(fixture_output)
        files = [*sorted(raw.iterdir()), out / f"{CASE}.png", *sorted(logs.iterdir())]
        source_paths = [ROOT / "Cargo.lock", ROOT / "rust-toolchain.toml", ROOT / "scripts/audit.py", ROOT / "scripts/audit_replay.py", ROOT / "scripts/audit_assets/requirements.lock", ROOT / "crates/theywork-collect/tests/collaboration.rs"]
        source_paths += sorted((ROOT / "crates").rglob("*.rs"))
        source_paths += sorted((ROOT / "crates").rglob("Cargo.toml"))
        source_paths = sorted(set(source_paths))
        metadata = {"schema": 1, "status": "pass", "scope": "one complete compositor replay and one synthetic collector fixture",
                    "commit": run(["git", "rev-parse", "HEAD"]).strip(),
                    "working_tree": run(["git", "status", "--porcelain", "--untracked-files=normal"]),
                    "platform": platform.platform(), "python": python, "cargo": version,
                    "rustc": rustc_info(cargo),
                    "case": record, "clock_ms": 1000, "motion": False,
                    "binary_sha256": digest(executable), "image": replay_info,
                    "fixture": {"name": FIXTURE, "tests_run": 1, "passed": 1, "live_provider": False},
                    "hit_regions": hit_count, "offline": True,
                    "visual_review": "not-tested", "terminal_transport": "not-tested",
                    "source_sha256": {str(p.relative_to(ROOT)): digest(p) for p in source_paths},
                    "files_sha256": {str(p.relative_to(out)): digest(p) for p in files}}
        write_json(out / "metadata.json", metadata)
        (out / "index.html").write_text('<!doctype html><meta charset="utf-8"><title>They Work audit smoke</title><h1>Complete Ui replay · 80×24 · 8×16</h1><p>One synthetic fixture passed. Visual review and terminal transport are not tested by this command.</p><img width="640" height="384" alt="Complete tower compositor replay" src="' + CASE + '.png"><p><a href="metadata.json">Provenance and hashes</a> · <a href="raw/' + CASE + '.txt">Native text</a> · <a href="logs/fixture.log">Fixture result</a></p>', encoding="utf-8")
        print(f"PASS: one complete 640x384 replay + one collector fixture.\nReview: {out / 'index.html'}\nMetadata: {out / 'metadata.json'}")
    except (AuditError, ValueError, OSError, KeyError) as error:
        failure["error"] = str(error)
        write_json(out / "metadata.json", failure)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    boot = sub.add_parser("bootstrap", help="Prepare local Python dependencies and fetch locked Cargo dependencies (network allowed).")
    boot.add_argument("--python", help="Python 3.12 executable; defaults to this interpreter or python3.12 on PATH.")
    boot.add_argument("--cargo", help="Cargo executable; normally discovered on PATH.")
    smoke_parser = sub.add_parser("smoke", help="Offline, bounded reproduction; no real providers or terminal required.")
    smoke_parser.add_argument("--cargo", help="Cargo executable; normally discovered on PATH.")
    smoke_parser.add_argument("--output", help="New output directory beneath target/audit (default: unique smoke run).")
    args = parser.parse_args()
    try:
        (bootstrap if args.command == "bootstrap" else smoke)(args)
    except (AuditError, ValueError, OSError, KeyError) as error:
        print(f"Audit error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
