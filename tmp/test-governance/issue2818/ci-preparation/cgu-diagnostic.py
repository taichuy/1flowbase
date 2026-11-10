#!/usr/bin/env python3
"""One bounded CGU diagnosis. No ordinary-build qualification or cache export."""
import gzip, hashlib, importlib.util, json, os, re, subprocess, time
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def main():
    entry = load('frozen_entry', HERE / 'coupled-api/entry.py')
    b = entry.check(execution=True)
    assert b['diagnostic_only'] and b['diagnostic_contract']['normal_acceptance'] is False
    g = load('existing_observer', HERE / 'experiment.py')
    g.SEED_SHA = b['cache']['source_sha']
    g.PRIOR_RUN = str(b['cache']['run'])
    g.PRIOR_NAME = b['cache']['name']
    g.PRIOR_ARTIFACT_ID = b['cache']['id']
    g.ARCHIVE_SHA = b['cache']['archive_sha256']
    g.ARCHIVE_BYTES = b['cache']['archive_bytes']
    g.END = time.monotonic() + 1100
    g.EVIDENCE.mkdir(parents=True, exist_ok=False)
    g.TEMP.mkdir(parents=True, exist_ok=False)
    e = g.EVIDENCE
    source, target = g.TEMP / 'seed-source', g.TEMP / 'target-seed'
    status = {'source_sha': b['candidate'], 'diagnostic_only': True,
              'normal_acceptance': False, 'cache_exported': False, 'status': 'incomplete'}
    row = None
    def cap(argv):
        return subprocess.check_output(argv, cwd=source, text=True, stderr=subprocess.STDOUT, timeout=60).strip()
    try:
        initial = g.budget_snapshot()
        entry.save(e / 'budget-before.json', initial)
        assert initial['memory.max'] == '8000000000' and initial['memory.high'] == 'max' and initial['memory.swap.max'] == '0'
        quota, period = map(int, initial['cpu.max'].split())
        assert quota / period <= os.cpu_count() * .6 + .001
        assert g.capture(['rustc', '-vV']) == b['cache']['compiler']
        assert not any(os.environ.get(k) for k in ['RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTC_BOOTSTRAP', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'MALLOC_CONF'])
        assert g.PG_GROUP is None and os.access(Path(initial['path']) / 'memory.reclaim', os.W_OK)
        os.environ['ISSUE2818_OBSERVED_SOURCE_SHA'] = b['cache']['source_sha']
        archive, _ = g.download_retained()
        resume = entry.read(e / 'cache-resume.json')
        resume.update(cache_state='inherited partial target; diagnosis only', normal_acceptance=False, cache_exported=False)
        entry.save(e / 'cache-resume.json', resume)
        g.run('checkout-cache-source', ['git', 'worktree', 'add', '--detach', str(source), b['cache']['source_sha']], seconds=120)
        g.run('restore-source-target', ['tar', '-I', 'zstd -T1', '-xf', str(archive), '--no-same-owner', '-C', str(g.TEMP)], seconds=180)
        assert cap(['git', 'rev-parse', 'HEAD']) == b['cache']['source_sha']
        cap(['git', 'diff', '--exit-code', 'HEAD', '--'])
        cap(['git', 'checkout', '--detach', b['candidate']])
        assert cap(['git', 'rev-parse', 'HEAD:api']) == b['api_tree']
        cap(['git', 'diff', '--exit-code', 'HEAD', '--'])
        assert all(entry.sha(source / path) == digest for path, digest in b['source_hashes'].items())
        os.environ['ISSUE2818_OBSERVED_SOURCE_SHA'] = b['candidate']
        entry.save(e / 'source-identity.json', {'source_sha': b['candidate'], 'api_tree': b['api_tree'], 'source_hashes': b['source_hashes'], 'tracked_clean': True})
        restored = g.budget_snapshot()
        entry.save(e / 'budget-after-restore.json', restored)
        assert int(restored['memory.peak']) <= 6400000000
        cargo_files = [str(f.relative_to(source)) for f in (source / 'api').rglob('Cargo.toml') if 'target' not in f.parts] + ['api/Cargo.lock']
        before = {path: entry.sha(source / path) for path in cargo_files}
        fetch = dict(os.environ)
        fetch.pop('CARGO_NET_OFFLINE', None)
        try:
            g.run('registry-prefetch', ['cargo', 'fetch', '--locked', '--manifest-path', 'api/Cargo.toml'], cwd=source, env=fetch, seconds=180)
        finally:
            after = {path: entry.sha(source / path) for path in cargo_files}
            entry.save(e / 'prefetch-proof.json', {'before': before, 'after': after, 'all_unchanged': before == after})
        assert before == after
        env = {**os.environ, 'CARGO_TARGET_DIR': str(target), 'CARGO_BUILD_JOBS': '1',
               'CARGO_INCREMENTAL': '0', 'CARGO_NET_OFFLINE': 'true', 'CARGO_TERM_COLOR': 'never',
               'RUST_TEST_THREADS': '1', 'TERM': 'dumb', 'ISSUE2818_ALL_RUSTC_LOG': str(e / 'all-rustc-invocations.jsonl')}
        mono_dir = e / 'mono'
        mono_dir.mkdir()
        flags = ['-Zno-codegen', '-Zprint-mono-items', '-Zdump-mono-stats=' + str(mono_dir),
                 '-Zdump-mono-stats-format=json', '-Ztime-passes', '-Ztime-passes-format=json']
        wrapper = (HERE / 'coupled-api/root-wrapper.py').read_text()
        marker = "    if mode=='diagnostic':"
        assert wrapper.count(marker) == 1
        insertion = "    if mode=='cgu-diagnostic' and crate=='api_server' and '-Zhelp' not in args:\n        effective += " + repr(flags) + "\n        env['RUSTC_BOOTSTRAP']=crate\n"
        g.ROOT_WRAPPER = wrapper.replace(marker, insertion + marker)
        diagnostic = g.install_stage_wrapper(source, env)
        diagnostic['ISSUE2818_RUSTC_MODE'] = 'cgu-diagnostic'
        diagnostic.pop('ISSUE2818_MONO_STATS', None)
        entry.save(e / 'diagnostic-inputs.json', {'contract': b['diagnostic_contract'], 'source_sha': b['candidate'], 'original_cargo_argv': b['A'], 'api_only_append': flags, 'wrapper_sha256': hashlib.sha256(g.ROOT_WRAPPER.encode()).hexdigest()})
        row = g.run('cgu-no-codegen', b['A'], cwd=source / 'api', env=diagnostic, heavy=True, seconds=600)
        invocations = [json.loads(line) for line in (e / 'root-rustc-invocations.jsonl').read_text().splitlines()]
        actual = [x for x in invocations if not x['probe'] and x['mode'] == 'cgu-diagnostic' and x['original_argv'][x['original_argv'].index('--crate-name') + 1] == 'api_server']
        assert len(actual) == 1
        root = actual[0]
        assert root['effective_argv'] == root['original_argv'] + flags and root['bootstrap_for_this_exec'] == 'api_server'
        assert '--test' not in root['original_argv'] and '--emit=dep-info,metadata,link' in root['original_argv']
        counts, linkage, instances, placements, repeated = Counter(), Counter(), 0, 0, 0
        seen = set()
        with Path(row['stdout_log']).open() as source_log, gzip.open(e / 'mono-cgu-map.jsonl.gz', 'wt') as output:
            for line in source_log:
                if not line.startswith('MONO_ITEM '):
                    continue
                item, raw_units = line.rstrip('\n')[10:].rsplit(' @@ ', 1)
                pairs = re.findall(r'([^\s\[\]]+)\[([^\]]+)\]', raw_units)
                assert pairs and re.sub(r'([^\s\[\]]+)\[([^\]]+)\]', '', raw_units).strip() == ''
                digest = hashlib.sha256(item.encode()).digest()
                assert digest not in seen
                seen.add(digest)
                instances += 1
                placements += len(pairs)
                repeated += len(pairs) > 1
                for unit, kind in pairs:
                    counts[unit] += 1
                    linkage[kind] += 1
                output.write(json.dumps({'mono': item, 'placements': pairs}) + '\n')
        assert instances > 0
        stats = list(mono_dir.glob('api_server*.json'))
        assert len(stats) == 1 and json.loads(stats[0].read_text())
        cap(['git', 'diff', '--exit-code', 'HEAD', '--'])
        entry.save(e / 'cgu-distribution.json', {'source_sha': b['candidate'], 'instances': instances, 'placements': placements, 'multi_cgu_instances': repeated, 'occupied_cgus': len(counts), 'member_counts': dict(counts.most_common()), 'linkage_counts': dict(linkage), 'limits': 'Membership only; neither CGU size nor RSS attribution. No ordinary build qualification.'})
        status.update(status='diagnostic-complete', instances=instances, occupied_cgus=len(counts), compiler_root=root, mono_stats_sha256=entry.sha(stats[0]))
    except Exception as error:
        status.update(status='diagnostic-incomplete', error=repr(error))
        raise
    finally:
        status['final_budget'] = g.budget_snapshot()
        status['commands'] = g.RESULTS
        status['no_unchanged_retry'] = True
        entry.save(e / 'diagnostic-result.json', status)
        entry.save(e / 'diagnostic-cache-policy.json', {'exported': False, 'normal_acceptance': False, 'reason': 'no-codegen target is diagnosis-only and never retained as an ordinary build cache'})
        print(json.dumps({k: v for k, v in status.items() if k not in ['commands', 'compiler_root', 'final_budget']}), flush=True)

if __name__ == '__main__':
    main()
