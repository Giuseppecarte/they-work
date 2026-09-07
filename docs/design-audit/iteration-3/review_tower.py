#!/usr/bin/env python3
"""Reproduce the software tower with synthetic local conversations in a native PTY.
Images reconstruct ANSI cells; they are not captured from a terminal window.
"""
import argparse, importlib.util, json, pathlib, re, sqlite3, sys, time
sys.dont_write_bytecode=True
ROOT=pathlib.Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('pty_review',ROOT/'docs/design-audit/iteration-2/review_pty.py')
base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
base.SCRATCH=ROOT/'docs/design-audit/tmp/iteration-3-tower'
original_fixture=base.fixture
WORKERS_PER_FLOOR=3

def floors_fixture(count,running=False):
    project,source=original_fixture(count*WORKERS_PER_FLOOR,False)
    state=sqlite3.connect(source/'state_5.sqlite');history=sqlite3.connect(source/'thread_history_1.sqlite')
    names=['atlas-web','billing-service','customer-onboarding','design-system','event-pipeline','mobile-app']
    for index in range(count*WORKERS_PER_FLOOR):
        floor=index//WORKERS_PER_FLOOR
        path=project.parent/f'{floor+1:02}-{names[floor%len(names)]}'
        (path/'.git').mkdir(parents=True,exist_ok=True)
        state.execute('UPDATE threads SET cwd=?, title=? WHERE id=?',(str(path),['Build account settings','Review the API contract','Update integration tests'][index%3],f'person-{index}'))
        if index%WORKERS_PER_FLOOR==0:
            history.execute("UPDATE thread_turns SET status='inProgress', completed_at=NULL WHERE thread_id=?",(f'person-{index}',))
            history.execute("UPDATE thread_items SET item_type='commandExecution',item_json=? WHERE thread_id=?",(json.dumps({'command':'cargo test --workspace'}),f'person-{index}'))
        if floor%4==3 and index%WORKERS_PER_FLOOR==0:
            old=int(time.time())-240
            state.execute('UPDATE threads SET updated_at=? WHERE id=?',(old,f'person-{index}'))
            history.execute('UPDATE thread_turns SET started_at=? WHERE thread_id=?',(old,f'person-{index}'))
            history.execute('UPDATE thread_items SET created_at_ms=? WHERE thread_id=?',(old*1000,f'person-{index}'))
        if floor%4==2 and index%WORKERS_PER_FLOOR==1:
            completed=int(time.time())+1
            history.execute("UPDATE thread_turns SET status='completed',completed_at=?,error_json=? WHERE thread_id=?",(completed,json.dumps({'message':'Integration test failed'}),f'person-{index}'))
            state.execute('UPDATE threads SET updated_at=? WHERE id=?',(completed,f'person-{index}'))
    state.commit();history.commit();state.close();history.close()
    return project,source
base.fixture=floors_fixture
# The shared harness normally selects a single project. This scenario intentionally
# removes only that filter so every synthetic independent project is collected.
original_popen=base.subprocess.Popen

def tower_popen(args,*a,**kw):
    args=list(args)
    if '--project' in args:
        index=args.index('--project');del args[index:index+2]
    return original_popen(args,*a,**kw)
base.subprocess.Popen=tower_popen

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=pathlib.Path,default=ROOT/'target/native-macos/release/they-work')
    parser.add_argument('--stage',default='tower-final')
    parser.add_argument('--floors',type=int,nargs='+',default=[1,6,20])
    parser.add_argument('--workers-per-floor',type=int,default=3)
    args=parser.parse_args()
    global WORKERS_PER_FLOOR
    WORKERS_PER_FLOOR=max(1,args.workers_per_floor)
    out=ROOT/'docs/design-audit/iteration-3/evidence'/args.stage;out.mkdir(parents=True,exist_ok=True)
    results={}
    def selected_floor(session):
        match=re.search(r'Floor (\d+)/(\d+)', '\n'.join(session.screen.display))
        assert match, 'Tower navigation footer missing'
        return int(match.group(1)),int(match.group(2))
    for count in args.floors:
        session=base.Session(args.binary,count,'half-blocks',motion=False)
        session.key(b'0')
        for width,height in [(80,24),(120,32),(192,58)]:
            previous=selected_floor(session)
            session.resize(width,height)
            assert selected_floor(session)==previous, 'Resize changed the selected project'
            session.key(b'\x1b[H')
            assert selected_floor(session)==(1,count)
            if count>1:
                session.key(b'\x1b[6~')
                assert selected_floor(session)[0]>1, 'PageDown did not advance the floor'
                session.key(b'\x1b[5~')
                assert selected_floor(session)==(1,count), 'PageUp did not restore the first floor'
            session.save(out/f'tower-{count}-{width}x{height}')
            session.key(b'\x1b[F')
            assert selected_floor(session)==(count,count), 'End did not reach the final project'
            session.save(out/f'tower-{count}-{width}x{height}-last')
        results[str(count)]={'navigation': {'home_end_resize': True, 'page_keys': count>1}, **session.finish()}
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(json.dumps(results,indent=2))
if __name__=='__main__':main()
