#!/usr/bin/env python3
"""Publish the bounded UI inventory without treating capture as visual approval."""
import argparse
import hashlib
import html
import json
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("raw", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--reviews", type=Path, action="append", default=[])
    parser.add_argument("--reuse-images", action="store_true",
                        help="Update review metadata only; require already exported current images")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    if not args.reuse_images:
        subprocess.run([sys.executable, str(repo / "docs/design-audit/iteration-4/export_ui.py"),
                        str(args.raw), str(args.output)], check=True)
    manifest = json.loads((args.raw / "inventory.json").read_text())
    reviews = {}
    for source in args.reviews:
        for name, review in json.loads(source.read_text()).items():
            reviews.setdefault(name, []).append(review)
    by_hash = {}
    for name, entries in reviews.items():
        for review in entries:
            by_hash.setdefault(review.get("sha256"), []).append((name, review))
    for case in manifest["cases"]:
        name = case["id"]
        image = args.output / f"{name}.png"
        cells = args.raw / f"{name}.cells.json"
        if args.reuse_images and cells.is_file() and (
                not image.is_file() or image.stat().st_mtime_ns < cells.stat().st_mtime_ns):
            raise RuntimeError(f"Export the current capture before reusing {name}")
        for suffix in ("txt", "hits.json"):
            source = args.raw / f"{name}.{suffix}"
            if source.is_file():
                shutil.copyfile(source, args.output / source.name)
        for key in ("physical_rgba", "native_cells"):
            case.pop(key, None)
        if image.is_file():
            case["image"] = image.name
            case["sha256"] = hashlib.sha256(image.read_bytes()).hexdigest()
            candidates = reviews.get(name, [])
            matching = [review for review in candidates if review.get("sha256") == case["sha256"]]
            if not matching:
                identical = by_hash.get(case["sha256"], [])
                if identical:
                    matching = [review for _, review in identical]
                    case["visual_review_reused_from"] = list(dict.fromkeys(source for source, _ in identical))
            if matching:
                rank = {"pass": 0, "not-tested": 1, "defect": 2}
                verdict = max((review.get("status", "not-tested") for review in matching),
                              key=lambda status: rank.get(status, 1))
                case["visual_review"] = verdict if verdict in ("pass", "defect", "not-tested") else "not-tested"
                case["review_note"] = " ".join(dict.fromkeys(review.get("note", "") for review in matching))
                if "visual_review_reused_from" in case:
                    case["review_note"] = ("Visual review reused from byte-identical PNG: "
                        + ", ".join(case["visual_review_reused_from"])
                        + ". Route and hit bounds remain independently recorded for this case. "
                        + case["review_note"])
                if case["status"] != "defect":
                    case["status"] = case["visual_review"]
            elif candidates:
                case["review_note"] = "The image changed after review; previous acceptance was not reused."
    manifest["summary"] = {status: sum(case["status"] == status for case in manifest["cases"])
                           for status in ("pass", "defect", "not-tested")}
    (args.output / "inventory.json").write_text(json.dumps(manifest, indent=2) + "\n")
    cards = []
    for case in manifest["cases"]:
        name = html.escape(case["id"])
        picture = (f'<a href="{html.escape(case["image"])}"><img loading="lazy" '
                   f'src="{html.escape(case["image"])}" alt="{name}"></a>') if "image" in case else ""
        reason = html.escape(case.get("review_note", case.get("reason", "")))
        links = f'<a href="{name}.txt">Native text</a> · <a href="{name}.hits.json">Targets</a>' if picture else ""
        cards.append(f'<article data-status="{case["status"]}"><h2>{name}</h2>'
                     f'<p class="status">{case["status"]}</p><p>{reason}</p>{picture}<p>{links}</p></article>')
    document = '''<!doctype html><html lang="en"><meta charset="utf-8">
<title>They Work · UI audit inventory</title><style>
body{margin:24px;font:15px system-ui;background:#f4f1e9;color:#20242a}header{max-width:900px}
input,select{font:inherit;padding:8px;margin:8px}main{display:grid;grid-template-columns:repeat(auto-fit,minmax(520px,1fr));gap:20px}
article{background:white;padding:16px;border:1px solid #aaa;border-radius:6px}h2{font-size:17px;word-break:break-word}
img{display:block;max-width:100%;height:auto;image-rendering:pixelated}p{line-height:1.45}.status{font-weight:700}
article[data-status=defect]{border:3px solid #963220}article[hidden]{display:none}a{color:#124d69}
@media(max-width:600px){main{display:block}article{margin-bottom:16px}body{margin:10px}}
</style><header><h1>UI review inventory</h1><p>Actual Ui frames reconstructed from native cells and physical pixels.
Capture is not visual acceptance. An accepted review is reused only when its image hash is unchanged.
Real terminal transport and provider behaviour are outside these images.</p>
<p>SUMMARY</p><label>Find <input id="query" type="search"></label><label>Status <select id="status">
<option value="">All</option><option>defect</option><option>not-tested</option><option>pass</option></select></label>
<p><a href="inventory.json">Full machine-readable inventory</a></p></header><main>CARDS</main>
<script>function filter(){let q=document.querySelector('#query').value.toLowerCase(),s=document.querySelector('#status').value;
for(let card of document.querySelectorAll('article'))card.hidden=(!card.textContent.toLowerCase().includes(q))||(s&&card.dataset.status!==s)}
document.querySelector('#query').addEventListener('input',filter);document.querySelector('#status').addEventListener('change',filter);</script></html>'''
    document = document.replace("SUMMARY", html.escape(str(manifest["summary"]))).replace("CARDS", "\n".join(cards))
    (args.output / "index.html").write_text(document)
    print(json.dumps(manifest["summary"]))


if __name__ == "__main__":
    main()
