#!/usr/bin/env python3
"""Exportable compile audit. Run heavy workloads only in the existing task slice."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import time
import tomllib
import uuid

sys.dont_write_bytecode = True

from sampling import admission, atomic_json, read_pairs, run_command, rustc_rows, volume

HERE = Path(__file__).resolve().parent
FILES = ('audit.py', 'sampling.py', 'incremental.py', 'report.py', 'rustc-wrap.sh')
SUPPORT = ('resource-job-run.py', 'resource-heavy-run.py', 'resource-budget-context.sh')
GIB = 1024 ** 3


def sha(file):
    digest = hashlib.sha256()
    with file.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def git(workspace, *args):
    return subprocess.check_output(['git', '-C', str(workspace), *args], stderr=subprocess.PIPE).decode().strip()


def source(workspace, expected, clean=True):
    workspace = workspace.resolve(strict=True)
    if git(workspace, 'rev-parse', '--show-toplevel') != str(workspace):
        raise ValueError('Workspace must be the real git root')
    if git(workspace, 'rev-parse', 'HEAD') != expected:
        raise ValueError('Source SHA does not match the explicitly requested source')
    if clean and git(workspace, 'status', '--porcelain'):
        raise ValueError('Source must be clean; intentional incremental diff is separately owned')
    return {'workspace': str(workspace), 'sha': expected,
            'manifest_sha256': sha(workspace / 'api/Cargo.toml'),
            'lock_sha256': sha(workspace / 'api/Cargo.lock')}


def helper(name, workspace=None):
    local = HERE / name
    if local.exists():
        return local
    if workspace:
        return workspace / 'scripts/shell' / name
    raise ValueError('Exported helper missing: ' + name)


def bundle_identity():
    return {name: sha(HERE / name) for name in FILES}


def validate_export():
    manifest = HERE / 'export.json'
    if manifest.exists():
        record = json.loads(manifest.read_text())
        if any(sha(HERE / name) != digest for name, digest in record['files'].items()):
            raise ValueError('Exported tool/helper bytes changed')
        return record
    return {'files': bundle_identity(), 'state': 'source-tree tool; use export for cross-checkout runs'}


def export(workspace, expected, destination):
    source(workspace, expected)
    destination = destination.resolve()
    if destination.is_relative_to(workspace):
        raise ValueError('Export must live outside the source tree')
    files = {name: HERE / name for name in FILES}
    files.update({name: helper(name, workspace) for name in SUPPORT})
    # Every exported byte must be the explicitly selected committed version.
    for name, file in files.items():
        relative = file.resolve().relative_to(workspace)
        committed = subprocess.check_output(['git', '-C', str(workspace), 'show', f'{expected}:{relative}'])
        if committed != file.read_bytes():
            raise ValueError('Tool is not the committed source: ' + name)
    destination.mkdir(exist_ok=False)
    for name, file in files.items():
        shutil.copy2(file, destination / name)
    (destination / 'rustc-wrap.sh').chmod(0o755)
    record = {'tool_source_sha': expected, 'source_workspace': str(workspace.resolve()),
              'files': {name: sha(destination / name) for name in files}}
    atomic_json(destination / 'export.json', record)
    return record


def budget():
    declared = os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP']
    identity = int(os.environ['ONEFLOWBASE_TASK_BUDGET_ID'])
    group = Path('/sys/fs/cgroup' + declared)
    actual = Path('/sys/fs/cgroup' + Path('/proc/self/cgroup').read_text().strip().split('::', 1)[1])
    if (not actual.is_relative_to(group) or group.stat().st_ino != identity
            or group.name != 'issue2824.slice' or identity != 14758159
            or (group / 'cpu.max').read_text().strip() != '1000000 100000'
            or int((group / 'memory.max').read_text()) != 18253611008
            or int((group / 'memory.high').read_text()) != 17179869184):
        raise ValueError('Actual task budget ancestry/identity/limits differ from the fixed budget')
    return actual


def config(workspace):
    script = workspace / 'scripts/node/testing/verify-runtime.js'
    # This existing source helper is present in both protected beta and candidate.
    code = 'process.stdout.write(JSON.stringify(require(process.argv[1]).loadVerifyRuntimeConfig({repoRoot:process.argv[2]})))'
    values = json.loads(subprocess.check_output(['node', '-e', code, str(script), str(workspace)]))['backend']
    if values['cargoJobs'] != 2 or values['cargoTestThreads'] != 2:
        raise ValueError('Original parsed jobs2/threads2 configuration changed')
    return {'cargoJobs': 2, 'cargoTestThreads': 2, 'resolver_sha256': sha(script)}


def command(workload, role, extra=()):
    if role not in {'baseline', 'candidate', 'instrumentation'}:
        raise ValueError('Unknown role')
    if role == 'instrumentation' and workload != 'external':
        raise ValueError('Instrumentation role is only for explicitly instrumented external commands')
    if role == 'baseline' and workload not in {'dev2', 'incremental'}:
        raise ValueError('Baseline role only records matched dev2/exact-bin workloads')
    if workload in {'dev2', 'dev10'}:
        return ['cargo', 'build', '--locked', '--offline', '-p', 'api-server']
    if workload == 'test2':
        return ['cargo', 'test', '--locked', '--offline', '-p', 'api-server', '--no-run']
    if workload == 'incremental':
        return ['cargo', 'build', '--locked', '--offline', '-p', 'api-server', '--bin', 'api-server']
    if workload == 'external' and role in {'candidate', 'instrumentation'} and extra:
        return list(extra)
    raise ValueError('Unknown or incomplete workload')


def environment(workspace, output, target, jobs):
    forbidden = ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WORKSPACE_WRAPPER',
                 'CARGO_BUILD_RUSTFLAGS', 'CARGO_INCREMENTAL', 'CARGO_BUILD_TARGET', 'CARGO_BUILD_BUILD_DIR',
                 'AUDIT_TIME_PASSES', 'AUDIT_BIN_PASSES', 'AUDIT_MONO_DIR')
    if any(os.environ.get(key) for key in forbidden) or any(key.startswith('CARGO_PROFILE_') for key in os.environ):
        raise ValueError('Unreviewed compiler/profile/build-directory environment override')
    env = dict(os.environ, CARGO_BUILD_JOBS=str(jobs), CARGO_TARGET_DIR=str(target),
               RUSTC_WRAPPER=str(HERE / 'rustc-wrap.sh'), RUSTC_MEM_LOG=str(output / 'rustc.tsv'),
               ONEFLOWBASE_AUDIT_BUDGET_HELPER=str(helper('resource-budget-context.sh', workspace)))
    return env


def owned_target(output, workspace, identity, reuse=None):
    if reuse:
        previous = json.loads((reuse / 'target-ownership.json').read_text())
        if (previous['workspace'] != str(workspace.resolve()) or previous['source_sha'] != identity['sha']
                or previous['tool_files'] != bundle_identity()
                or Path(previous['target']).resolve() != (reuse.resolve() / 'target')):
            raise ValueError('Reused target ownership/source/tool mismatch')
        target = Path(previous['target'])
        if not target.is_dir():
            raise ValueError('Reused target is absent; no silent cold fallback')
        state = 'warm existing task-owned target'
    else:
        target = output / 'target'
        target.mkdir(exist_ok=False)
        state = 'fresh exclusive task-owned target; registry/download caches preserved'
    atomic_json(output / 'target-ownership.json', {'target': str(target.resolve()),
                'owner_output': str((reuse or output).resolve()), 'workspace': str(workspace.resolve()),
                'source_sha': identity['sha'], 'tool_files': bundle_identity(), 'cache_state': state})
    return target, state


def artifacts(target):
    files = []
    debug = target / 'debug'
    if debug.exists():
        for file in sorted(debug.iterdir()):
            if file.is_file() and not file.is_symlink() and os.access(file, os.X_OK):
                data = file.stat()
                files.append({'path': str(file), 'apparent_bytes': data.st_size,
                              'allocated_bytes': data.st_blocks * 512, 'sha256': sha(file),
                              'device': data.st_dev, 'inode': data.st_ino})
    harnesses = []
    deps = debug / 'deps'
    if deps.exists():
        for file in sorted(deps.iterdir()):
            if file.is_file() and not file.is_symlink() and os.access(file, os.X_OK) and file.suffix not in {'.so', '.dylib', '.dll'}:
                data = file.stat()
                harnesses.append({'path': str(file), 'apparent_bytes': data.st_size,
                                  'allocated_bytes': data.st_blocks * 512, 'sha256': sha(file),
                                  'device': data.st_dev, 'inode': data.st_ino})
    seen = set()
    unique = []
    for file in files:
        key = file['device'], file['inode']
        if key not in seen:
            unique.append(file)
            seen.add(key)
    return {'bins': files, 'test_or_bin_harness_executables': harnesses, 'unique_bin_apparent_bytes': sum(p['apparent_bytes'] for p in unique),
            'unique_bin_allocated_bytes': sum(p['allocated_bytes'] for p in unique),
            'scope': 'all produced executable regular files at target/debug; no required-feature bin is invented'}


def checks(role, workload, measured, rows):
    valid = measured['returncode'] == 0 and not measured['error'] and not measured['cancel_signals']
    valid = valid and all(measured['cleanup'].values()) and not any(measured['memory_events_delta'].get(k, 0) for k in ('oom', 'oom_kill', 'oom_group_kill'))
    result = {'workload_completed': bool(valid)}
    if workload in {'dev2', 'dev10', 'test2', 'incremental'}:
        result['rustc_records_present'] = bool(rows)
    if role == 'candidate':
        result['all_observed_rustc_invocations_at_most_4_gib'] = (bool(rows) or workload == 'external') and all(p['maxrss_kib'] * 1024 <= 4 * GIB for p in rows)
        if workload in {'dev2', 'test2'}:
            result['anonymous_peak_limit'] = measured['anon_peak_bytes'] <= (7.5 if workload == 'dev2' else 8) * GIB
        if workload in {'dev2', 'dev10', 'incremental'}:
            bins = [p for p in rows if p['crate'] == 'api_server' and p['kind'] == 'bin,']
            # A full-bin invocation descendant bound is a valid upper bound for its linker,
            # explicitly NOT a direct linker PID peak. Keep actual PID samples separately.
            result['api_server_link_descendant_upper_bound_at_most_3_9_gib'] = bool(bins) and all(p['maxrss_kib'] * 1024 <= 3.9 * GIB for p in bins)
    return result


def configuration_hashes(workspace):
    paths = [workspace / '.1flowbase.verify.local.json', workspace / '.cargo/config.toml',
             workspace / 'api/.cargo/config.toml', workspace / 'api/rust-toolchain.toml',
             Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo'))) / 'config.toml']
    return {str(p): sha(p) for p in paths if p.is_file()}


def phase(workspace, output, target, role, workload, argv, identity, jobs=2):
    cgroup = budget()
    before = admission(workspace)
    before['target_filesystem_admission'] = admission(target)
    configuration = configuration_hashes(workspace)
    profiles = tomllib.loads((workspace / 'api/Cargo.toml').read_text()).get('profile', {})
    atomic_json(output / 'admission.json', before)
    env = environment(workspace, output, target, jobs)
    measured = run_command(argv, workspace if workload == 'external' else workspace / 'api', env, output / 'process', cgroup)
    rows = rustc_rows(output / 'rustc.tsv')
    check = checks(role, workload, measured, rows)
    check['source_and_configuration_unchanged'] = (configuration_hashes(workspace) == configuration
        and git(workspace, 'rev-parse', 'HEAD') == identity['sha']
        and sha(workspace / 'api/Cargo.toml') == identity['manifest_sha256']
        and sha(workspace / 'api/Cargo.lock') == identity['lock_sha256'])
    direct = {}
    for process_role in ['rustc', 'linker-driver', 'linker']:
        observations = [p for p in measured['sampled_processes'] if p['role'] == process_role]
        direct[process_role] = {'observed_pid_count': len(observations),
            'rss_peak_kib': max((p['rss_kib'] for p in observations), default=None),
            'hwm_peak_kib': max((p['hwm_kib'] for p in observations), default=None),
            'scope': 'real PID samples with starttime/exe/cgroup and compiler ancestry; sampled HWM can miss a late exit peak'}
    result = {'version': 1, 'role': role, 'workload': workload, 'source': identity,
              'tool': validate_export(), 'tool_files': bundle_identity(), 'jobs': jobs,
              'profile': profiles,
              'rustc_version': subprocess.check_output(['rustc', '-vV'], text=True),
              'cargo_executable': shutil.which('cargo'), 'raw_command': argv,
              'configuration_hashes': configuration, 'direct_pid_samples': direct,
              'cgroup_observation': measured, 'rustc': rows, 'checks': check,
              'candidate_thresholds_applied': role == 'candidate',
              'target_volume': volume(target), 'artifacts': artifacts(target), 'n': 1}
    atomic_json(output / 'measurement.json', result)
    return result


def measure(args):
    workspace = args.workspace.resolve()
    identity = source(workspace, args.source_sha)
    validate_export()
    config(workspace)
    command(args.workload, args.role, args.command)
    if args.workload in {'incremental', 'external'} and not args.reuse_target_from:
        raise ValueError('Incremental/external observations require an already measured owned target')
    if args.workload == 'external':
        previous = json.loads((args.reuse_target_from / 'measurement.json').read_text())
        if previous['workload'] != 'test2' or previous['source'] != identity or not all(previous['checks'].values()):
            raise ValueError('External observations must reuse completed test2 evidence at this source')
    if args.workload in {'dev2', 'dev10', 'test2'} and args.reuse_target_from:
        raise ValueError('Cold workloads cannot reuse an existing target')
    output = args.output.resolve()
    output.mkdir(exist_ok=False)
    atomic_json(output / 'source.json', identity)
    target, cache_state = owned_target(output, workspace, identity, args.reuse_target_from)
    if args.workload == 'incremental':
        from incremental import run_incremental
        return run_incremental(args, identity, output, target, phase)
    argv = command(args.workload, args.role, args.command)
    result = phase(workspace, output, target, args.role, args.workload, argv, identity,
                   10 if args.workload == 'dev10' else 2)
    result['cache_state'] = cache_state
    result['source_clean_after'] = source(workspace, args.source_sha) == identity
    atomic_json(output / 'measurement.json', result)
    return 0 if all(result['checks'].values()) else 2


def claim_campaign(file, role, workload, source_sha, output):
    # Immutable slot claims prevent accidental repeats of the four cold workloads.
    if workload not in {'dev2', 'dev10', 'test2'}:
        return
    import fcntl
    file.parent.mkdir(parents=True, exist_ok=True)
    with file.open('a+') as stream:
        fcntl.flock(stream, fcntl.LOCK_EX)
        stream.seek(0)
        content = stream.read()
        record = json.loads(content) if content else {'version': 1, 'campaign_id': uuid.uuid4().hex, 'tool_files': bundle_identity(), 'sources': {}, 'claims': {}}
        if record.get('version') != 1 or record.get('tool_files') != bundle_identity():
            raise ValueError('Unknown campaign schema or changed measurement tool')
        if role in record['sources'] and record['sources'][role] != source_sha:
            raise ValueError('Campaign role source changed; return to Root')
        slot = role + ':' + workload
        if slot in record['claims']:
            raise ValueError('Cold workload already claimed, even if it failed; no blind retry')
        record['sources'][role] = source_sha
        record['claims'][slot] = {'output': str(output), 'claimed_at': time.time(),
                                'source_sha': source_sha, 'controller_receipt': str(output / 'controller.json'),
                                'original_job_receipts': str(output / 'job'),
                                'started_receipt': str(output / 'job/started.json'),
                                'command_started_receipt': str(output / 'job/command-started.json'),
                                'finished_receipt': str(output / 'job/finished.json')}
        stream.seek(0)
        stream.truncate()
        json.dump(record, stream, indent=2)
        stream.flush()
        os.fsync(stream.fileno())


def launch(args):
    validate_export()
    if args.deadline_seconds <= 0 or not args.campaign.is_absolute():
        raise ValueError('Deadline must be positive and campaign path must be absolute')
    source(args.workspace, args.source_sha)
    command(args.workload, args.role, args.command)  # Reject role/workload before creating output.
    receipt_root = args.output.resolve()
    receipt_root.mkdir(exist_ok=False)
    declared = os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP']
    group = Path('/sys/fs/cgroup' + declared)
    if group.name != 'issue2824.slice' or group.stat().st_ino != int(os.environ['ONEFLOWBASE_TASK_BUDGET_ID']):
        raise ValueError('Invalid declared existing task slice')
    claim_campaign(args.campaign.resolve(), args.role, args.workload, args.source_sha, receipt_root)
    forwarded = ['--workspace', str(args.workspace.resolve()), '--source-sha', args.source_sha,
                 '--role', args.role, '--workload', args.workload, '--output', str(receipt_root / 'data')]
    if args.reuse_target_from:
        forwarded += ['--reuse-target-from', str(args.reuse_target_from.resolve())]
    if args.command:
        forwarded += ['--', *args.command]
    argv = ['systemd-run', '--user', '--scope', '--slice=' + group.name,
            '--unit=issue2824-audit-' + uuid.uuid4().hex[:12], sys.executable,
            str(helper('resource-job-run.py', args.workspace)), '--deadline-seconds', str(args.deadline_seconds),
            '--output-dir', str(receipt_root / 'job'), '--', sys.executable,
            str(helper('resource-heavy-run.py', args.workspace)), sys.executable,
            str(HERE / 'audit.py'), '_measure', *forwarded]
    atomic_json(receipt_root / 'controller.json', {'argv': argv, 'source_sha': args.source_sha,
                'tool': validate_export(), 'budget': declared, 'campaign': str(args.campaign.resolve()),
                'cold_slot': args.role + ':' + args.workload if args.workload in {'dev2', 'dev10', 'test2'} else None, 'at': time.time()})
    child = subprocess.Popen(argv)
    atomic_json(receipt_root / 'controller-started.json', {'pid': child.pid, 'at': time.time()})
    status = child.wait()
    atomic_json(receipt_root / 'controller-finished.json', {'returncode': status, 'at': time.time(),
                'original_finished_receipt': str(receipt_root / 'job/finished.json'),
                'finished_receipt_present': (receipt_root / 'job/finished.json').is_file(),
                'finished_receipt_sha256': sha(receipt_root / 'job/finished.json') if (receipt_root / 'job/finished.json').is_file() else None})
    return status


def clean(args):
    budget()
    workspace = args.workspace.resolve()
    source(workspace, args.source_sha)
    output = args.output.resolve()
    ownership = json.loads((output / 'target-ownership.json').read_text())
    owner = Path(ownership['owner_output']).resolve()
    if (ownership['workspace'] != str(workspace) or ownership['source_sha'] != args.source_sha
            or ownership['tool_files'] != bundle_identity()
            or Path(ownership['target']).resolve() != owner / 'target'):
        raise ValueError('Cleanup ownership mismatch')
    if (output / 'clean.json').exists():
        raise ValueError('Cleanup receipt already exists; do not blindly resend')
    if not (owner / 'measurement.json').exists() and not (owner / 'incremental.json').exists():
        raise ValueError('Cannot clean before raw measurement/volume receipt exists')
    argv = ['cargo', 'clean', '--manifest-path', str(workspace / 'api/Cargo.toml'), '--target-dir', ownership['target']]
    gate = admission(workspace)
    with (output / 'clean.log').open('w') as log:
        result = subprocess.run(argv, stdout=log, stderr=subprocess.STDOUT)
    atomic_json(output / 'clean.json', {'argv': argv, 'returncode': result.returncode, 'admission': gate,
                'ownership': ownership, 'before_volume_record': str(owner / 'measurement.json'), 'at': time.time()})
    return result.returncode


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    sub = result.add_subparsers(dest='action', required=True)
    for name in ('run', '_measure'):
        item = sub.add_parser(name)
        item.add_argument('--workspace', type=Path, required=True)
        item.add_argument('--source-sha', required=True)
        item.add_argument('--output', type=Path, required=True)
        item.add_argument('--role', choices=['baseline', 'candidate', 'instrumentation'], required=True)
        item.add_argument('--workload', choices=['dev2', 'dev10', 'test2', 'incremental', 'external'], required=True)
        item.add_argument('--reuse-target-from', type=Path)
        if name == 'run':
            item.add_argument('--campaign', type=Path, required=True)
            item.add_argument('--deadline-seconds', type=int, required=True)
        item.add_argument('command', nargs=argparse.REMAINDER)
    item = sub.add_parser('export')
    item.add_argument('--workspace', type=Path, required=True)
    item.add_argument('--source-sha', required=True)
    item.add_argument('--destination', type=Path, required=True)
    item = sub.add_parser('clean')
    item.add_argument('--workspace', type=Path, required=True)
    item.add_argument('--source-sha', required=True)
    item.add_argument('--output', type=Path, required=True, help='measurement data directory, not its wrapper receipt root')
    item = sub.add_parser('recover')
    item.add_argument('--workspace', type=Path, required=True)
    item.add_argument('--backup-directory', type=Path, required=True)
    return result


def main(argv=None):
    args = parser().parse_args(argv)
    if hasattr(args, 'command') and args.command[:1] == ['--']:
        args.command = args.command[1:]
    if args.action == 'export':
        export(args.workspace, args.source_sha, args.destination)
        return 0
    if args.action == 'recover':
        from incremental import recover
        recover(args.workspace.resolve(), args.backup_directory.resolve())
        return 0
    if args.action == 'clean':
        return clean(args)
    return launch(args) if args.action == 'run' else measure(args)


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, KeyError, subprocess.CalledProcessError) as error:
        print('compile audit:', error, file=sys.stderr)
        raise SystemExit(1)
