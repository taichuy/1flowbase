import ast, importlib.util, pathlib, unittest
P=pathlib.Path(__file__).resolve().parents[1]/'sampler.py'
spec=importlib.util.spec_from_file_location('frozen_sampler',P)
sampler=importlib.util.module_from_spec(spec);spec.loader.exec_module(sampler)
ROOTS={'api':10,'postgres':20,'driver':30,'mock':40}
def rows():return [{'pid':pid,'ppid':1,'start_ticks':pid+100} for pid in ROOTS.values()]
class RootFenceTests(unittest.TestCase):
 def test_pins_root_starttime_without_live_reads(self):
  fence=sampler.RootIdentityFence(ROOTS,rows());fence.assert_current(rows())
  self.assertEqual(fence.roots['api'],(10,110))
 def test_rejects_pid_reuse(self):
  fence=sampler.RootIdentityFence(ROOTS,rows());changed=rows();changed[0]['start_ticks']+=1
  with self.assertRaisesRegex(RuntimeError,'changed identity: api'):fence.assert_current(changed)
 def test_rejects_disappearance(self):
  fence=sampler.RootIdentityFence(ROOTS,rows())
  with self.assertRaisesRegex(RuntimeError,'postgres'):fence.assert_current([r for r in rows() if r['pid']!=20])
 def test_rejects_missing_root_at_startup(self):
  with self.assertRaisesRegex(RuntimeError,'absent at startup'):sampler.RootIdentityFence(ROOTS,rows()[1:])
 def test_rejects_unowned_or_duplicate_root(self):
  for roots in [{**ROOTS,'api':1},{**ROOTS,'api':20},{'api':10}]:
   with self.assertRaises(ValueError):sampler.RootIdentityFence(roots,rows())
 def test_descendants_exclude_unrelated_tree(self):
  graph=[{'pid':11,'ppid':10},{'pid':12,'ppid':11},{'pid':21,'ppid':20},{'pid':99,'ppid':1}]
  self.assertEqual(sampler.descendants(graph,10),{10,11,12})
 def test_no_command_execution_or_sensitive_proc_content(self):
  source=P.read_text();tree=ast.parse(source)
  imported={n.name for node in ast.walk(tree) if isinstance(node,ast.Import) for n in node.names}
  self.assertEqual(imported,{'os','sys','time','json','select'})
  forbidden={'system','exec','execv','execve','popen','spawn','fork','ptrace'}
  for node in ast.walk(tree):
   if isinstance(node,ast.Call):
    name=node.func.attr if isinstance(node.func,ast.Attribute) else node.func.id if isinstance(node.func,ast.Name) else ''
    self.assertNotIn(name,forbidden)
  for path in ['/maps','/mem"','/mem\'','/environ','/stack','/sys/kernel','setcap','perf record']:
   self.assertNotIn(path,source)
if __name__=='__main__':unittest.main()
