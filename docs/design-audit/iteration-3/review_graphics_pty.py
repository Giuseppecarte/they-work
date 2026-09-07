#!/usr/bin/env python3
"""Exercise the compiled run loop with a controlled Kitty probe responder.
This verifies emitted bytes and restoration, not a real Kitty window's pixels.
"""
import argparse, fcntl, hashlib, json, os, pathlib, re, select, signal, struct, subprocess, sys, termios, time
ROOT=pathlib.Path(__file__).resolve().parents[3]
SCRATCH=ROOT/'docs/design-audit/tmp/iteration-3/graphics-pty'
OUT=ROOT/'docs/design-audit/iteration-3/evidence/graphics-pty'
IMAGE=re.compile(rb'\x1b_G([^;\x1b]*);([^\x1b]*)\x1b\\')

class Session:
    def __init__(self,binary):
        SCRATCH.mkdir(parents=True,exist_ok=True)
        self.master,self.slave=os.openpty();self.raw=bytearray();self.replied=False
        fcntl.ioctl(self.slave,termios.TIOCSWINSZ,struct.pack('HHHH',24,80,640,384))
        def child():os.setsid();fcntl.ioctl(self.slave,termios.TIOCSCTTY,0)
        env=os.environ.copy();env.update(TERM='xterm-kitty',TERM_PROGRAM='kitty',COLORTERM='truecolor',LANG='en_US.UTF-8')
        for key in ['NO_COLOR','THEYWORK_COLOR','THEYWORK_ENCODING']:env.pop(key,None)
        helper="import subprocess,sys,termios,json,signal; before=termios.tcgetattr(0); p=subprocess.Popen(sys.argv[1:]); signal.signal(signal.SIGTERM,lambda *_:p.send_signal(signal.SIGTERM)); code=p.wait(); after=termios.tcgetattr(0); mask=~getattr(termios,'PENDIN',0);before[3]&=mask;after[3]&=mask;print('PTY_RESULT '+json.dumps({'exit':code,'terminal_restored':before==after}),flush=True)"
        self.process=subprocess.Popen([sys.executable,'-c',helper,str(binary.resolve()),'--demo','--no-save'],stdin=self.slave,stdout=self.slave,stderr=self.slave,env=env,cwd=SCRATCH,preexec_fn=child)
        self.pump(.7)
        if not self.replied or b'a=T' not in self.raw:
            self.abort()
            raise AssertionError(('No successful graphics transmission after the probe',bytes(self.raw[-3000:])))
    def pump(self,duration):
        end=time.monotonic()+duration
        while time.monotonic()<end:
            if select.select([self.master],[],[],max(0,end-time.monotonic()))[0]:
                data=os.read(self.master,1048576)
                if not data:break
                self.raw.extend(data)
                if not self.replied and b'a=q' in self.raw:
                    os.write(self.master,b'\x1b_Gi=31;OK\x1b\\\x1b[6;16;8t\x1b[8;24;80t')
                    self.replied=True
    def key(self,key,duration=.3):os.write(self.master,key);self.pump(duration)
    def stage(self,name,key,heading):
        start=len(self.raw);self.key(key,.5);data=bytes(self.raw[start:])
        (SCRATCH/(name+'.ansi')).write_bytes(data)
        frames=data.split(b'\x1b8')[:-1]
        image_frames=[];repeated=0
        for frame in frames:
            packets=list(IMAGE.finditer(frame))
            images=[p for p in packets if b'a=T' in p.group(1)]
            if images:
                # Every chunk belonging to the image ends before native text.
                last_packet=max(p.end() for p in packets)
                marker=frame.find(heading,last_packet)
                image_frames.append({'image_end_byte':last_packet,'heading_byte':marker,'after_image':marker>=last_packet})
            elif heading in frame:repeated+=1
        assert image_frames,f'{name}: no complete image frame was captured'
        assert all(frame['after_image'] for frame in image_frames),f'{name}: native heading was absent after image transmission: {image_frames}'
        assert repeated,f'{name}: unchanged frames did not repaint native text'
        return {'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'complete_image_frames':len(image_frames),'native_heading_after_every_image':True,'unchanged_frames_with_native_heading':repeated,'sample_order':image_frames[0]}
    def abort(self):
        if self.process.poll() is None:
            self.process.terminate()
            deadline=time.monotonic()+2
            while self.process.poll() is None and time.monotonic()<deadline:self.pump(.1)
            if self.process.poll() is None:self.process.kill();self.process.wait()
    def finish(self):
        self.key(b'q',.3)
        deadline=time.monotonic()+4
        while self.process.poll() is None and time.monotonic()<deadline:self.pump(.1)
        if self.process.poll() is None:self.process.terminate();raise AssertionError('Native UI failed to exit')
        self.pump(.1)
        result=json.loads(bytes(self.raw).rsplit(b'PTY_RESULT ',1)[1].splitlines()[0])
        assert result=={'exit':0,'terminal_restored':True},result
        (SCRATCH/'complete.ansi').write_bytes(self.raw)
        os.close(self.master);os.close(self.slave);return result

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=pathlib.Path,default=ROOT/'target/native-macos/release/they-work');args=parser.parse_args()
    session=Session(args.binary)
    try:
        # Freeze decorative motion so unchanged frames exercise text replay.
        session.key(b's');session.key(b'\x1b[B'*3);session.key(b'\x1b[C');session.key(b'\x1b')
        results={'terminal_cells':[80,24],'cell_pixels':[8,16],'probe':'controlled Kitty direct OK response','binary_sha256':hashlib.sha256(args.binary.read_bytes()).hexdigest()}
        results['help']=session.stage('help',b'?',b'HELP')
        session.key(b'\x1b')
        results['finder']=session.stage('finder',b'/',b'FIND YOUR TEAM')
        session.key(b'\x1b');results['exit']=session.finish()
        OUT.mkdir(parents=True,exist_ok=True);(OUT/'results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(results,indent=2))
    finally:
        session.abort()
if __name__=='__main__':main()
