#!/usr/bin/env python3
"""Rebuild specimen boards from comparison-final; writes ignored scratch output."""
from pathlib import Path
import json,shutil
from PIL import Image,ImageDraw,ImageFont
root=Path('.');src=root/'target/audit/iteration-11/comparison-final';out=root/'target/audit/iteration-11/retained-visual';out.mkdir(parents=True,exist_ok=True)
font=ImageFont.truetype('scripts/audit_assets/fonts/DejaVuSansMono.ttf',13)
for preset in ['studio','workshop','lab']:
    names=[f'{preset}-p{p}-{kind}-{surface}-80x24' for kind in ['desks','team'] for surface in ['tower','office-auto'] for p in range(4)]
    board=Image.new('RGB',(640*4,410*4),'#0d1828')
    for i,n in enumerate(names):
        im=Image.open(src/'after'/f'{n}.png');board.paste(im,((i%4)*640,(i//4)*410+26));ImageDraw.Draw(board).text(((i%4)*640+8,(i//4)*410+5),n,font=font,fill='#fff4de')
    board.save(out/f'{preset}-all-palettes.png')
names=['studio-p0-team-office-auto-80x24','studio-p0-desks-office-auto-80x24','inspector-now-image-120x36-8x16','inspector-now-image-80x24-8x16','tower-image-192x58-8x16','tower-image-120x36-10x20','office-auto-image-120x36-8x16','attention-image-80x24-8x16','attention-image-120x36-8x16','connections-image-80x24-8x16','connections-image-120x36-8x16','office-auto-native-80x24-8x16','inspector-now-light-80x24-8x16','inspector-now-mono-80x24-8x16','office-auto-256-80x24-8x16','office-auto-image-32x14-8x16']
for stage in ['before','after']:
    d=out/stage;d.mkdir(exist_ok=True)
    for n in names:shutil.copy2(src/stage/f'{n}.png',d/f'{n}.png')
n='inspector-now-image-120x36-8x16';a=Image.open(src/'before'/f'{n}.png');b=Image.open(src/'after'/f'{n}.png');im=Image.new('RGB',(a.width*2,a.height+26),'#0d1828');im.paste(a,(0,26));im.paste(b,(a.width,26));d=ImageDraw.Draw(im);d.text((8,5),'BEFORE / same facts',font=font,fill='#fff4de');d.text((a.width+8,5),'AFTER / warmer materials + focused controls',font=font,fill='#fff4de');im.save(out/'work-panel-comparison.png')
shutil.copy2(src/'comparison.json',out/'comparison.json')
print('Retained 32 complete specimens, 3 full-scene palette contact sheets and one paired work-panel comparison')
