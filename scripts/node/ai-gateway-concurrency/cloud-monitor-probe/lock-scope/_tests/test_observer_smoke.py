import sys,unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from observer_smoke import validate_snapshots
class Tests(unittest.TestCase):
 def test_empty_snapshots_do_not_prove_loaded_observer_health(self):
  with self.assertRaises(AssertionError):validate_snapshots([{'activities':[],'locks':[]}]*3)
 def test_requires_native_key_wait_and_real_blocker_edge(self):
  good={'activities':[{'backend':'b','blockers':['a'],'wait_event_type':'Lock'}],'locks':[{'native_app_alias':'app_0','granted':False}]}
  validate_snapshots([good]*3)
  with self.assertRaises(AssertionError):validate_snapshots([{**good,'locks':[]}]*3)
if __name__=='__main__':unittest.main()
