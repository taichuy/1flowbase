import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('cache_tag', Path(__file__).resolve().parents[1] / 'cache-tag.py')
cache_tag = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cache_tag)


class CacheTagTests(unittest.TestCase):
    def test_canonical_path_rejects_symlink_and_symlink_parent(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            real = base / 'real'
            real.mkdir()
            (real / 'file').write_text('original')
            (base / 'link').symlink_to(real, target_is_directory=True)
            for file in [base / 'link', base / 'link/file']:
                with self.assertRaisesRegex(ValueError, 'Symlink'):
                    cache_tag.canonical(file)
            self.assertEqual(cache_tag.canonical(real), real)

    def test_source_requires_real_root_exact_sha_and_clean_committed_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'api').mkdir()
            (root / 'api/Cargo.toml').write_text('[workspace]\n')
            (root / 'api/Cargo.lock').write_text('version = 4\n')
            def git(*args):
                return subprocess.check_output(['git', '-C', str(root), *args], stderr=subprocess.PIPE).decode().strip()
            git('init', '-q')
            git('add', '.')
            git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture')
            source_sha = git('rev-parse', 'HEAD')
            self.assertEqual(cache_tag.source(root, source_sha)['sha'], source_sha)
            with self.assertRaisesRegex(ValueError, 'Source SHA mismatch'):
                cache_tag.source(root, '0' * 40)
            with self.assertRaisesRegex(ValueError, 'real git root'):
                cache_tag.source(root / 'api', source_sha)
            (root / 'api/Cargo.toml').write_text('[workspace]\n# independent edit\n')
            with self.assertRaisesRegex(ValueError, 'not clean'):
                cache_tag.source(root, source_sha)

    def test_unknown_or_unmeasured_input_never_gets_a_marker(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            (base / 'target').mkdir()
            with self.assertRaises(subprocess.CalledProcessError):
                cache_tag.verified_inputs(base, '0' * 40, base, base)
            self.assertFalse((base / 'target/CACHEDIR.TAG').exists())

    def test_marker_creation_is_exclusive_and_existing_valid_or_invalid_bytes_survive(self):
        with tempfile.TemporaryDirectory() as temporary:
            target = Path(temporary)
            info = target.stat()
            identity = info.st_dev, info.st_ino
            for content in [b'invalid existing marker', cache_tag.SIGNATURE]:
                marker = target / 'CACHEDIR.TAG'
                marker.write_bytes(content)
                with self.assertRaisesRegex(ValueError, 'never overwritten'):
                    cache_tag.create_marker(target, cache_tag.SIGNATURE, identity)
                self.assertEqual(marker.read_bytes(), content)
                marker.unlink()  # Unit fixture teardown, never a repair action.
            cache_tag.create_marker(target, cache_tag.SIGNATURE, identity)
            self.assertEqual((target / 'CACHEDIR.TAG').read_bytes(), cache_tag.SIGNATURE)

    def test_marker_symlink_does_not_modify_its_destination(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            target = base / 'target'
            target.mkdir()
            outside = base / 'outside'
            outside.write_bytes(b'untouched')
            (target / 'CACHEDIR.TAG').symlink_to(outside)
            info = target.stat()
            with self.assertRaisesRegex(ValueError, 'never overwritten'):
                cache_tag.create_marker(target, cache_tag.SIGNATURE, (info.st_dev, info.st_ino))
            self.assertEqual(outside.read_bytes(), b'untouched')

    def test_directory_identity_change_is_rejected_before_any_write(self):
        with tempfile.TemporaryDirectory() as temporary:
            target = Path(temporary)
            info = target.stat()
            with self.assertRaisesRegex(ValueError, 'directory changed'):
                cache_tag.create_marker(target, cache_tag.SIGNATURE, (info.st_dev, info.st_ino + 1))
            self.assertFalse((target / 'CACHEDIR.TAG').exists())

    def test_target_volume_rejects_symlinks_instead_of_following_them(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            target = base / 'target'
            target.mkdir()
            (base / 'outside').write_bytes(b'untouched')
            (target / 'link').symlink_to(base / 'outside')
            with self.assertRaisesRegex(ValueError, 'Symlink inside'):
                cache_tag.volume(target)
            self.assertEqual((base / 'outside').read_bytes(), b'untouched')

    def test_artifact_identity_requires_actual_recorded_inodes_and_hashes(self):
        with tempfile.TemporaryDirectory() as temporary:
            target = Path(temporary)
            with self.assertRaisesRegex(ValueError, 'produced artifacts'):
                cache_tag.artifact_identity(target, {'bins': [], 'test_or_bin_harness_executables': []})
            binary = target / 'probe'
            binary.write_bytes(b'unit artifact')
            info = binary.stat()
            artifact = {'path': str(binary), 'device': info.st_dev, 'inode': info.st_ino,
                        'apparent_bytes': info.st_size, 'allocated_bytes': info.st_blocks * 512,
                        'sha256': cache_tag.digest(binary)}
            artifacts = {'bins': [artifact], 'test_or_bin_harness_executables': [artifact]}
            self.assertEqual(cache_tag.artifact_identity(target, artifacts)['bins'], 1)
            self.assertEqual(cache_tag.artifact_identity(target, {'bins': [], 'test_or_bin_harness_executables': [artifact]})['harness_executables'], 1)
            binary.write_bytes(b'changed unit artifact')
            with self.assertRaisesRegex(ValueError, 'identity/hash mismatch'):
                cache_tag.artifact_identity(target, artifacts)

    def test_modified_original_evidence_fails_the_final_check(self):
        with tempfile.TemporaryDirectory() as temporary:
            file = Path(temporary) / 'receipt.json'
            file.write_text('{}')
            expected = {str(file): cache_tag.digest(file)}
            cache_tag.unchanged(expected)
            file.write_text('{"independent":true}')
            with self.assertRaisesRegex(ValueError, 'evidence changed'):
                cache_tag.unchanged(expected)

    def test_repair_receipt_cannot_be_written_inside_original_data_or_tool_export(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            data, tools = base / 'data', base / 'tools'
            data.mkdir()
            tools.mkdir()
            for parent in [data, tools]:
                args = SimpleNamespace(workspace=base, data_dir=data, tool_directory=tools,
                                       observation_data_dir=None, receipt_directory=parent / 'repair', source_sha='0' * 40)
                with self.assertRaisesRegex(ValueError, 'outside original data'):
                    cache_tag.repair(args)
                self.assertFalse(args.receipt_directory.exists())


class AllocationReconciliationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.target = Path(temporary.name)
        self.binary = self.target / 'probe'
        with self.binary.open('wb') as stream:
            stream.truncate(8192)  # Real sparse zero bytes, no Cargo or compiler.

    def recorded_artifact(self, file):
        info = file.stat()
        return {'path': str(file), 'device': info.st_dev, 'inode': info.st_ino,
                'apparent_bytes': info.st_size, 'allocated_bytes': info.st_blocks * 512,
                'sha256': cache_tag.digest(file)}

    def allocate_same_bytes(self, file):
        before = self.recorded_artifact(file)
        descriptor = os.open(file, os.O_RDWR)
        try:
            os.posix_fallocate(descriptor, 0, before['apparent_bytes'])
        finally:
            os.close(descriptor)
        after = self.recorded_artifact(file)
        self.assertEqual(after['sha256'], before['sha256'])
        self.assertEqual(after['inode'], before['inode'])
        self.assertGreater(after['allocated_bytes'], before['allocated_bytes'])
        return after['allocated_bytes'] - before['allocated_bytes']

    def test_same_content_allocation_delta_exactly_explains_target(self):
        recorded = self.recorded_artifact(self.binary)
        measured_volume = cache_tag.volume(self.target)
        delta = self.allocate_same_bytes(self.binary)
        result = cache_tag.reconcile_allocation(self.target, measured_volume,
            {'bins': [recorded], 'test_or_bin_harness_executables': []})
        self.assertEqual(result['measured_target_volume'], measured_volume)
        self.assertEqual(result['current_target_volume'], cache_tag.volume(self.target))
        self.assertEqual(result['target_allocated_delta_bytes'], delta)
        self.assertEqual(result['artifact_allocated_delta_bytes'], delta)
        self.assertEqual(len(result['unique_artifact_allocations']), 1)
        allocation = result['unique_artifact_allocations'][0]
        self.assertEqual(allocation['measured_allocated_bytes'], recorded['allocated_bytes'])
        self.assertEqual(allocation['current_allocated_bytes'], recorded['allocated_bytes'] + delta)
        self.assertEqual(allocation['allocated_delta_bytes'], delta)

    def test_hardlink_bin_and_harness_allocation_is_counted_once(self):
        alias = self.target / 'hardlink'
        os.link(self.binary, alias)
        recorded = self.recorded_artifact(self.binary)
        harness = self.recorded_artifact(alias)
        measured_volume = cache_tag.volume(self.target)
        delta = self.allocate_same_bytes(self.binary)
        result = cache_tag.reconcile_allocation(self.target, measured_volume,
            {'bins': [recorded], 'test_or_bin_harness_executables': [harness, recorded]})
        self.assertEqual(result['artifact_allocated_delta_bytes'], delta)
        self.assertEqual(result['target_allocated_delta_bytes'], delta)
        self.assertEqual(len(result['unique_artifact_allocations']), 1)
        self.assertEqual(set(result['unique_artifact_allocations'][0]['paths']), {str(self.binary), str(alias)})

    def test_unrecorded_file_allocation_delta_is_rejected(self):
        unknown = self.target / 'unrecorded'
        with unknown.open('wb') as stream:
            stream.truncate(8192)
        recorded = self.recorded_artifact(self.binary)
        measured_volume = cache_tag.volume(self.target)
        self.allocate_same_bytes(unknown)
        with self.assertRaisesRegex(ValueError, 'not exactly explained'):
            cache_tag.reconcile_allocation(self.target, measured_volume,
                {'bins': [recorded], 'test_or_bin_harness_executables': []})

    def test_hash_change_is_rejected_even_when_allocation_delta_would_match(self):
        recorded = self.recorded_artifact(self.binary)
        measured_volume = cache_tag.volume(self.target)
        self.allocate_same_bytes(self.binary)
        with self.binary.open('r+b') as stream:
            stream.write(b'X')
        self.assertEqual(self.binary.stat().st_size, recorded['apparent_bytes'])
        with self.assertRaisesRegex(ValueError, 'identity/hash mismatch'):
            cache_tag.reconcile_allocation(self.target, measured_volume,
                {'bins': [recorded], 'test_or_bin_harness_executables': []})

    def test_every_non_allocation_volume_field_remains_exact(self):
        recorded = self.recorded_artifact(self.binary)
        measured_volume = cache_tag.volume(self.target)
        for key in measured_volume:
            if key == 'allocated_bytes':
                continue
            with self.subTest(field=key):
                wrong = dict(measured_volume)
                wrong[key] = 'changed' if isinstance(wrong[key], str) else wrong[key] + 1
                with self.assertRaisesRegex(ValueError, 'outside allocated bytes'):
                    cache_tag.reconcile_allocation(self.target, wrong,
                        {'bins': [recorded], 'test_or_bin_harness_executables': []})

    def test_conflicting_hardlink_allocation_snapshots_are_rejected(self):
        alias = self.target / 'hardlink'
        os.link(self.binary, alias)
        recorded = self.recorded_artifact(self.binary)
        conflicting = self.recorded_artifact(alias)
        conflicting['allocated_bytes'] += 512
        with self.assertRaisesRegex(ValueError, 'Conflicting hardlink'):
            cache_tag.reconcile_allocation(self.target, cache_tag.volume(self.target),
                {'bins': [recorded], 'test_or_bin_harness_executables': [conflicting]})


class ThresholdCleanupGateTests(unittest.TestCase):
    MEMORY_KEYS = ('all_observed_rustc_invocations_at_most_4_gib',
                   'observed_direct_rustc_hwm_at_most_4_gib',
                   'anonymous_peak_limit',
                   'api_server_link_descendant_upper_bound_at_most_3_9_gib')

    def fixture(self, role='candidate', workload='dev2', failed=()):
        checks = {'workload_completed': True, 'source_and_configuration_unchanged': True}
        if workload in ('dev2', 'dev10', 'test2', 'incremental'):
            checks['rustc_records_present'] = True
        if role == 'candidate':
            checks.update({key: True for key in self.MEMORY_KEYS[:2]})
            if workload in ('dev2', 'test2'):
                checks['anonymous_peak_limit'] = True
            if workload in ('dev2', 'dev10', 'incremental'):
                checks['api_server_link_descendant_upper_bound_at_most_3_9_gib'] = True
        for key in failed:
            checks[key] = False
        measured = {'role': role, 'workload': workload, 'checks': checks,
                    'candidate_thresholds_applied': role == 'candidate',
                    'cgroup_observation': {'returncode': 0, 'error': None, 'cancel_signals': [],
                        'cleanup': {'process_group_empty': True, 'principal_reaped': True},
                        'memory_events_delta': {'oom': 0, 'oom_kill': 0, 'oom_group_kill': 0}}}
        status = 2 if failed else 0
        command = {'kind': 'exit', 'returncode': status, 'exit_code': status,
                   'wait_status': status << 8, 'signal': None}
        finished = {'classification': 'exit', 'runner_exit_code': status, 'command': dict(command)}
        return measured, command, finished

    def test_only_known_candidate_memory_failures_qualify_without_changing_failure(self):
        for failed in [(key,) for key in self.MEMORY_KEYS] + [self.MEMORY_KEYS]:
            with self.subTest(failed=failed):
                measured, command, finished = self.fixture(failed=failed)
                before = json.dumps((measured, command, finished), sort_keys=True)
                status = cache_tag.measurement_exit_status(measured, command, finished)
                self.assertEqual(status['original_gate_exit_code'], 2)
                self.assertEqual(status['failed_memory_checks'], sorted(failed))
                self.assertTrue(status['original_measurement_failure_preserved'])
                self.assertEqual(json.dumps((measured, command, finished), sort_keys=True), before)

    def test_original_success_exit_zero_remains_eligible_for_each_role(self):
        for role in ('baseline', 'candidate', 'instrumentation'):
            with self.subTest(role=role):
                status = cache_tag.measurement_exit_status(*self.fixture(role=role))
                self.assertEqual(status['original_gate_exit_code'], 0)
                self.assertEqual(status['failed_memory_checks'], [])
                self.assertFalse(status['original_measurement_failure_preserved'])

    def test_known_key_set_is_complete_and_every_value_must_be_boolean(self):
        for unknown in (False, True):
            measured, command, finished = self.fixture()
            measured['checks']['unknown_check'] = unknown
            with self.assertRaisesRegex(ValueError, 'complete known boolean'):
                cache_tag.measurement_exit_status(measured, command, finished)
        measured, _, _ = self.fixture()
        for key in measured['checks']:
            candidate, command, finished = self.fixture()
            del candidate['checks'][key]
            with self.subTest(missing=key), self.assertRaisesRegex(ValueError, 'complete known boolean'):
                cache_tag.measurement_exit_status(candidate, command, finished)
        for value in (0, 1, None, 'true'):
            candidate, command, finished = self.fixture()
            candidate['checks']['workload_completed'] = value
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, 'complete known boolean'):
                cache_tag.measurement_exit_status(candidate, command, finished)

    def test_safety_check_failure_never_becomes_a_cleanup_threshold_exception(self):
        for key in ('workload_completed', 'rustc_records_present', 'source_and_configuration_unchanged'):
            measured, command, finished = self.fixture(failed=(self.MEMORY_KEYS[-1],))
            measured['checks'][key] = False
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'safety checks'):
                cache_tag.measurement_exit_status(measured, command, finished)

    def test_underlying_process_failure_cancel_oom_or_incomplete_cleanup_is_rejected(self):
        changes = [('returncode', 1), ('returncode', True), ('error', 'compiler error'),
                   ('cancel_signals', [15]), ('cleanup', {'process_group_empty': False, 'principal_reaped': True}),
                   ('cleanup', {'process_group_empty': 1, 'principal_reaped': True})]
        for key, value in changes:
            measured, command, finished = self.fixture(failed=(self.MEMORY_KEYS[-1],))
            measured['cgroup_observation'][key] = value
            with self.subTest(key=key, value=value), self.assertRaisesRegex(ValueError, 'Raw measured process'):
                cache_tag.measurement_exit_status(measured, command, finished)
        for key in ('oom', 'oom_kill', 'oom_group_kill'):
            measured, command, finished = self.fixture(failed=(self.MEMORY_KEYS[-1],))
            measured['cgroup_observation']['memory_events_delta'][key] = 1
            with self.subTest(oom=key), self.assertRaisesRegex(ValueError, 'Raw measured process'):
                cache_tag.measurement_exit_status(measured, command, finished)

    def test_original_managed_exit_must_exactly_match_all_checks_result(self):
        for failed in ((), (self.MEMORY_KEYS[-1],)):
            expected = 2 if failed else 0
            for field in ('returncode', 'exit_code', 'wait_status', 'kind', 'signal',
                          'runner_exit_code', 'classification', 'command_copy'):
                measured, command, finished = self.fixture(failed=failed)
                if field in ('returncode', 'exit_code'):
                    command[field] = 0 if expected else 2
                    finished['command'] = dict(command)
                elif field == 'wait_status':
                    command[field] = (0 if expected else 2) << 8
                    finished['command'] = dict(command)
                elif field in ('kind', 'signal'):
                    command[field] = 'signal' if field == 'kind' else 15
                    finished['command'] = dict(command)
                elif field == 'runner_exit_code':
                    finished[field] = 0 if expected else 2
                elif field == 'classification':
                    finished[field] = 'deadline'
                else:
                    finished['command']['returncode'] = 101
                with self.subTest(failed=failed, field=field), self.assertRaisesRegex(ValueError, 'managed exit contradicts'):
                    cache_tag.measurement_exit_status(measured, command, finished)

        # Even mutually consistent original exits must match all(checks), not merely each other.
        for failed in ((), (self.MEMORY_KEYS[-1],)):
            measured, command, finished = self.fixture(failed=failed)
            wrong = 0 if failed else 2
            command.update(returncode=wrong, exit_code=wrong, wait_status=wrong << 8)
            finished.update(runner_exit_code=wrong, command=dict(command))
            with self.subTest(consistent_wrong_exit=wrong), self.assertRaisesRegex(ValueError, 'managed exit contradicts'):
                cache_tag.measurement_exit_status(measured, command, finished)

    def test_threshold_exception_requires_candidate_role_and_exact_threshold_flag(self):
        for role in ('baseline', 'instrumentation'):
            measured, command, finished = self.fixture(role=role, failed=(self.MEMORY_KEYS[-1],))
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, 'complete known boolean'):
                cache_tag.measurement_exit_status(measured, command, finished)
        for flag in (False, None, 1):
            measured, command, finished = self.fixture(failed=(self.MEMORY_KEYS[-1],))
            measured['candidate_thresholds_applied'] = flag
            with self.subTest(flag=flag), self.assertRaisesRegex(ValueError, 'threshold flag'):
                cache_tag.measurement_exit_status(measured, command, finished)

    def test_workload_specific_check_keys_match_current_audit_schema(self):
        expected_memory = {'dev2': set(self.MEMORY_KEYS), 'dev10': set(self.MEMORY_KEYS) - {'anonymous_peak_limit'},
                           'test2': set(self.MEMORY_KEYS) - {self.MEMORY_KEYS[-1]},
                           'incremental': set(self.MEMORY_KEYS) - {'anonymous_peak_limit'},
                           'external': set(self.MEMORY_KEYS[:2]), 'exact-bin-warmup': set()}
        for workload, expected in expected_memory.items():
            role = 'instrumentation' if workload == 'exact-bin-warmup' else 'candidate'
            measured, command, finished = self.fixture(role=role, workload=workload)
            actual = set(measured['checks']) - {'workload_completed', 'rustc_records_present', 'source_and_configuration_unchanged'}
            self.assertEqual(actual, expected)
            self.assertEqual(cache_tag.measurement_exit_status(measured, command, finished)['original_gate_exit_code'], 0)
        for field in ('role', 'workload'):
            measured, command, finished = self.fixture()
            measured[field] = 'unknown'
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'Unknown measurement'):
                cache_tag.measurement_exit_status(measured, command, finished)


if __name__ == '__main__':
    unittest.main()
