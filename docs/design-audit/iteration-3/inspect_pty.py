#!/usr/bin/env python3
"""Inspect native Desk/Phone using synthetic, isolated Codex SQLite records.
The PNGs are ANSI-cell replays, not screenshots of a terminal application.
"""
import argparse, importlib.util, json, pathlib, sqlite3, time
ROOT=pathlib.Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('review', ROOT/'docs/design-audit/iteration-2/review_pty.py')
r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
r.SCRATCH=ROOT/'docs/design-audit/tmp/iteration-3'
base_fixture=r.fixture
REQUEST='deploy --project project-production-eu-west --require-backup verified-2026-09-07 --migration customer-account-id-index'
NAMES=['Review the release plan for database migration and production cutover', 'Investigate the payment service connection refused after deploy', 'Verify the customer import that stopped reporting progress', 'Review the accessibility findings and keyboard navigation', 'Prepare release notes for the customer dashboard']
def fixture(count,running=False):
    project, source=base_fixture(5, running)
    now=int(time.time()*1000);old=now-240000
    state=sqlite3.connect(source/'state_5.sqlite');h=sqlite3.connect(source/'thread_history_1.sqlite')
    state.execute('ALTER TABLE threads ADD COLUMN thread_source TEXT')
    state.execute('CREATE TABLE thread_spawn_edges(parent_thread_id TEXT,child_thread_id TEXT,status TEXT)')
    h.execute('DELETE FROM thread_items');h.execute('DELETE FROM thread_turns')
    for i,title in enumerate(NAMES):
        at=old if i in (0,2) else now;worker=f'person-{i}'
        state.execute('UPDATE threads SET title=?,updated_at=? WHERE id=?',(title,at//1000,worker))
        h.execute('INSERT INTO thread_turns VALUES (?,?,?,?,?,?,?)',(worker,'turn','inProgress' if i in (0,2,3) else 'completed',at//1000,None if i in (0,2,3) else at//1000,None,json.dumps({'message':'Connection refused at payments.internal:443; retry exhausted.'}) if i==1 else None))
        items=[(at-20000,'commandExecution',{'command':'cargo test --workspace','status':'completed','exitCode':0}), (at-10000,'fileChange',{'path':'src/payments/connection.rs','added':12,'removed':3}), (at-5000,'agentMessage',{'text':'Verified the test outcome and recorded the migration checklist.'})]
        items += [(at,'commandExecution',{'command':REQUEST,'status':'inProgress'})] if i==0 else [(at,'commandExecution',{'command':'old-import --customer-file customers.csv','status':'inProgress'})] if i==2 else []
        for n,(stamp,kind,payload) in enumerate(items):h.execute('INSERT INTO thread_items VALUES (?,?,?,?,?,?)',(worker,'turn',f'item-{n}',stamp,kind,json.dumps(payload)))
    state.execute('INSERT INTO threads VALUES (?,?,?,?,?,?,?,?,?,?)',('assessor','/synthetic',old//1000,old//1000,str(project),'Internal approval review',0,None,0,'approval'))
    state.execute('INSERT INTO thread_spawn_edges VALUES (?,?,?)',('person-0','assessor','active'))
    state.commit();h.commit();state.close();h.close();return project,source
r.fixture=fixture

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--binary',type=pathlib.Path,required=True);ap.add_argument('--stage',required=True);args=ap.parse_args()
    out=ROOT/'docs/design-audit/iteration-3/evidence'/args.stage;out.mkdir(parents=True,exist_ok=True)
    s=r.Session(args.binary,5,'apple-terminal');s.key(b'1');s.key(b'\r')
    for cols,rows in [(80,24),(120,36)]:s.resize(cols,rows);s.save(out/f'desk-request-{cols}x{rows}')
    s.resize(80,24)
    s.key(b'\x1b[H');s.save(out/'desk-oldest-80x24')
    if args.stage!='before':assert 'Conversation:' in '\n'.join(s.screen.display), 'Home did not reach complete context'
    s.key(b'\x1b[F');s.save(out/'desk-latest-80x24')
    if args.stage!='before':assert 'NOW' in '\n'.join(s.screen.display), 'End did not reach current state'
    s.key(b'p')
    for channel in [1,2,3,4]:
        s.key(str(channel).encode())
        for cols,rows in [(80,24),(120,36)]:s.resize(cols,rows);s.save(out/f'phone-{channel}-{cols}x{rows}')
    s.key(b'2');s.resize(80,24)
    s.key(b'\x1b[B');s.save(out/'phone-request-selected-80x24')
    s.key(b'\x1b[B');s.save(out/'phone-silent-selected-80x24')
    if args.stage!='before':
        text='\n'.join(s.screen.display)
        assert 'No approval was identified' in text, 'Silence is not an approval request'
        assert 'Waiting; no approval' not in text
    s.key(b'\x1b');s.key(b'\x1b')
    result=s.finish();(out/'results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
if __name__=='__main__':main()
