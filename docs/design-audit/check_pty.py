#!/usr/bin/env python3
"""Exercise the native executable through a real PTY; render captured ANSI cells.
The PNGs are reconstructions, not Terminal.app screenshots.
Run after installing pyte and Pillow in docs/design-audit/tmp/python.
"""
import codecs, fcntl, json, os, pathlib, select, signal, struct, subprocess, sys, termios, time
ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'docs/design-audit/tmp/python'))
import pyte
from PIL import Image, ImageDraw, ImageFont
OUT = ROOT / 'docs/design-audit/evidence/native-pty'
OUT.mkdir(parents=True, exist_ok=True)
FIX = ROOT / 'docs/design-audit/tmp/pty-fixture'
PROJECT = FIX / 'launchpad'
(PROJECT / '.git').mkdir(parents=True, exist_ok=True)
for name in ['connections.json','appearance.json','project']:
    (FIX/'config'/name).unlink(missing_ok=True)
HOME = FIX / 'claude'
(HOME / 'projects/launchpad').mkdir(parents=True, exist_ok=True)
now = int(time.time() * 1000)
lines = [
    {'type':'system','timestamp':now,'sessionId':'design-worker','cwd':str(PROJECT),'customTitle':'Build the launch screen'},
    {'type':'assistant','timestamp':now,'sessionId':'design-worker','cwd':str(PROJECT),'message':{'content':[{'type':'tool_use','id':'question','name':'AskUserQuestion','input':{'question':'Choose the launch date'}}]}}
]
(HOME / 'projects/launchpad/session.jsonl').write_text(''.join(json.dumps(v)+'\n' for v in lines))
BIN = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / 'target/native-macos/release/they-work').resolve()
FONT = '/System/Library/Fonts/Menlo.ttc'
font = ImageFont.truetype(FONT, 15)
colors = {'default':'#e8e2d6','black':'#000000','red':'#aa0000','green':'#00aa00','brown':'#aa5500','blue':'#0000aa','magenta':'#aa00aa','cyan':'#00aaaa','white':'#aaaaaa','brightblack':'#555555','brightwhite':'#ffffff','brightred':'#ff5555','brightgreen':'#55ff55','brightblue':'#5555ff','brightmagenta':'#ff55ff','brightcyan':'#55ffff','yellow':'#ffff55'}
def color(value, background=False):
    if value == 'default': return '#0d0b14' if background else '#e8e2d6'
    return colors.get(value, '#'+value if len(value)==6 else '#999999')
class Session:
    def __init__(self, args, cols=100, rows=32):
        self.cols,self.rows=cols,rows
        self.master,self.slave=os.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack('HHHH',rows,cols,0,0))
        self.original=termios.tcgetattr(self.slave)
        env=os.environ.copy()
        env.update(TERM='xterm-256color',COLORTERM='truecolor',LANG='en_US.UTF-8',THEYWORK_CLAUDE_HOME=str(HOME),THEYWORK_CODEX_HOME=str(FIX/'missing-codex'),THEYWORK_ENCODING='quadrants')
        env.pop('NO_COLOR',None)
        env.pop('THEYWORK_COLOR',None)
        def child():
            os.setsid()
            fcntl.ioctl(self.slave, termios.TIOCSCTTY,0)
        helper = "import json,subprocess,sys,termios,signal; original=termios.tcgetattr(0); p=subprocess.Popen(sys.argv[1:]); signal.signal(signal.SIGTERM,lambda *_:p.send_signal(signal.SIGTERM)); code=p.wait(); after=termios.tcgetattr(0); pending=getattr(termios,'PENDIN',0); kernel_pending=bool(after[3]&pending); original[3]&=~pending; after[3]&=~pending; print('PTY_RESULT '+json.dumps({'exit':code,'terminal_restored':after==original,'kernel_pending_input':kernel_pending}),flush=True)"
        self.process=subprocess.Popen([sys.executable,'-c',helper,str(BIN),*args],stdin=self.slave,stdout=self.slave,stderr=self.slave,env=env,cwd=FIX,preexec_fn=child)
        self.screen=pyte.Screen(cols,rows)
        self.stream=pyte.Stream(self.screen)
        self.decoder=codecs.getincrementaldecoder('utf8')('replace')
        self.raw=bytearray()
        self.pump(.7)
    def pump(self, duration=.3):
        end=time.monotonic()+duration
        while time.monotonic()<end:
            if select.select([self.master],[],[],max(0,end-time.monotonic()))[0]:
                data=os.read(self.master,65536)
                if not data: break
                self.raw.extend(data)
                self.stream.feed(self.decoder.decode(data))
    def key(self, value):
        os.write(self.master,value)
        self.pump(.5)
    def shot(self, name, contains=None):
        text='\n'.join(self.screen.display)
        if contains: assert contains in text,(name,contains,text)
        (OUT/(name+'.txt')).write_text('\n'.join(line.rstrip() for line in text.splitlines()).rstrip()+'\n')
        image=Image.new('RGB',(self.cols*10+16,self.rows*20+16),'#0d0b14')
        draw=ImageDraw.Draw(image)
        for y in range(self.rows):
            for x in range(self.cols):
                cell=self.screen.buffer[y][x]
                bg,fg=color(cell.bg,True),color(cell.fg)
                if cell.reverse: bg,fg=fg,bg
                draw.rectangle((8+x*10,8+y*20,17+x*10,27+y*20),fill=bg)
                draw.text((8+x*10,8+y*20),cell.data,font=font,fill=fg)
        image.save(OUT/(name+'.png'))
    def finish(self, send_signal=False):
        if send_signal:
            self.process.send_signal(signal.SIGTERM)
        else: self.key(b'q')
        self.pump(.3)
        try: code=self.process.wait(timeout=4)
        except subprocess.TimeoutExpired:
            self.process.kill();raise AssertionError('UI did not exit')
        marker=bytes(self.raw).rsplit(b'PTY_RESULT ',1)[1].splitlines()[0]
        report=json.loads(marker)
        code=report['exit']
        restored=report['terminal_restored']
        assert restored,('terminal settings not restored',report)
        assert b'\x1b[?1049l' in self.raw,'alternate screen not restored'
        if not send_signal: assert code==0,bytes(self.raw[-500:])
        os.close(self.master);os.close(self.slave)
        return {'exit':code,'terminal_restored':restored,'alternate_screen_restored':True}
results={}
s=Session(['--setup','--config-dir',str(FIX/'config')])
s.shot('01-connections', 'Connect your team')
s.key(b'\r')
s.shot('02-tower','launchpad')
s.key(b'\r');s.shot('03-floor','launchpad')
s.key(b'\r');s.shot('04-desk','Build the launch screen')
s.key(b'w');s.shot('05-character','CHARACTER')
before_count='\n'.join(s.screen.display).count('Choose the launch date')
s.key(b'c');s.shot('06-reconnect','Connect your team')
s.key(b'\x1b')
assert '\n'.join(s.screen.display).count('Choose the launch date') == before_count,'cancel replayed conversation history'
s.key(b's');s.shot('07-settings')
s.key(b'\x1b');results['setup-connect-inspect-character-reconnect-settings-quit']=s.finish()
assert (FIX/'config/connections.json').exists()
assert (FIX/'config/appearance.json').exists()
results['persistence']={'sources':True,'appearance':True}
s=Session(['--config-dir',str(FIX/'config')]);s.shot('08-restored','launchpad');results['restore']=s.finish()
s=Session(['--demo'],cols=80,rows=24);s.shot('09-demo-80x24');results['signal']=s.finish(True)
s=Session(['--setup'],cols=40,rows=16);s.shot('10-connections-40x16','Enter connect');results['compact']=s.finish()
s=Session(['--config-dir',str(FIX/'config')]);s.key(b'c');s.key(b' ');s.key(b'\r');s.shot('11-all-disconnected')
results['disconnect-all']=s.finish()
choices=json.loads((FIX/'config/connections.json').read_text())
assert not choices['claude'] and not choices['codex']
s=Session(['--config-dir',str(FIX/'config')]);s.key(b'c');s.key(b' ');s.key(b'\r')
real_floor=(FIX/'config/project').read_text() if (FIX/'config/project').exists() else ''
s.key(b'c');s.key(b'd');s.shot('12-demo-transition')
s.key(b'c');s.shot('13-demo-connections','Choices will be remembered')
s.key(b'\r');s.shot('14-back-to-live','launchpad')
results['live-demo-live']=s.finish()
assert json.loads((FIX/'config/connections.json').read_text())['claude']
assert (FIX/'config/project').read_text() == real_floor,'demo replaced the real selected project'
(OUT/'results.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps(results,indent=2))
