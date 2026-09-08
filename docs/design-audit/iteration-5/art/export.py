#!/usr/bin/env python3
"""Annotate exact Rust art exports; no sprite resampling. GIF is a palette preview."""
import argparse, hashlib, json, shutil
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
p=argparse.ArgumentParser();p.add_argument('input',type=Path);p.add_argument('output',type=Path);a=p.parse_args()
a.output.mkdir(parents=True,exist_ok=True)
font_path=Path('/System/Library/Fonts/Menlo.ttc')
font=ImageFont.truetype(str(font_path),12) if font_path.exists() else ImageFont.load_default(size=12)
cast=['Headphones','Chef','Explorer','Gardener','Astronaut','Artist','Wizard','Rocker','Bookworm','Runner','Hard hat','Dinosaur']
manifest={'kind':'Exact Rust artwork at integer sizes, with labels; not terminal screenshots','source_grids':[[48,64],[24,32]],'files':[]}
for f in sorted(a.input.glob('*.ppm')):
    im=Image.open(f).convert('RGB');d=ImageDraw.Draw(im)
    if f.stem=='cast-twelve-authored-grids':
        for i,name in enumerate(cast):
            x,y=i%6*144,i//6*300
            d.text((x+14,y+140),name,font=font,fill='#d5d9d2')
            d.text((x+15,y+222),'2x      help',font=font,fill='#aabdc4')
            d.text((x+52,y+277),'1x',font=font,fill='#aabdc4')
    elif f.stem=='front-and-three-quarter-pilot':
        for i in range(9):
            d.text((i*96+8,146),['Front','3/4 right','3/4 left'][i%3],font=font,fill='#d5d9d2')
        d.text((8,265),'Authored frontal face/torso for human requests; directional poses remain 3/4.',font=font,fill='#aabdc4')
    elif f.stem=='gestures-at-double-size':
        for i,name in enumerate(['Working','Reading','Delegate','Deliver','Message','Human help','Coffee']):
            d.text((i*120+8,143),name,font=font,fill='#d5d9d2')
        d.text((8,260),'48x64 detail above; independently authored 24x32 overview below. Both 2x.',font=font,fill='#aabdc4')
    out=a.output/(f.stem+'.png');im.save(out)
    manifest['files'].append({'path':out.name,'width':im.width,'height':im.height,'sha256':hashlib.sha256(out.read_bytes()).hexdigest()})
frames=[]
for i,f in enumerate(sorted((a.input/'motion').glob('frame-*.ppm'))):
    source=Image.open(f).convert('RGB');im=Image.new('RGB',(source.width,source.height+24),'#1c222d');im.paste(source,(0,0));d=ImageDraw.Draw(im)
    stage='Walking to a real corner' if i<24 else ('Returning to the same desk' if i>104 else 'Decorative vignette; provider state is unchanged')
    d.text((8,source.height+4),f'{min(8000+i*125,23999)/1000:4.1f}s  {stage}',font=font,fill='#d5d9d2')
    frames.append(im)
    if i in [0,12,48,116,128]:im.save(a.output/f'route-{i:03}.png')
if frames:
    out=a.output/'outbound-action-return.gif';durations=[120 if i%2==0 else 130 for i in range(len(frames))]
    frames[0].save(out,save_all=True,append_images=frames[1:],duration=durations,loop=0,optimize=True)
    manifest['files'].append({'path':out.name,'frames':len(frames),'duration_ms':sum(durations),'source_cadence_ms':125,'gif':'palette approximation; PNGs retain full RGB','sha256':hashlib.sha256(out.read_bytes()).hexdigest()})
shutil.copyfile(a.input/'motion.csv',a.output/'motion.csv')
(a.output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
