#!/usr/bin/env python3
"""Exercise this kit's actual launchers separately from the REPRO smoke."""
import argparse
import gzip
import importlib.util
import json
import platform
import shutil
import sqlite3
import subprocess
import sys
import traceback
from pathlib import Path
from PIL import Image
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('routes', HERE.parent/'docs/check_routes.py')
r = importlib.util.module_from_spec(spec); spec.loader.exec_module(r)
w = r.w


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,default=ROOT/'target/native-macos/release/they-work')
    args=p.parse_args()
    out=HERE/'evidence';out.mkdir(exist_ok=True)
    scratch=ROOT/'docs/design-audit/tmp/iteration-8-study';scratch.mkdir(parents=True,exist_ok=True)
    binary=scratch/'preflight-they-work';shutil.copy2(args.binary,binary)
    folder=scratch/'study-P01'
    result={'status':'running','host':platform.platform(),'python':sys.version,'binary_sha256':w.digest(binary),
            'participant_sessions':0,'baseline_frozen':False,'physical_terminal':'not-tested','real_provider_calls':0,
            'capture':'Actual launcher + executable PTY; simulated Kitty handshake 8x16 when stated; Menlo ANSI replay',
            'conditions':{},'checks':{}}
    def run(script,*arguments):
        command=[sys.executable,str(HERE/script),*map(str,arguments)]
        value=subprocess.run(command,capture_output=True,text=True,timeout=20)
        assert value.returncode==0,(command,value.stdout,value.stderr)
        return value
    def launch(condition,extra=()):
        return [sys.executable,str(HERE/'launch_fixture.py'),'--fixture',str(folder),
                '--binary',str(binary),'--condition',condition,*extra]
    try:
        run('prepare_session.py','P01','--reset')
        denied=subprocess.run(launch('compact',['--doctor']),capture_output=True,text=True,timeout=20)
        assert denied.returncode==2 and 'Freeze the baseline' in denied.stderr
        manifest=scratch/'test-lock.json';manifest.write_text(json.dumps({'locked':True,'binary_sha256':w.digest(binary)}))
        accepted=subprocess.run(launch('compact',['--doctor','--candidate-manifest',str(manifest)]),capture_output=True,text=True,timeout=20)
        assert accepted.returncode==0,accepted.stderr
        manifest.write_text(json.dumps({'locked':True,'binary_sha256':'0'*64}))
        mismatch=subprocess.run(launch('compact',['--doctor','--candidate-manifest',str(manifest)]),capture_output=True,text=True,timeout=20)
        assert mismatch.returncode==2 and 'SHA-256 must match' in mismatch.stderr
        result['checks']['baseline_lock']={'unlocked_rejected':True,'exact_hash_accepted':True,'changed_binary_rejected':True,'test_manifest_only':True}
        stores={name:w.digest(folder/'codex'/name) for name in ['state_5.sqlite','thread_history_1.sqlite']}
        for condition in ['tower','compact','reduced']:
            for local in ['notebook.json','project']:
                (folder/'settings'/local).unlink(missing_ok=True)
            graphics=condition!='compact'
            s=r.Session(binary,folder,[],launcher=launch(condition,['--preflight']),graphics=graphics,columns=120,rows=36)
            try:
                s.wait(lambda:'alpha-shop' in s.text(),'three-project fixture')
                s.action('Tower',b'0')
                s.wait(lambda:'Software tower' in s.text(),'tower')
                assert bool(s.art)==graphics,(condition,bool(s.art))
                prefs=json.loads((folder/'settings/appearance.json').read_text())
                assert prefs['motion']==(condition!='reduced')
                s.capture(out,'launcher-'+condition)
                s.action('All-floor Attention',b'b')
                s.wait(lambda:'APPROVAL NEEDED' in s.text(),'approval observation')
                assert 'ERROR' in s.text()
                s.action('Close Attention',r.ESC)
                s.resize(80,24);s.pump(.5)
                w.find(s,'Draft API guide')
                assert 'Draft API guide' in s.text()
                s.capture(out,'launcher-'+condition+'-brief80')
                exit_result=r.close(s,out,'launcher-'+condition)
                assert {name:w.digest(folder/'codex'/name) for name in stores}==stores
                result['conditions'][condition]={'cells_initial':[120,36],'cells_return':[80,24],
                    'same_source_path':str(folder/'codex'),'source_hashes_unchanged':True,'motion':prefs['motion'],
                    'simulated_kitty':graphics,'pixel_cell_geometry':[8,16],'exit':exit_result}
            finally:
                r.cleanup(s,folder)
        # Execute both fresh semantic variants, including the actual injection helper.
        variants={}
        for variant,title,token in [('A','Draft API guide','API examples now include pagination.'),('B','Draft SDK guide','SDK examples now include authentication.')]:
            run('prepare_session.py','P01','--reset','--variant',variant)
            db=sqlite3.connect(folder/'codex/state_5.sqlite');rows=db.execute('SELECT id,title FROM threads').fetchall();db.close()
            facts=json.loads((folder/'fixture-facts.json').read_text())
            assert [(item[1],item[2]) for item in facts]==rows
            variants[variant]={'tasks':len(rows),'facts_manifest_matches_ids_and_titles':True}
            s=r.Session(binary,folder,[],launcher=launch('compact',['--preflight']),columns=80,rows=24)
            try:
                s.wait(lambda:'alpha-shop' in s.text(),'fresh variant')
                w.find(s,title)
                run('prepare_session.py','P01','--variant',variant,'--add-result')
                s.wait(lambda:token in s.text(),'new synthetic result in selected brief')
                s.capture(out,'return-variant-'+variant)
                variants[variant].update(injected_result_visible=True,title=title,exit=r.close(s,out,'variant-'+variant))
            finally:r.cleanup(s,folder)
        assert variants['A']['tasks']==variants['B']['tasks']==6
        result['checks']['fresh_variants']=variants
        result['status']='pass'
    except Exception:
        result['status']='fail';result['failure']=traceback.format_exc();raise
    finally:
        for raw in out.glob('*.ansi'):
            data = raw.read_bytes()
            raw.with_suffix('.ansi.gz').write_bytes(gzip.compress(data, mtime=0))
            raw.unlink()
        (out/'preflight.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'status':result['status'],'binary_sha256':result['binary_sha256'],'participant_sessions':0}))


if __name__=='__main__':main()
