#!/usr/bin/env python3
"""Render exported TestBackend cells; these are not physical terminal captures."""
import json
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'docs/design-audit/tmp/python'))
from PIL import Image,ImageDraw,ImageFont
folder=Path(sys.argv[1]) if len(sys.argv)>1 else ROOT/'docs/design-audit/iteration-6/evidence/panel-pilot'
font=ImageFont.truetype('/System/Library/Fonts/Menlo.ttc',15)
for path in sorted(folder.glob('*.json')):
    data=json.loads(path.read_text());image=Image.new('RGB',(data['columns']*10,data['rows']*18));draw=ImageDraw.Draw(image)
    for y,row in enumerate(data['cells']):
        for x,cell in enumerate(row):
            draw.rectangle((x*10,y*18,x*10+9,y*18+17),fill=cell['bg'])
            draw.text((x*10,y*18),cell['text'],font=font,fill=cell['fg'],stroke_width=0)
    image.save(path.with_suffix('.png'))
print('Rendered',len(list(folder.glob('*.json'))),'cell buffers')
