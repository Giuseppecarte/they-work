#!/usr/bin/env python3
"""Compose actual UI RGBA and native masks using Menlo font glyphs.

The pixel art is copied 1:1, without sampling or rescaling. This reconstruction
checks layout and native/image ordering, not a physical terminal implementation.
"""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

parser=argparse.ArgumentParser()
parser.add_argument("input",type=Path)
parser.add_argument("output",type=Path)
args=parser.parse_args()
args.output.mkdir(parents=True,exist_ok=True)
regular=ImageFont.truetype("/System/Library/Fonts/Menlo.ttc",13,index=0)
bold=ImageFont.truetype("/System/Library/Fonts/Menlo.ttc",13,index=1)
manifest={"kind":"Ui TestBackend + exact physical RGBA + native mask; Menlo/Pillow replay, not a terminal screenshot","files":[]}
for source in sorted(args.input.glob("*.cells.json")):
    data=json.loads(source.read_text())
    cell_width,cell_height=data.get("cell_width",8),data.get("cell_height",16)
    font_size=max(1,round(13*cell_height/16))
    regular=ImageFont.truetype("/System/Library/Fonts/Menlo.ttc",font_size,index=0)
    bold=ImageFont.truetype("/System/Library/Fonts/Menlo.ttc",font_size,index=1)
    width,height=data["columns"]*cell_width,data["rows"]*cell_height
    image=Image.new("RGB",(width,height),"#000000")
    draw=ImageDraw.Draw(image)
    for y,row in enumerate(data["cells"]):
        for x,cell in enumerate(row):draw.rectangle((x*cell_width,y*cell_height,(x+1)*cell_width-1,(y+1)*cell_height-1),fill=cell["bg"])
    name=source.name.removesuffix(".cells.json")
    if data["image"]:
        box=data["image"]
        art=Image.frombytes("RGBA",(box["pixel_width"],box["pixel_height"]),(args.input/(name+".rgba")).read_bytes())
        image.paste(art,(box["x"]*cell_width,box["y"]*cell_height),art)
    draw=ImageDraw.Draw(image)
    for y,row in enumerate(data["cells"]):
        for x,cell in enumerate(row):
            if not cell["native"]:continue
            draw.rectangle((x*cell_width,y*cell_height,(x+1)*cell_width-1,(y+1)*cell_height-1),fill=cell["bg"])
            draw.text((x*cell_width,y*cell_height-1),cell["text"],font=bold if cell["bold"] else regular,fill=cell["fg"])
    destination=args.output/(name+".png")
    image.save(destination)
    manifest["files"].append({"path":destination.name,"width":width,"height":height,"sha256":hashlib.sha256(destination.read_bytes()).hexdigest()})
(args.output/"ui-manifest.json").write_text(json.dumps(manifest,indent=2)+"\n")
