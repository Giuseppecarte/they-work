#!/usr/bin/env python3
"""Label the exact PPM exports from the living_gallery Rust example.

No image resampling or smoothing. These are art/compositor exports, not
screenshots of a terminal. Run living_gallery into audit scratch first.
"""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

parser = argparse.ArgumentParser()
parser.add_argument("input", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
font = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 14)
costumes = ["Headphones", "Chef", "Explorer", "Gardener", "Astronaut", "Artist", "Wizard", "Rocker", "Bookworm", "Runner", "Hard hat", "Dinosaur"]
poses = ["Work", "Read", "Search", "Human help", "Coffee", "Stretch", "Water plant", "Paper plane"]
manifest = {"kind": "original pixel artwork, software composition; not terminal screenshots", "character_source": [48, 64], "files": []}
for source in sorted(args.input.glob("*.ppm")):
    if source.stem.startswith("motion-"):
        continue
    image = Image.open(source).convert("RGB")
    draw = ImageDraw.Draw(image)
    if source.stem == "cast":
        for index, title in enumerate(costumes):
            draw.text((24 + index % 4 * 200, 215 + index // 4 * 240), title, font=font, fill="#d5d9d2")
    elif source.stem == "poses":
        for index, title in enumerate(poses):
            draw.text((6 + index * 120, 186), title, font=font, fill="#d5d9d2")
    elif source.stem == "interactions":
        for index,title in enumerate(["Delegate", "Message", "Deliver", "Await team"]):
            draw.text((6+index*120,184),title,font=font,fill="#d5d9d2")
    destination = args.output / (source.stem + ".png")
    image.save(destination)
    manifest["files"].append({"path": destination.name, "width": image.width, "height": image.height, "sha256": hashlib.sha256(destination.read_bytes()).hexdigest()})
motion=[Image.open(path).convert("RGB") for path in sorted(args.input.glob("motion-*.ppm"))]
if motion:
    destination=args.output/"decorative-motion.gif"
    # GIF stores centiseconds. Alternating 120/130 ms preserves the authored
    # 125 ms average and total duration instead of silently speeding playback.
    durations=[120 if index%2==0 else 130 for index in range(len(motion))]
    motion[0].save(destination,save_all=True,append_images=motion[1:],duration=durations,loop=0,optimize=True)
    manifest["files"].append({"path":destination.name,"frames":len(motion),"source_frame_ms":125,"gif_frame_ms":durations,"duration_ms":sum(durations),"sha256":hashlib.sha256(destination.read_bytes()).hexdigest()})
(args.output / "art-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
