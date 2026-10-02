import sys,unittest
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
if __name__=='__main__':unittest.main()
