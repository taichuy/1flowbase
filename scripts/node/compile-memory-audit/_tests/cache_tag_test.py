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


if __name__ == '__main__':
    unittest.main()
