#!/usr/bin/env python3
"""Generate source-bound audit artifacts; absent measurements remain null."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import statistics
import subprocess
import sys

sys.dont_write_bytecode = True
from sampling import atomic_json, rustc_rows


def load(file):
    return json.loads(file.read_text()) if file else None


def digest(file):
    result = hashlib.sha256()
    with file.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(block)
    return result.hexdigest()


def inventory_key(record):
    return json.dumps([record['package_id'], record['cargo_target_name'], record['target_source'],
                       record['target_kind'], record['function'], record['kind']], separators=(',', ':'))


def retention(before, after, remap=None):
    if before is None or after is None:
        return {'baseline_discovered': before['summary']['discovered'] if before else None,
                'candidate_discovered': None, 'retained': None, 'retention_fraction': None,
                'multiplicity_preserved': None, 'new_ignored': None, 'scope': 'actual runtime discovery, not execution/defect proof'}
    base = Counter(inventory_key(r) for r in before['records'])
    candidate = Counter(inventory_key(r) for r in after['records'])
    mapping = {}
    if remap:
        if remap['baseline_sha'] != before['input_sha'] or remap['candidate_sha'] != after['input_sha']:
            raise ValueError('Inventory source-remap provenance mismatch')
        values = set()
        for pair in remap['pairs']:
            a, b = inventory_key(pair['before']), inventory_key(pair['after'])
            if a in mapping or b in values or a not in base or b not in candidate:
                raise ValueError('Ambiguous, reused or nonexistent runtime remap identity')
            mapping[a] = b
            values.add(b)
    projected = Counter()
    for key, count in base.items():
        projected[mapping.get(key, key)] += count
    if sum(projected.values()) != sum(base.values()) or len(projected) != len(base):
        raise ValueError('Remap collapsed original target multiplicity')
    retained = sum((projected & candidate).values())
    ignored_before = Counter(mapping.get(inventory_key(r), inventory_key(r)) for r in before['records'] if r['ignored'])
    ignored_after = Counter(inventory_key(r) for r in after['records'] if r['ignored'])
    new_ignored = list((ignored_after - ignored_before).elements())
    return {'baseline_discovered': sum(base.values()), 'candidate_discovered': sum(candidate.values()),
            'retained': retained, 'retention_fraction': retained / sum(base.values()) if base else None,
            'missing': list((projected - candidate).elements()), 'multiplicity_preserved': projected <= candidate,
            'ignored_state_preserved': ignored_before <= ignored_after, 'new_ignored': new_ignored,
            'source_remap_required_for_moved_identities': bool(projected - candidate),
            'scope': 'actual runtime package/target/source/kind/function instances; no deduplication by function name; discovery is not execution/defect proof'}


def ci_summary(samples):
    if samples is None:
        return {'n': 0, 'raw_samples': None, 'p50_runner_minutes': None, 'p95_runner_minutes': None}
    values = [r['sum_job_elapsed_minutes'] for r in samples]
    eligible = (len(values) >= 20 and all(r.get('status') == 'completed' for r in samples)
                and len({r.get('conclusion') for r in samples}) == 1)
    sorted_values = sorted(values)
    return {'n': len(values), 'raw_samples': samples,
            'p50_runner_minutes': statistics.median(values) if eligible else None,
            'p95_runner_minutes': sorted_values[max(0, int(.95 * len(values) + .999) - 1)] if eligible else None,
            'scope': 'sum of actual job elapsed minutes; no queue, billing rounding or causal attribution',
            'quantile_policy': 'at least 20 completed samples with one conclusion; otherwise null'}


def scout(directory):
    if directory is None:
        return None
    result = load(directory / 'incremental-baseline-results.json')
    phases = []
    for row in result:
        label = row['label']
        log = directory / (label + '.log')
        text = log.read_text()
        times = re.findall(r'wall=([\d.]+) user=([\d.]+) sys=([\d.]+) maxrss_kib=(\d+)', text)
        if len(times) != 1:
            raise ValueError('Original Scout GNU time receipt missing/ambiguous')
        wall, user, system, rss = times[0]
        phases.append(dict(row, gnu_time_wall_seconds=float(wall), cpu={'user_seconds': float(user), 'system_seconds': float(system)},
                           invocation_descendant_maxrss_kib=int(rss),
                           rustc=rustc_rows(directory / (label + '.rustc.tsv')),
                           raw_log={'path': str(log), 'sha256': digest(log)}))
    return {'n': 1, 'phases': phases, 'scope': 'historical exact-bin workload only; never merged with canonical all-bin before',
            'direct_linker_pid_peak_kib': None}


def governance(workspace, baseline, candidate):
    lines = subprocess.check_output(['git', '-C', str(workspace), 'diff', '--numstat', baseline, candidate, '--'], text=True)
    rows = []
    for line in lines.splitlines():
        added, removed, file = line.split('\t', 2)
        if file.startswith(('scripts/', '.github/')) or file.endswith('AGENTS.md'):
            rows.append({'path': file, 'added': int(added) if added.isdigit() else None,
                         'removed': int(removed) if removed.isdigit() else None})
    return {'baseline_sha': baseline, 'candidate_sha': candidate, 'files': rows,
            'added_lines': sum(r['added'] or 0 for r in rows), 'removed_lines': sum(r['removed'] or 0 for r in rows),
            'net_lines': sum((r['added'] or 0) - (r['removed'] or 0) for r in rows),
            'scope': 'scripts/, .github/, all AGENTS.md; binary entries retain null counts; no attribution from line totals'}


def build(args):
    measurements = [load(file) for file in args.measurement]
    for row in measurements:
        if row['source']['sha'] not in {args.baseline_source, args.candidate_source}:
            raise ValueError('Measurement source is outside the requested comparison')
    grouped = {}
    for role, source_sha in [('baseline', args.baseline_source), ('candidate', args.candidate_source)]:
        grouped[role] = {}
        for workload in ['dev2', 'dev10', 'test2']:
            rows = [r for r in measurements if r['role'] == role and r['source']['sha'] == source_sha and r['workload'] == workload]
            if len(rows) > 1:
                raise ValueError('Multiple samples for an n=1 workload; do not silently choose or average')
            grouped[role][workload] = rows[0] if rows else None
    incremental = [load(file) for file in args.incremental]
    for row in incremental:
        if row['source']['sha'] not in {args.baseline_source, args.candidate_source}:
            raise ValueError('Incremental source mismatch')
    inputs = [*args.measurement, *args.incremental]
    inputs += [p for p in [args.ci_before, args.ci_after, args.before_inventory, args.after_inventory,
                           args.identity_map, args.defect_evidence] if p]
    from audit import bundle_identity
    exported = Path(__file__).resolve().with_name('export.json')
    tool_source = load(exported)['tool_source_sha'] if exported.exists() else subprocess.check_output(['git', '-C', str(args.workspace), 'rev-parse', 'HEAD'], text=True).strip()
    return {'version': 1, 'baseline_source': args.baseline_source, 'candidate_source': args.candidate_source,
            'tool_source': tool_source, 'tool_source_state': 'committed export' if exported.exists() else 'source tree HEAD; staged/dirty development binding is separate', 'tool_files_sha256': bundle_identity(),
            'canonical_all_bin_measurements': grouped, 'matched_incremental': incremental or None,
            'additional_instrumented_commands': [r for r in measurements if r['workload'] == 'external'] or None,
            'historical_scout_exact_bin': scout(args.scout_directory),
            'ci_before': ci_summary(load(args.ci_before)), 'ci_after': ci_summary(load(args.ci_after)),
            'runtime_inventory_retention': retention(load(args.before_inventory), load(args.after_inventory), load(args.identity_map)),
            'controlled_defect_evidence': load(args.defect_evidence),
            'defect_evidence_scope': 'attached negative/assertion evidence needs an independent source/dependency/receipt audit; discovery retention alone does not establish defect detection',
            'governance_net_diff': governance(args.workspace, args.baseline_source, args.candidate_source),
            'inputs': [{'path': str(p), 'sha256': digest(p)} for p in inputs],
            'limitations': ['Cold workload n=1; no confidence interval or causal CI claim.',
                            'GNU time RSS is an invocation/descendant bound, not direct PID RSS.',
                            'Direct linker RSS/HWM is sampled; short-lived processes can be missed.',
                            'Cold means new Cargo target; OS page cache and registry cache are preserved.',
                            'Missing measurements, execution and controlled defect evidence stay null.']}


def markdown(report):
    lines = ['# API compilation measurement report', '',
             f"Baseline `{report['baseline_source']}`; candidate `{report['candidate_source']}`.", '',
             'Each cold workload is n=1. Missing observations are null; this report does not settle final Root AC.', '',
             '| Role / workload | CPU user + sys (s) | GNU invocation/descendant RSS (KiB) | cgroup anon (B) | target apparent / allocated (B) | all-bin apparent / allocated (B) |',
             '| --- | ---: | ---: | ---: | --- | --- |']
    for role, entries in report['canonical_all_bin_measurements'].items():
        for workload, row in entries.items():
            if row:
                measure = row['cgroup_observation']
                cpu = measure['cpu']
                target = row['target_volume']
                bins = row['artifacts']
                values = (cpu['user_seconds'] + cpu['system_seconds'], measure['invocation_descendant_maxrss_kib'],
                          measure['anon_peak_bytes'], f"{target['apparent_bytes']} / {target['allocated_bytes']}",
                          f"{bins['unique_bin_apparent_bytes']} / {bins['unique_bin_allocated_bytes']}")
            else:
                values = ('null',) * 5
            lines.append(f"| {role} / {workload} | " + ' | '.join(map(str, values)) + ' |')
    lines += ['', 'Direct rustc/linker PID observations, per-rustc rows, all-bin files/hashes, OOM deltas, cleanup and original commands remain in the JSON/raw receipts. The RSS column above is never labeled a direct linker measurement.', '',
              'Matched incremental (exact-bin warmup then original get_session mutation): ' + ('available in JSON' if report['matched_incremental'] else 'null'), '',
              'Historical Scout exact-bin (separate from canonical all-bin): ' + ('available in JSON' if report['historical_scout_exact_bin'] else 'null'), '',
              'Runtime inventory retention / multiplicity / ignored checks:', '```json', json.dumps(report['runtime_inventory_retention'], indent=2), '```', '',
              'Controlled bad-fixture / assertion-preservation evidence: ' + ('attached in JSON' if report['controlled_defect_evidence'] else 'null'), '',
              '| CI samples | n | P50 runner minutes | P95 runner minutes |', '| --- | ---: | ---: | ---: |']
    for label in ['ci_before', 'ci_after']:
        row = report[label]
        lines.append(f"| {label} | {row['n']} | {row['p50_runner_minutes']} | {row['p95_runner_minutes']} |".replace('None', 'null'))
    lines += ['', 'CI uses actual job elapsed sums, excluding queue; this is not billed rounding. Historical failure runs do not establish causal improvements.', '',
              f"Governance diff: +{report['governance_net_diff']['added_lines']} / -{report['governance_net_diff']['removed_lines']} lines; net {report['governance_net_diff']['net_lines']}. Exact files and source SHAs are retained in JSON.", '',
              *['- ' + limit for limit in report['limitations']], '']
    return '\n'.join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace', type=Path, required=True)
    parser.add_argument('--baseline-source', required=True)
    parser.add_argument('--candidate-source', required=True)
    parser.add_argument('--measurement', type=Path, action='append', default=[])
    parser.add_argument('--incremental', type=Path, action='append', default=[])
    for name in ['scout-directory', 'ci-before', 'ci-after', 'before-inventory', 'after-inventory', 'identity-map', 'defect-evidence']:
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--output', type=Path, required=True, help='new exclusive report directory')
    args = parser.parse_args(argv)
    args.output.mkdir(exist_ok=False)
    report = build(args)
    atomic_json(args.output / 'report.json', report)
    (args.output / 'report.md').write_text(markdown(report))
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        print('compile report:', error, file=sys.stderr)
        raise SystemExit(1)
