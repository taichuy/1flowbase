from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import report


def inventory(records, sha='source'):
    return {'input_sha': sha, 'records': records, 'summary': {'discovered': len(records)}}


def record(target='one', ignored=False):
    return {'package_id': 'package', 'cargo_target_name': target, 'target_source': target + '.rs',
            'target_kind': ['lib'], 'function': 'same::test', 'kind': 'test', 'ignored': ignored}


class ReportTests(unittest.TestCase):
    def test_missing_samples_and_small_historical_ci_stay_null(self):
        self.assertIsNone(report.retention(inventory([record()]), None)['retention_fraction'])
        row = report.ci_summary([{'status': 'completed', 'conclusion': 'failure', 'sum_job_elapsed_minutes': n} for n in [26.75, 47.1, 21.65]])
        self.assertIsNone(row['p50_runner_minutes'])
        self.assertIsNone(row['p95_runner_minutes'])
        self.assertEqual(row['n'], 3)

    def test_multiplicity_and_new_ignored_are_not_hidden_by_function_name(self):
        before = inventory([record('one'), record('two')])
        missing = report.retention(before, inventory([record('one')]))
        self.assertEqual(missing['retention_fraction'], .5)
        self.assertFalse(missing['multiplicity_preserved'])
        ignored = report.retention(before, inventory([record('one'), record('two', True)]))
        self.assertEqual(len(ignored['new_ignored']), 1)
        self.assertEqual(ignored['retained'], 2)

    def test_exact_one_to_one_runtime_remap_and_source_identity(self):
        before, after = inventory([record('one')], 'before'), inventory([record('moved')], 'after')
        mapping = {'baseline_sha': 'before', 'candidate_sha': 'after', 'pairs': [{'before': record('one'), 'after': record('moved')}]}
        self.assertEqual(report.retention(before, after, mapping)['retention_fraction'], 1)
        mapping['pairs'].append(mapping['pairs'][0])
        with self.assertRaises(ValueError):
            report.retention(before, after, mapping)
        mapping['pairs'].pop()
        mapping['baseline_sha'] = 'wrong'
        with self.assertRaises(ValueError):
            report.retention(before, after, mapping)
