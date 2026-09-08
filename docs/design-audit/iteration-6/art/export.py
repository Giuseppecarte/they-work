#!/usr/bin/env python3
"""Export exact authored pixels and labeled comparisons, never rescale artwork."""
import argparse, hashlib, json, shutil
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
p=argparse.ArgumentParser();p.add_argument('input',type=Path);p.add_argument('output',type=Path);a=p.parse_args()
a.output.mkdir(parents=True,exist_ok=True)
f=Path('/System/Library/Fonts/Menlo.ttc');font=ImageFont.truetype(str(f),12) if f.exists() else ImageFont.load_default(size=12)
manifest={'kind':'Exact Rust compositor artwork; not a terminal screenshot','source_grids':[[48,64],[24,32]],'files':[]}
for source in sorted(a.input.glob('*.ppm')):
 im=Image.open(source).convert('RGB');out=a.output/(source.stem+'.png');im.save(out)
 manifest['files'].append({'path':out.name,'size':im.size,'sha256':hashlib.sha256(out.read_bytes()).hexdigest()})
for mode in ['detail','overview']:
 paths=[a.output/f'{kind}-{mode}-0.png' for kind in ['individual','shared']]
 if all(p.exists() for p in paths):
  ims=[Image.open(p) for p in paths];chart=Image.new('RGB',(640,sum(i.height+24 for i in ims)),'#161e28');d=ImageDraw.Draw(chart);y=0
  for label,im in zip(['Individual: landscape monitor, stand and keyboard','Shared: laptop lid, hinge and keyboard'],ims):d.text((8,y+4),label,font=font,fill='#d5d9d2');chart.paste(im,(0,y+24));y+=im.height+24
  chart.save(a.output/f'compare-{mode}.png')
frames=[]
for i,source in enumerate(sorted((a.input/'motion').glob('frame-*.ppm'))):
 im=Image.open(source).convert('RGB');frame=Image.new('RGB',(im.width,im.height+24),'#161e28');frame.paste(im,(0,0));d=ImageDraw.Draw(frame)
 stage='Back at the workstation' if i==128 else ('Walking to a room anchor' if i<24 else ('Returning to the same workstation' if i>104 else 'Decorative vignette; source facts are unchanged'))
 d.text((8,im.height+4),f'{(8000+i*125)/1000:4.1f}s  {stage}',font=font,fill='#d5d9d2');frames.append(frame)
 if i in [0,12,48,116,128]:frame.save(a.output/f'route-{i:03}.png')
if frames:
 out=a.output/'outbound-action-return.gif';durations=[120 if i%2==0 else 130 for i in range(len(frames))];frames[0].save(out,save_all=True,append_images=frames[1:],duration=durations,loop=0,optimize=True)
 manifest['files'].append({'path':out.name,'frames':len(frames),'duration_ms':sum(durations),'sha256':hashlib.sha256(out.read_bytes()).hexdigest(),'kind':'GIF palette preview; PNG evidence retains RGB'})
for name in ['geometry.txt','motion.csv']:
 if (a.input/name).exists():shutil.copyfile(a.input/name,a.output/name)
known={entry['path'] for entry in manifest['files']}
for output in sorted(a.output.glob('*.png')):
 if output.name not in known:
  with Image.open(output) as im:size=im.size
  manifest['files'].append({'path':output.name,'size':size,'sha256':hashlib.sha256(output.read_bytes()).hexdigest()})
repo=Path(__file__).resolve().parents[4]
sources=['crates/theywork-render/src/living_office.rs']+[f'crates/theywork-render/src/living_office/{name}.rs' for name in ['art','overview','rooms','simulation','workstation']]
manifest['sources']={name:hashlib.sha256((repo/name).read_bytes()).hexdigest() for name in sources}
(a.output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
