#!/usr/bin/env python3
"""Compare identical synthetic full screens, without claiming terminal paint."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("replay9", ROOT / "docs/design-audit/iteration-9/replay.py")
replay9 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay9)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    before = json.loads((args.before / "inventory.json").read_text())["cases"]
    after = json.loads((args.after / "inventory.json").read_text())["cases"]
    if {c["id"] for c in before} != {c["id"] for c in after}:
        raise SystemExit("Before/after case IDs differ")
    cases = []
    for case in after:
        name = case["id"]
        if not all(case["automatic"].values()):
            raise SystemExit(f"Route/bounds failure: {name}")
        records = []
        cells = []
        for label, source in [("before", args.before), ("after", args.after)]:
            target = args.output / label
            target.mkdir(exist_ok=True)
            source_cells = source / f"{name}.cells.json"
            records.append(replay9.replay(source_cells, target / f"{name}.png"))
            cells.append(json.loads(source_cells.read_text()))
        # The buffer also includes color-dependent block encodings beneath the
        # image. Only native labels describe semantics; block changes are art.
        native = lambda data: [[(c["native"], c["text"] if c["native"] else "") for c in row] for row in data["cells"]]
        text_same = native(cells[0]) == native(cells[1])
        hits_same = (args.before / f"{name}.hits.json").read_bytes() == (args.after / f"{name}.hits.json").read_bytes()
        geometry_same = cells[0]["image"] == cells[1]["image"]
        if not text_same or not hits_same or not geometry_same:
            raise SystemExit(f"Unexpected semantic/geometry change: {name}")
        cases.append({**case, "comparison": {
            "native_text_identical": text_same, "hit_geometry_identical": hits_same,
            "image_geometry_identical": geometry_same,
            "pixels_changed": records[0]["rgb_sha256"] != records[1]["rgb_sha256"],
            "before_rgb_sha256": records[0]["rgb_sha256"],
            "after_rgb_sha256": records[1]["rgb_sha256"],
        }})
    manifest = {"kind": "Compositor comparisons; no terminal, provider or human trial",
                "baseline_revision": "282722294b8b372d7e20bda4a3705f3e19b62b1b",
                "fixture": "crates/theywork-render/examples/color_preview.rs",
                "fixture_sha256": digest(ROOT / "crates/theywork-render/examples/color_preview.rs"),
                "replay": {"fonts": records[0]["fonts"], "pillow": records[0]["pillow"], "freetype": records[0]["freetype"]},
                "cases": cases}
    (args.output / "comparison.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"{len(cases)} complete screens: identical text, hit geometry and image geometry; replayed before/after")


if __name__ == "__main__":
    main()
