import sys,unittest,io,json
from unittest.mock import patch
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from lock_watch import key_tag,safe_export
class Tests(unittest.TestCase):
 def test_private_keys_are_anonymous_and_domains_distinct(self):
  r={'locktype':'advisory','classid':123,'objid':456};t=key_tag(b'synthetic',r);self.assertEqual(t,key_tag(b'synthetic',r));self.assertNotEqual(t,key_tag(b'other',r));self.assertNotEqual(t,key_tag(b'synthetic',dict(r,locktype='tuple')))
 def test_blocker_edges_keep_alias_and_null_waitstart(self):
  raw={'server_epoch_ns':'1900000000000000000','observer_backend_pid':999,'activities':[{'pid':1,'query_id':'9223372036854775807','blockers':[2],'state':'active'}],'locks':[{'pid':1,'locktype':'advisory','classid':123,'objid':456,'granted':False,'waitstart_epoch_ns':None}]};r=safe_export(raw,b'synthetic',{});self.assertEqual(r['activities'][0]['backend'],r['locks'][0]['backend']);self.assertEqual(r['activities'][0]['query_id'],'9223372036854775807');self.assertIsNone(r['locks'][0]['waitstart_epoch_ns']);self.assertNotIn('classid',r['locks'][0]);self.assertNotIn('objid',r['locks'][0]);self.assertNotIn('pid',r['activities'][0])
 def test_row_guard_and_prepared_blocker_are_not_silently_zero(self):
  raw={'observer_backend_pid':999,'activities':[{'pid':1,'blockers':[0]}]*1001};r=safe_export(raw,b'synthetic',{});self.assertTrue(r['row_guard']['truncated']);self.assertEqual(r['activities'][0]['blockers'],['prepared-transaction-unknown'])
 def test_compact_multiple_records_null_and_escaped_values_keep_single_frame(self):
  raw={'server_epoch_ns':'1900000000000000000','observer_backend_pid':999,'activities':[{'pid':1,'query_id':None,'blockers':[],'wait_event':None},{'pid':2,'query_id':'123','blockers':[1]}],'locks':[{'pid':1,'locktype':'advisory','granted':True,'relation_name':None},{'pid':2,'locktype':'tuple','granted':False,'relation_name':'synthetic\nquote"','waitstart_epoch_ns':None}]}
  line=json.dumps(raw,separators=(',',':'))+'\n';self.assertEqual(len(line.splitlines()),1);r=safe_export(json.loads(io.StringIO(line).readline()),b'synthetic',{});self.assertEqual(len(r['activities']),2);self.assertEqual(len(r['locks']),2);self.assertIsNone(r['activities'][0]['query_id']);self.assertEqual(r['locks'][1]['relation_name'],'synthetic\nquote"');self.assertEqual(r['activities'][1]['blockers'],[r['locks'][0]['backend']])
class FakeProcess:
 def __init__(self):self.stdin=io.StringIO();self.stdout=io.StringIO();self.returncode=1
 def poll(self):return self.returncode
class FailureEvidenceTests(unittest.TestCase):
 def test_failed_owned_psql_retains_exit_and_stderr_without_another_query(self):
  from lock_watch import LockWatch
  def popen(*args,**kw):
   kw['stderr'].write(b'ERROR: synthetic observer statement failed\n');kw['stderr'].flush();return FakeProcess()
  with patch('lock_watch.subprocess.Popen',side_effect=popen):
   watch=LockWatch('synthetic','qadb1234',['00000000-0000-0000-0000-000000000001'])
   with self.assertRaisesRegex(RuntimeError,'exit=1.*synthetic observer statement failed'):watch.tick()
   self.assertEqual(watch.errors,1);self.assertIn('synthetic observer statement failed',watch.close()['stderr_tail'])
if __name__=='__main__':unittest.main()
