#!/usr/bin/env python3
"""Resize the native app against synthetic Codex data and capture its ANSI output.
PNGs/GIFs reconstruct terminal cells; they are not Terminal.app screenshots.
"""
import argparse, codecs, fcntl, json, os, pathlib, select, signal, sqlite3, struct, subprocess, sys, termios, time
ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/design-audit/tmp/python'))
import pyte
from PIL import Image, ImageDraw, ImageFont
SCRATCH = ROOT/'docs/design-audit/tmp/iteration-2'
EVIDENCE = ROOT/'docs/design-audit/iteration-2/evidence'
FONT = ImageFont.truetype('/System/Library/Fonts/Menlo.ttc', 15)
CW, CH = 10, 18
QUADS = dict(zip(' ▘▝▀▖▌▞▛▗▚▐▜▄▙▟█', range(16)))
SEXTANTS = [n for n in range(1,63) if n not in (21,42)]
COLORS={'default':'#e8e2d6','black':'#000000','red':'#aa0000','green':'#00aa00','brown':'#aa5500','blue':'#0000aa','magenta':'#aa00aa','cyan':'#00aaaa','white':'#aaaaaa','brightblack':'#555555','brightwhite':'#ffffff','brightred':'#ff5555','brightgreen':'#55ff55','brightblue':'#5555ff','brightmagenta':'#ff55ff','brightcyan':'#55ffff','yellow':'#ffff55'}
def color(value, bg=False):
    if value=='default':return '#0d0b14' if bg else '#e8e2d6'
    return COLORS.get(value, '#'+value if len(value)==6 else '#888888')
def image_of(screen, literal_font=False):
    im=Image.new('RGB',(screen.columns*CW,screen.lines*CH),'#0d0b14');d=ImageDraw.Draw(im)
    for y in range(screen.lines):
        for x in range(screen.columns):
            c=screen.buffer[y][x]; bg,fg=color(c.bg,True),color(c.fg)
            if c.reverse:bg,fg=fg,bg
            ox,oy=x*CW,y*CH
            d.rectangle((ox,oy,ox+CW-1,oy+CH-1),fill=bg)
            symbol=c.data
            if not literal_font and symbol in QUADS:
                mask=QUADS[symbol]
                for bit in range(4):
                    if mask&(1<<bit):
                        x0,y0=ox+(bit%2)*CW//2,oy+(bit//2)*CH//2
                        d.rectangle((x0,y0,x0+CW//2-1,y0+CH//2-1),fill=fg)
            elif not literal_font and len(symbol)==1 and 0x1fb00<=ord(symbol)<=0x1fb3b:
                mask=SEXTANTS[ord(symbol)-0x1fb00]
                for bit in range(6):
                    if mask&(1<<bit):
                        x0,y0=ox+(bit%2)*CW//2,oy+(bit//2)*CH//3
                        d.rectangle((x0,y0,x0+CW//2-1,y0+CH//3-1),fill=fg)
            else:d.text((ox,oy),symbol,font=FONT,fill=fg)
    return im

def fixture(count, running=False):
    folder=SCRATCH/f'fixture-{count}-{int(running)}';folder.mkdir(parents=True,exist_ok=True)
    project=folder/'they-work';(project/'.git').mkdir(parents=True,exist_ok=True)
    source=folder/'codex';source.mkdir(exist_ok=True)
    for name in ['state_5.sqlite','thread_history_1.sqlite']:(source/name).unlink(missing_ok=True)
    now=int(time.time()*1000)
    state=sqlite3.connect(source/'state_5.sqlite');history=sqlite3.connect(source/'thread_history_1.sqlite')
    state.execute('CREATE TABLE threads (id TEXT, rollout_path TEXT, created_at INTEGER, updated_at INTEGER, cwd TEXT, title TEXT, tokens_used INTEGER, git_branch TEXT, archived INTEGER)')
    history.executescript('CREATE TABLE thread_items (thread_id TEXT, turn_id TEXT, item_id TEXT, created_at_ms INTEGER, item_type TEXT, item_json TEXT); CREATE TABLE thread_turns (thread_id TEXT, turn_id TEXT, status TEXT, started_at INTEGER, completed_at INTEGER, duration_ms INTEGER, error_json TEXT);')
    for i in range(count):
        worker=f'person-{i}';title=['Design the office','Review onboarding flow','Ship the release'][i%3]+(f' #{i+1:02}' if count>3 else '')
        state.execute('INSERT INTO threads VALUES (?,?,?,?,?,?,?,?,?)',(worker,'/synthetic',now//1000,now//1000,str(project),title,4200+i*120,'main',0))
        status='inProgress' if running else 'completed'
        history.execute('INSERT INTO thread_turns VALUES (?,?,?,?,?,?,?)',(worker,'turn',status,now//1000,None if running else now//1000,None,None))
        kind='commandExecution' if running else 'agentMessage'
        payload={'command':'cargo test --workspace'} if running else {'text':'Review finished. Ready for the next task.'}
        history.execute('INSERT INTO thread_items VALUES (?,?,?,?,?,?)',(worker,'turn','item',now,kind,json.dumps(payload)))
    state.commit();history.commit();state.close();history.close()
    return project,source

class Session:
    def __init__(self,binary,count,encoding,running=False,motion=True):
        project,source=fixture(count,running)
        self.cols,self.rows=192,58;self.master,self.slave=os.openpty();self.raw=bytearray()
        self.screen=pyte.Screen(self.cols,self.rows);self.stream=pyte.Stream(self.screen);self.decoder=codecs.getincrementaldecoder('utf8')('replace')
        fcntl.ioctl(self.slave,termios.TIOCSWINSZ,struct.pack('HHHH',self.rows,self.cols,0,0))
        def child():os.setsid();fcntl.ioctl(self.slave,termios.TIOCSCTTY,0)
        env=os.environ.copy();env.update(TERM='xterm-256color',COLORTERM='truecolor',LANG='en_US.UTF-8',THEYWORK_ENCODING=encoding)
        env.pop('NO_COLOR',None);env.pop('THEYWORK_COLOR',None)
        if encoding=='apple-terminal':
            env['TERM_PROGRAM']='Apple_Terminal';env.pop('COLORTERM',None);env.pop('THEYWORK_ENCODING',None)
        config=project.parent/'config';config.mkdir(exist_ok=True)
        (config/'appearance.json').write_text(json.dumps({'motion':motion,'projection':'isometric'}))
        args=[str(binary.resolve()),'--sources','codex','--codex-home',str(source),'--project',str(project),'--view','iso','--config-dir',str(config)]
        helper="import subprocess,sys,termios,json,signal; before=termios.tcgetattr(0); p=subprocess.Popen(sys.argv[1:]); signal.signal(signal.SIGTERM,lambda *_:p.send_signal(signal.SIGTERM)); code=p.wait(); after=termios.tcgetattr(0); mask=~getattr(termios,'PENDIN',0);before[3]&=mask;after[3]&=mask;print('PTY_RESULT '+json.dumps({'exit':code,'terminal_restored':before==after}),flush=True)"
        self.process=subprocess.Popen([sys.executable,'-c',helper,*args],stdin=self.slave,stdout=self.slave,stderr=self.slave,env=env,cwd=project,preexec_fn=child)
        self.pump(.6)
        assert self.process.poll() is None,bytes(self.raw[-2000:])
    def pump(self,duration=.25):
        end=time.monotonic()+duration
        while time.monotonic()<end:
            if select.select([self.master],[],[],max(0,end-time.monotonic()))[0]:
                data=os.read(self.master,1048576)
                if not data:break
                self.raw.extend(data);self.stream.feed(self.decoder.decode(data))
    def key(self,key):os.write(self.master,key);self.pump()
    def resize(self,cols,rows):
        self.cols,self.rows=cols,rows;self.screen.resize(lines=rows,columns=cols)
        fcntl.ioctl(self.slave,termios.TIOCSWINSZ,struct.pack('HHHH',rows,cols,0,0));os.killpg(self.process.pid,signal.SIGWINCH);self.pump(.35)
    def save(self,path):
        path.parent.mkdir(parents=True,exist_ok=True)
        image_of(self.screen).save(path.with_suffix('.png'))
        cells=[]
        for y in range(self.rows):
            row=[]
            for x in range(self.cols):
                c=self.screen.buffer[y][x];fg,bg=color(c.fg),color(c.bg,True)
                if c.reverse:fg,bg=bg,fg
                row.append({'text':c.data,'fg':fg,'bg':bg,'bold':c.bold})
            cells.append(row)
        path.with_suffix('.cells.json').write_text(json.dumps({'columns':self.cols,'rows':self.rows,'cells':cells}))
        path.with_suffix('.txt').write_text('\n'.join(line.rstrip() for line in self.screen.display).rstrip()+'\n')
    def finish(self):
        self.key(b'q')
        deadline=time.monotonic()+4
        # Keep consuming ANSI while the UI exits. Waiting without reading can
        # fill the PTY output buffer and block an otherwise healthy renderer.
        while self.process.poll() is None and time.monotonic()<deadline:self.pump(.1)
        self.pump(.1)
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGTERM);self.pump(.2)
            if self.process.poll() is None:self.process.kill()
            raise AssertionError('UI did not exit while the PTY was drained')
        marker=bytes(self.raw).rsplit(b'PTY_RESULT ',1)[1].splitlines()[0];result=json.loads(marker)
        os.close(self.master);os.close(self.slave)
        assert result['exit']==0 and result['terminal_restored'],result
        return result

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--binary',type=pathlib.Path,default=ROOT/'target/native-macos/release/they-work');ap.add_argument('--stage',default='after');ap.add_argument('--encodings',nargs='+',default=['sextants']);ap.add_argument('--workers',type=int,nargs='+',default=[1,3,20]);ap.add_argument('--motion',action='store_true');ap.add_argument('--surfaces',action='store_true');ap.add_argument('--sizes',nargs='+',default=['80x24','120x32','192x58','240x70','110x80']);args=ap.parse_args()
    out=EVIDENCE/args.stage;out.mkdir(parents=True,exist_ok=True);results={}
    for encoding in args.encodings:
        for count in args.workers:
            s=Session(args.binary,count,encoding)
            for view in ['iso','top','side']:
                for size in args.sizes:
                    s.resize(*map(int,size.split('x')));s.save(out/f'{encoding}-{count}-{view}-{size}')
                s.key(b'v')
            results[f'{encoding}-{count}-resize']=s.finish()
    if args.surfaces:
        s=Session(args.binary,3,args.encodings[0])
        for name,enter,leave in [('tower',b'0',b'\r'),('desk',b'\r',b'\x1b'),('phone',b'p',b'\x1b'),('settings',b's',b'\x1b'),('help',b'?',b'\x1b'),('sources',b'c',b'\x1b')]:
            s.key(enter)
            for size in ['80x24','192x58','110x80']:
                s.resize(*map(int,size.split('x')));s.save(out/f'surface-{name}-{size}')
            s.key(leave)
        results['other-surfaces']=s.finish()
    if args.motion:
        for running in [False,True]:
            for motion in [True,False]:
                s=Session(args.binary,3,'sextants',running=running,motion=motion);s.resize(120,36)
                # Exclude UI clocks/status chrome: judge actual room/people artwork.
                frames=[];hashes=[];s.pump(.4)
                for i in range(18):
                    s.pump(.25);im=image_of(s.screen);frames.append(im);hashes.append(hash(im.crop((0,CH*4,im.width,im.height-CH*3)).tobytes()))
                label=f"motion-{'working' if running else 'idle'}-{'on' if motion else 'off'}"
                frames[0].save(out/(label+'.gif'),save_all=True,append_images=frames[1:],duration=250,loop=0)
                results[label]={'distinct_room_frames':len(set(hashes)),**s.finish()}
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(results,indent=2))
if __name__=='__main__':main()
