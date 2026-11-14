#!/usr/bin/env python3
"""Issue #2824 only: mark a verified, measured fresh target before Cargo cleanup.

This standalone helper never deletes files or runs Cargo. A missing CACHEDIR.TAG
is the only permitted target write; an existing marker is always left untouched.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import time

sys.dont_write_bytecode = True
FILES = ('audit.py', 'sampling.py', 'incremental.py', 'report.py', 'rustc-wrap.sh')
FRESH = 'fresh exclusive task-owned target; registry/download caches preserved'
SIGNATURE = b'Signature: 8a477f597d28d172789f06886806bc55\n'
BUDGET = '/user.slice/user-1000.slice/user@1000.service/issue2824.slice'
BUDGET_INODE = 14758159


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(file):
    result = hashlib.sha256()
    with file.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(chunk)
    return result.hexdigest()


def canonical(file):
    file = Path(os.path.abspath(file))
    require(file.resolve(strict=True) == file, 'Symlink/noncanonical input path: ' + str(file))
    for parent in (file, *file.parents):
        require(not stat.S_ISLNK(parent.lstat().st_mode), 'Symlink input path: ' + str(parent))
    return file


def git(workspace, *args):
    return subprocess.check_output(['git', '-C', str(workspace), *args], stderr=subprocess.PIPE).decode().strip()


def source(workspace, expected):
    workspace = canonical(workspace)
    require(git(workspace, 'rev-parse', '--show-toplevel') == str(workspace), 'Workspace must be the real git root')
    require(git(workspace, 'rev-parse', 'HEAD') == expected, 'Source SHA mismatch')
    require(not git(workspace, 'status', '--porcelain'), 'Workspace source is not clean')
    return {'workspace': str(workspace), 'sha': expected,
            'manifest_sha256': digest(canonical(workspace / 'api/Cargo.toml')),
            'lock_sha256': digest(canonical(workspace / 'api/Cargo.lock'))}


def budget():
    require(os.environ.get('ONEFLOWBASE_TASK_BUDGET_CGROUP') == BUDGET
            and os.environ.get('ONEFLOWBASE_TASK_BUDGET_ID') == str(BUDGET_INODE), 'Explicit original budget declaration required')
    group = Path('/sys/fs/cgroup' + BUDGET)
    actual = Path('/sys/fs/cgroup' + Path('/proc/self/cgroup').read_text().strip().split('::', 1)[1])
    require(actual.is_relative_to(group) and group.stat().st_ino == BUDGET_INODE
            and (group / 'cpu.max').read_text().strip() == '1000000 100000'
            and (group / 'memory.max').read_text().strip() == '18253611008'
            and (group / 'memory.high').read_text().strip() == '17179869184', 'Actual original budget identity/limits mismatch')
    return {'cgroup': str(actual), 'budget': str(group), 'inode': BUDGET_INODE}


def no_cargo():
    found = []
    for proc in Path('/proc').iterdir():
        if not proc.name.isdigit():
            continue
        try:
            name = (proc / 'comm').read_text().strip()
        except (FileNotFoundError, ProcessLookupError):
            continue
        try:
            executable = os.readlink(proc / 'exe')
        except (FileNotFoundError, ProcessLookupError):
            continue
        except PermissionError:
            executable = ''  # /proc/comm remains the global Cargo identity for other users.
        if name == 'cargo' or Path(executable).name == 'cargo':
            found.append(int(proc.name))
    require(not found, 'Active Cargo exists; never stop unrelated processes: ' + str(found))


def admission(paths):
    no_cargo()
    def cpu():
        return list(map(int, Path('/proc/stat').read_text().splitlines()[0].split()[1:9]))
    before = cpu()
    time.sleep(.25)
    delta = [b - a for a, b in zip(before, cpu())]
    memory = {line.split(':')[0]: int(line.split()[1]) for line in Path('/proc/meminfo').read_text().splitlines()}
    disks = [os.statvfs(p) for p in paths]
    result = {'cpu_used': 1 - (delta[3] + delta[4]) / sum(delta) if sum(delta) else 0,
              'physical_ram_used': 1 - memory['MemAvailable'] / memory['MemTotal'],
              'disk_used': max(1 - d.f_bfree / d.f_blocks for d in disks), 'at': time.time()}
    require(max(result['cpu_used'], result['physical_ram_used'], result['disk_used']) <= .9, 'Machine resources exceed 90%')
    no_cargo()
    return result


def volume(target):
    seen = set()
    result = dict(apparent_bytes=0, allocated_bytes=0, entries=0, unique_inodes=0, symlinks=0)
    def visit(file):
        info = file.lstat()
        result['entries'] += 1
        key = info.st_dev, info.st_ino
        if key in seen:
            return
        seen.add(key)
        result['unique_inodes'] += 1
        result['apparent_bytes'] += info.st_size
        result['allocated_bytes'] += info.st_blocks * 512
        require(not stat.S_ISLNK(info.st_mode), 'Symlink inside measured target: ' + str(file))
        if stat.S_ISDIR(info.st_mode):
            for child in file.iterdir():
                visit(child)
    visit(target)
    result['accounting'] = 'lstat includes directory metadata; unique dev/inode; symlinks not followed'
    return result


def artifact_identity(target, artifacts):
    bins = artifacts.get('bins')
    harnesses = artifacts.get('test_or_bin_harness_executables')
    require(isinstance(bins, list) and isinstance(harnesses, list) and (bins or harnesses), 'Measured produced artifacts are required')
    hashed_inodes = {}
    for item in bins + harnesses:
        file = canonical(Path(item['path']))
        require(file.is_relative_to(target) and file.is_file(), 'Measured artifact escaped target')
        info = file.stat()
        key = info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns
        if key not in hashed_inodes:
            hashed_inodes[key] = digest(file)
        require((info.st_dev, info.st_ino, info.st_size, info.st_blocks * 512) ==
                (item['device'], item['inode'], item['apparent_bytes'], item['allocated_bytes'])
                and hashed_inodes[key] == item['sha256'], 'Measured artifact identity/hash mismatch: ' + str(file))
    return {'bins': len(bins), 'harness_executables': len(harnesses)}


def verified_inputs(workspace, expected, data, tools, observation=None):
    identity = source(workspace, expected)
    data, tools = canonical(data), canonical(tools)
    require(data.name == 'data', 'Measurement input must be the original data directory')
    hashes = {}
    def load(file):
        file = canonical(file)
        require(file.is_file(), 'Missing regular receipt: ' + str(file))
        raw = file.read_bytes()
        hashes[str(file)] = hashlib.sha256(raw).hexdigest()
        return json.loads(raw)
    observation = canonical(observation or data)
    ownership = load(data / 'target-ownership.json')
    owner_measurement = load(data / 'measurement.json')
    require(load(data / 'source.json') == identity and owner_measurement['source'] == identity,
            'Owner measurement source/manifest/lock mismatch')
    exported = load(tools / 'export.json')
    expected_names = set(FILES) | {'resource-job-run.py', 'resource-heavy-run.py', 'resource-budget-context.sh'}
    require(set(exported['files']) == expected_names, 'Unknown original tool export file set')
    tool_hashes = {name: digest(canonical(tools / name)) for name in FILES}
    require(all(digest(canonical(tools / name)) == value for name, value in exported['files'].items()),
            'Original exported tool/helper bytes changed')
    require(ownership['workspace'] == str(workspace) and ownership['source_sha'] == expected
            and ownership['owner_output'] == str(data) and ownership['target'] == str(data / 'target')
            and ownership['cache_state'] == FRESH and owner_measurement['cache_state'] == FRESH,
            'Fresh target ownership mismatch')
    target = canonical(data / 'target')
    require(target.is_dir(), 'Owned target is not a directory')

    def validate_measurement(measured, phase, job_data, source_identity, source_clean=True):
        require(measured['source'] == source_identity and measured['tool_files'] == tool_hashes
                and measured['tool'] == exported, 'Observation source/tool identity mismatch')
        require(measured.get('version') == 1 and measured.get('n') == 1
                and (not source_clean or measured.get('source_clean_after') is True)
                and measured.get('jobs') in (2, 10) and measured.get('checks')
                and all(value is True for value in measured['checks'].values()),
                'Measurement did not complete successfully')
        observed = measured['cgroup_observation']
        require(observed == load(phase / 'process/result.json') and observed['returncode'] == 0
                and not observed['error'] and not observed['cancel_signals']
                and observed['cleanup'] == {'process_group_empty': True, 'principal_reaped': True}
                and not any(observed['memory_events_delta'].get(k, 0) for k in ('oom', 'oom_kill', 'oom_group_kill')),
                'Raw measured process did not succeed and clean up')
        job_dir = job_data.parent / 'job'
        job = load(job_dir / 'job.json')
        started = load(job_dir / 'started.json')
        command_start = load(job_dir / 'command-started.json')
        command_finish = load(job_dir / 'command-finished.json')
        finished = load(job_dir / 'finished.json')
        argv = started['argv']
        def argument(key, value):
            return any(argv[index:index + 2] == [key, value] for index in range(len(argv) - 1))
        require(argument('--workspace', str(workspace)) and argument('--source-sha', expected)
                and (argument('--output', str(job_data)) or argument('--data-dir', str(job_data))),
                'Original managed command does not bind this workspace/source/data')
        require(finished['job'] == job and started['unit'] == job['unit']
                and job['cgroup'].startswith(BUDGET + '/')
                and started['declared_budget'] == {'cgroup': BUDGET, 'inode': str(BUDGET_INODE)}
                and finished['budget'] == {'cgroup': BUDGET, 'inode': BUDGET_INODE}
                and command_start['cgroup'] == job['cgroup']
                and observed['cgroup'] == '/sys/fs/cgroup' + job['cgroup']
                and finished['command'] == command_finish and command_finish['kind'] == 'exit'
                and command_finish['returncode'] == 0 and command_finish['exit_code'] == 0
                and finished['classification'] == 'exit' and finished['runner_exit_code'] == 0
                and all(finished['cleanup'].get(k) is True for k in ('complete', 'cgroup_empty', 'launcher_reaped')),
                'Original corresponding managed job did not succeed and clean up')
        process_start = load(phase / 'process/started.json')
        require(process_start['argv'] == measured['raw_command'] == observed['argv']
                and process_start['identity']['cgroup'].strip() == '0::' + job['cgroup'],
                'Measured command/job identity mismatch')
        return job

    validate_measurement(owner_measurement, data, data, identity)
    selected_identity = identity
    measured = owner_measurement
    phase = data
    if observation != data:
        require(observation.name == 'data' and load(observation / 'source.json') == identity,
                'Observation must be an existing source-bound data directory')
        linked = load(observation / 'target-ownership.json')
        require(all(linked[key] == ownership[key] for key in
                    ('target', 'owner_output', 'workspace', 'source_sha', 'tool_files'))
                and linked['cache_state'] == 'warm existing task-owned target', 'Observation ownership chain mismatch')
        if (observation / 'incremental.json').exists():
            incremental = load(observation / 'incremental.json')
            measured = load(observation / 'mutation/measurement.json')
            backup = load(observation / 'recovery/backup.json')
            restored = load(observation / 'recovery/restored.json')
            require(incremental['source'] == identity and incremental['source_restored_clean'] is True
                    and incremental['mutation'] == measured and incremental['handler'] == backup
                    and incremental['warmup']['checks']['workload_completed'] is True,
                    'Incremental result/restoration mismatch')
            allowed = ('api/crates/api/console-interaction/src/routes/identity/session.rs',
                       'api/apps/api-server/src/routes/identity/session.rs')
            require(backup['workspace'] == str(workspace) and backup['source_sha'] == expected
                    and backup['handler'] in allowed, 'Unknown incremental handler/source')
            handler = canonical(workspace / backup['handler'])
            original = canonical(observation / 'recovery/original.bytes')
            hashes[str(original)] = digest(original)
            committed = subprocess.check_output(['git', '-C', str(workspace), 'show', expected + ':' + backup['handler']])
            require(handler.read_bytes() == original.read_bytes() == committed
                    and digest(handler) == backup['original_sha256']
                    and stat.S_IMODE(handler.stat().st_mode) == backup['mode']
                    and restored == {'sha256': backup['original_sha256'], 'exact_original_bytes': True},
                    'Incremental handler is not restored to exact committed bytes/mode')
            patch = canonical(observation / 'recovery/mutation.patch')
            hashes[str(patch)] = digest(patch)
            selected_identity = dict(identity, intentional_diff_sha256=hashes[str(patch)],
                                     handler_original_sha256=backup['original_sha256'],
                                     handler_changed_sha256=backup['changed_sha256'])
            require(measured['workload'] == 'incremental', 'Unknown incremental observation')
            phase = observation / 'mutation'
        else:
            measured = load(observation / 'measurement.json')
            require(measured['workload'] == 'external' and measured['cache_state'] == 'warm existing task-owned target',
                    'Only an existing external or incremental observation may replace the cold snapshot')
            phase = observation
    job = validate_measurement(measured, phase, observation, selected_identity, phase == observation)
    for file, value in measured['configuration_hashes'].items():
        require(digest(canonical(Path(file))) == value, 'Measured configuration changed: ' + file)
    require(volume(target) == measured['target_volume'], 'Measured target volume changed before repair')
    artifact_counts = artifact_identity(target, measured['artifacts'])
    return target, hashes, {'source': identity, 'ownership': ownership, 'job': job,
                            'tool_export': exported, 'target_volume': measured['target_volume'],
                            'artifact_counts': artifact_counts, 'observation_data_directory': str(observation),
                            'selected_measurement': str(phase / 'measurement.json'),
                            'root_identity_scope': 'current repair-time directory only; historical root inode was not recorded'}


def unchanged(hashes):
    for file, value in hashes.items():
        require(digest(canonical(Path(file))) == value, 'Original evidence changed during validation: ' + file)


def create_marker(target, content, expected_stat):
    require(not os.path.lexists(target / 'CACHEDIR.TAG'), 'Existing CACHEDIR.TAG is never overwritten, valid or invalid')
    directory = os.open(target, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        info = os.fstat(directory)
        require((info.st_dev, info.st_ino) == expected_stat, 'Target directory changed during repair')
        marker = os.open('CACHEDIR.TAG', os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o644, dir_fd=directory)
        with os.fdopen(marker, 'wb') as stream:
            stream.write(content)
            stream.flush()
            os.fsync(stream.fileno())
        os.fsync(directory)
        final = canonical(target).stat()
        require((final.st_dev, final.st_ino) == expected_stat, 'Target path changed during repair; inspect receipt, no retry')
    finally:
        os.close(directory)


def repair(args):
    workspace, data, tools = canonical(args.workspace), canonical(args.data_dir), canonical(args.tool_directory)
    observation = canonical(args.observation_data_dir) if args.observation_data_dir else data
    parent = canonical(args.receipt_directory.parent)
    receipt = parent / args.receipt_directory.name
    require(not receipt.is_relative_to(data) and not receipt.is_relative_to(observation) and not receipt.is_relative_to(tools), 'Repair receipt must be outside original data and tool export')
    receipt.mkdir(exist_ok=False)
    record = {'version': 1, 'action': 'issue2824-missing-cache-tag-only', 'helper_sha256': digest(Path(__file__)),
              'workspace': str(workspace), 'expected_source_sha': args.source_sha, 'data_directory': str(data),
              'receipt_directory': str(receipt), 'started_at': time.time(), 'status': 'validating'}
    def publish():
        temporary = receipt / 'repair.json.tmp'
        temporary.write_text(json.dumps(record, indent=2, allow_nan=False) + '\n')
        temporary.replace(receipt / 'repair.json')
    publish()
    guard_fd = None
    try:
        record['budget'] = budget()
        record['admission'] = admission((workspace, data, tools, receipt))
        guard_fd = os.open(canonical(data / 'target'), os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        pinned = os.fstat(guard_fd)
        require(not os.path.lexists(data / 'target/CACHEDIR.TAG'), 'Existing CACHEDIR.TAG is never overwritten, valid or invalid')
        target, hashes, evidence = verified_inputs(workspace, args.source_sha, data, tools, observation)
        require(not os.path.lexists(target / 'CACHEDIR.TAG'), 'Existing CACHEDIR.TAG is never overwritten, valid or invalid')
        root = target.stat()
        expected_stat = pinned.st_dev, pinned.st_ino
        require((root.st_dev, root.st_ino) == expected_stat, 'Target directory changed during validation')
        record.update(evidence=evidence, original_receipt_hashes=hashes,
                      target_identity_at_repair={'device': root.st_dev, 'inode': root.st_ino})
        record['final_admission'] = admission((workspace, target, receipt))
        unchanged(hashes)
        require(source(workspace, args.source_sha) == evidence['source'], 'Source changed during validation')
        content = SIGNATURE + (f'# Issue #2824: missing cache metadata only; created by verified task helper.\n'
            f'# Source: {args.source_sha}\n# Measurement: {data}\n# Repair receipt: {receipt / "repair.json"}\n').encode()
        record.update(status='authorized-marker-create', marker_sha256=hashlib.sha256(content).hexdigest())
        publish()
        create_marker(target, content, expected_stat)
        unchanged(hashes)
        record.update(status='created', completed_at=time.time(), marker_path=str(target / 'CACHEDIR.TAG'))
        publish()
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        record.update(status='refused-or-incomplete', error=str(error), completed_at=time.time())
        publish()
        raise
    finally:
        if guard_fd is not None:
            os.close(guard_fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace', required=True, type=Path)
    parser.add_argument('--source-sha', required=True)
    parser.add_argument('--data-dir', required=True, type=Path)
    parser.add_argument('--observation-data-dir', type=Path, help='explicit latest existing external/incremental data on the same ownership chain')
    parser.add_argument('--tool-directory', required=True, type=Path)
    parser.add_argument('--receipt-directory', required=True, type=Path)
    return repair(parser.parse_args())


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        print('cache-tag repair refused:', error, file=sys.stderr)
        raise SystemExit(1)
