import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import audit
import incremental
import sampling


class AuditTests(unittest.TestCase):
    def test_real_linker_driver_and_actual_linker_names_are_distinct(self):
        for name in ['cc', 'x86_64-linux-gnu-gcc-13', 'collect2', 'clang++-18']:
            self.assertEqual(sampling.process_role('/usr/bin/' + name), 'linker-driver')
        for name in ['ld', 'x86_64-linux-gnu-ld.bfd', 'rust-lld', 'ld.lld', 'lld']:
            self.assertEqual(sampling.process_role('/usr/bin/' + name), 'linker')
        self.assertEqual(sampling.process_role('/toolchain/bin/rustc'), 'rustc')
        self.assertIsNone(sampling.process_role('/usr/bin/python3'))

    def test_exact_workloads_and_roles(self):
        build = ['cargo', 'build', '--locked', '--offline', '-p', 'api-server']
        self.assertEqual(audit.command('dev2', 'baseline'), build)
        self.assertEqual(audit.command('dev10', 'candidate'), build)
        self.assertEqual(audit.command('test2', 'candidate'), ['cargo', 'test', '--locked', '--offline', '-p', 'api-server', '--no-run'])
        self.assertEqual(audit.command('incremental', 'candidate'), build + ['--bin', 'api-server'])
        for role, workload in [('unknown', 'dev2'), ('baseline', 'test2'), ('candidate', 'unknown'), ('instrumentation', 'dev2')]:
            with self.assertRaises(ValueError):
                audit.command(workload, role)
        with self.assertRaises(ValueError):
            audit.command('external', 'candidate')

    def test_before_records_large_rss_candidate_rejects_it(self):
        row = {'crate': 'api_server', 'kind': 'bin,', 'maxrss_kib': 9 * 1024 ** 2}
        measured = {'returncode': 0, 'error': None, 'cancel_signals': [],
                    'cleanup': {'principal_reaped': True, 'process_group_empty': True},
                    'memory_events_delta': {'oom_kill': 0}, 'anon_peak_bytes': 9 * audit.GIB}
        self.assertTrue(all(audit.checks('baseline', 'dev2', measured, [row]).values()))
        result = audit.checks('candidate', 'dev2', measured, [row])
        self.assertFalse(result['all_observed_rustc_invocations_at_most_4_gib'])
        self.assertFalse(result['anonymous_peak_limit'])
        self.assertFalse(result['api_server_link_descendant_upper_bound_at_most_3_9_gib'])
        measured['sampled_processes'] = [{'role': 'rustc', 'hwm_kib': 5 * 1024 ** 2}]
        low_row = dict(row, maxrss_kib=1024)
        self.assertFalse(audit.checks('candidate', 'external', measured, [low_row])['observed_direct_rustc_hwm_at_most_4_gib'])
        self.assertTrue(all(audit.checks('baseline', 'dev2', measured, [row]).values()))
        measured['returncode'] = 7
        self.assertFalse(audit.checks('baseline', 'dev2', measured, [row])['workload_completed'])
        measured['returncode'] = 0
        measured['memory_events_delta'] = {'oom': 1}
        self.assertFalse(audit.checks('baseline', 'dev2', measured, [row])['workload_completed'])

    def test_campaign_claims_do_not_retry_failed_cold_slots(self):
        with tempfile.TemporaryDirectory() as temporary:
            file = Path(temporary) / 'campaign.json'
            audit.claim_campaign(file, 'baseline', 'dev2', 'before', Path('before'))
            audit.claim_campaign(file, 'candidate', 'dev2', 'after', Path('after'))
            with self.assertRaises(ValueError):
                audit.claim_campaign(file, 'candidate', 'dev2', 'after', Path('retry'))
            with self.assertRaises(ValueError):
                audit.claim_campaign(file, 'candidate', 'test2', 'different', Path('changed'))
            audit.claim_campaign(file, 'candidate', 'dev10', 'after', Path('ten'))
            audit.claim_campaign(file, 'candidate', 'test2', 'after', Path('test'))
            self.assertEqual(len(json.loads(file.read_text())['claims']), 4)

    def test_volume_deduplicates_hardlinks_and_does_not_follow_symlinks(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            target = base / 'target'
            target.mkdir()
            (target / 'one').write_bytes(b'a' * 8192)
            os.link(target / 'one', target / 'two')
            (base / 'external').write_bytes(b'x' * 100000)
            (target / 'link').symlink_to(base / 'external')
            result = sampling.volume(target)
            self.assertEqual(result['symlinks'], 1)
            self.assertEqual(result['unique_inodes'], 3)
            self.assertEqual(result['apparent_bytes'], target.stat().st_size + 8192 + (target / 'link').lstat().st_size)

    def test_nonzero_child_wait4_and_cleanup_without_cargo(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            (base / 'memory.events').write_text('oom 0\noom_kill 0\n')
            (base / 'memory.stat').write_text('anon 123\n')
            with patch('sampling.processes', return_value={}):
                result = sampling.run_command([sys.executable, '-c', 'raise SystemExit(7)'], base, os.environ.copy(), base / 'run', base, .01, monitor_resources=False)
            self.assertEqual(result['returncode'], 7)
            self.assertEqual(result['anon_peak_bytes'], 123)
            self.assertTrue(all(result['cleanup'].values()))
            self.assertIn('one wait4', result['cpu']['scope'])

    def test_source_and_target_identity_fail_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            with patch('audit.git', side_effect=[str(base), 'actual']):
                with self.assertRaises(ValueError):
                    audit.source(base, 'wrong')
            (base / 'target').mkdir()
            with self.assertRaises(FileExistsError):
                audit.owned_target(base, base, {'sha': 'source'})
            reuse = base / 'reuse'
            reuse.mkdir()
            (reuse / 'target-ownership.json').write_text(json.dumps({'workspace': 'wrong', 'source_sha': 'source', 'tool_files': {}, 'target': 'unknown'}))
            with self.assertRaises(ValueError):
                audit.owned_target(base, base, {'sha': 'source'}, reuse)

    def test_exact_context_mutation_and_guarded_recovery(self):
        original = b'pub async fn get_session() {\n    Ok(Json(ApiSuccess::new(response)))\n}\n\n#[utoipa::path(\nfn another() { Ok(Json(ApiSuccess::new(response))) }\n'
        changed = incremental.mutation(original)
        self.assertEqual(changed.count(incremental.NEW), 1)
        self.assertEqual(changed.count(incremental.OLD), 1)
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            handler = base / incremental.HANDLERS[0]
            handler.parent.mkdir(parents=True)
            handler.write_bytes(original)
            _, data, _ = incremental.prepare(base, base / 'recovery', 'source')
            incremental.replace(handler, data, 0o644)
            with patch('audit.git', return_value='source'):
                incremental.recover(base, base / 'recovery')
                self.assertEqual(handler.read_bytes(), original)
                handler.write_bytes(b'independent user edit')
                with self.assertRaises(ValueError):
                    incremental.recover(base, base / 'recovery')
                self.assertEqual(handler.read_bytes(), b'independent user edit')

    def test_incremental_failure_restores_original_bytes(self):
        original = b'pub async fn get_session() { Ok(Json(ApiSuccess::new(response))) }\n#[utoipa::path(\n'
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            workspace = base / 'source'
            handler = workspace / incremental.HANDLERS[0]
            handler.parent.mkdir(parents=True)
            handler.write_bytes(original)
            output, reuse = base / 'out', base / 'reuse'
            output.mkdir()
            reuse.mkdir()
            identity = {'sha': 'source', 'workspace': str(workspace)}
            (reuse / 'measurement.json').write_text(json.dumps({'workload': 'dev2', 'source': identity, 'checks': {'ok': True}}))
            args = SimpleNamespace(workspace=workspace, source_sha='source', reuse_target_from=reuse, role='candidate')
            calls = []
            def phase(*args):
                calls.append(args)
                if len(calls) == 1:
                    return {'checks': {'workload_completed': True}}
                raise RuntimeError('controlled compiler failure')
            with patch('audit.git', return_value='source'), patch('audit.source', return_value=identity):
                with self.assertRaises(RuntimeError):
                    incremental.run_incremental(args, identity, output, reuse, phase)
            self.assertEqual(handler.read_bytes(), original)
            self.assertTrue(json.loads((output / 'incremental.json').read_text())['source_restored_clean'])

    def test_export_is_committed_and_self_contained_outside_source(self):
        # A tiny committed source fixture; no Cargo or heavy-lock acquisition.
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            workspace = base / 'source'
            tool = workspace / 'tool'
            tool.mkdir(parents=True)
            for name in audit.FILES:
                shutil.copy2(audit.HERE / name, tool / name)
            repository = Path(__file__).resolve().parents[4]
            for name in audit.SUPPORT:
                shutil.copy2(repository / 'scripts/shell' / name, tool / name)
            (workspace / 'api').mkdir()
            (workspace / 'api/Cargo.toml').write_text('[workspace]\n')
            (workspace / 'api/Cargo.lock').write_text('version = 4\n')
            def git(*args):
                return subprocess.check_output(['git', '-C', str(workspace), *args], stderr=subprocess.PIPE).decode().strip()
            git('init', '-q')
            git('add', '.')
            git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture')
            source_sha = git('rev-parse', 'HEAD')
            destination = base / 'export'
            with patch('audit.HERE', tool):
                record = audit.export(workspace, source_sha, destination)
                self.assertEqual(len(record['files']), len(audit.FILES) + len(audit.SUPPORT))
                with self.assertRaises(FileExistsError):
                    audit.export(workspace, source_sha, destination)
                with self.assertRaises(ValueError):
                    audit.export(workspace, source_sha, workspace / 'nested')
            result = subprocess.run([sys.executable, '-B', str(destination / 'audit.py'), '--help'], cwd=base, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            result = subprocess.run([sys.executable, '-B', str(destination / 'resource-job-run.py'), '--help'], cwd=base, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
