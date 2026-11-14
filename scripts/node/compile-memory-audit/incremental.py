"""The original get_session mutation, with a durable hash-guarded recovery owner."""
import difflib
import hashlib
import json
import os
from pathlib import Path
import signal
import stat
import tempfile

from sampling import atomic_json

HANDLERS = ('api/crates/api/console-interaction/src/routes/identity/session.rs',
            'api/apps/api-server/src/routes/identity/session.rs')
OLD = b'Ok(Json(ApiSuccess::new(response)))'
NEW = b'Ok(Json(ApiSuccess::new({ let session_response = response; session_response })))'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def mutation(data):
    anchor = b'pub async fn get_session('
    if data.count(anchor) != 1:
        raise ValueError('Expected exactly one real get_session function')
    start = data.index(anchor)
    end = data.index(b'\n#[utoipa::path(', start)
    if data[start:end].count(OLD) != 1:
        raise ValueError('Original get_session response anchor changed')
    return data[:start] + data[start:end].replace(OLD, NEW, 1) + data[end:]


def replace(file, data, mode):
    fd, temporary = tempfile.mkstemp(prefix='.compile-audit-', dir=file.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
            os.fchmod(stream.fileno(), mode)
        os.replace(temporary, file)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def prepare(workspace, directory, source_sha):
    selected = [workspace / p for p in HANDLERS if (workspace / p).is_file()
                and b'pub async fn get_session(' in (workspace / p).read_bytes()]
    if len(selected) != 1:
        raise ValueError('Canonical session owner is missing or ambiguous')
    file = selected[0]
    if file.is_symlink() or not file.resolve().is_relative_to(workspace.resolve()):
        raise ValueError('Handler must be a real file inside the source')
    original = file.read_bytes()
    changed = mutation(original)
    directory.mkdir(exist_ok=False)
    with (directory / 'original.bytes').open('xb') as stream:
        stream.write(original)
        stream.flush()
        os.fsync(stream.fileno())
    record = {'workspace': str(workspace), 'source_sha': source_sha,
              'handler': str(file.relative_to(workspace)), 'original_sha256': digest(original),
              'changed_sha256': digest(changed), 'mode': stat.S_IMODE(file.stat().st_mode)}
    atomic_json(directory / 'backup.json', record)
    patch = ''.join(difflib.unified_diff(original.decode().splitlines(True), changed.decode().splitlines(True),
                                       fromfile=record['handler'], tofile=record['handler']))
    (directory / 'mutation.patch').write_text(patch)
    return file, changed, record


def recover(workspace, directory):
    from audit import git
    record = json.loads((directory / 'backup.json').read_text())
    if record['workspace'] != str(workspace) or git(workspace, 'rev-parse', 'HEAD') != record['source_sha']:
        raise ValueError('Backup belongs to a different workspace/source; manual inspection required')
    if record['handler'] not in HANDLERS:
        raise ValueError('Backup does not name an approved session owner')
    file = workspace / record['handler']
    original = (directory / 'original.bytes').read_bytes()
    if digest(original) != record['original_sha256']:
        raise ValueError('Original backup bytes changed')
    if file.is_symlink() or digest(file.read_bytes()) not in {record['original_sha256'], record['changed_sha256']}:
        raise ValueError('Handler changed independently; refuse to overwrite it')
    replace(file, original, record['mode'])
    atomic_json(directory / 'restored.json', {'sha256': digest(file.read_bytes()), 'exact_original_bytes': True})


def run_incremental(args, identity, output, target, phase):
    from audit import command, source
    if not args.reuse_target_from:
        raise ValueError('Matched incremental requires the already measured dev2 target, no fourth cold build')
    owner = json.loads((args.reuse_target_from / 'measurement.json').read_text())
    if owner['workload'] != 'dev2' or owner['source'] != identity or not all(owner['checks'].values()):
        raise ValueError('Incremental warm target must come from the completed dev2 measurement')
    backup = output / 'recovery'
    file, changed, record = prepare(args.workspace.resolve(), backup, args.source_sha)
    old_handlers = {}
    def cancelled(signum, _frame):
        raise RuntimeError('Incremental cancellation signal ' + str(signum))
    for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        old_handlers[sig] = signal.signal(sig, cancelled)
    result = {'source': identity, 'handler': record, 'warmup_cache_state':
              'cached dev2 target; extra bin artifacts pre-exist but exact-bin workload does not consume them',
              'comparison_scope': 'only mutation phase matches original Scout exact-bin incremental; warmup is separately cached',
              'warmup': None, 'mutation': None}
    try:
        warm = output / 'warmup'
        warm.mkdir()
        result['warmup'] = phase(args.workspace.resolve(), warm, target, 'instrumentation', 'exact-bin-warmup',
                                 command('incremental', args.role), identity)
        if not result['warmup']['checks']['workload_completed']:
            raise RuntimeError('Exact-bin warmup failed or was cancelled')
        if file.read_bytes() != (backup / 'original.bytes').read_bytes():
            raise ValueError('Handler changed before owned mutation')
        replace(file, changed, record['mode'])
        altered = dict(identity, intentional_diff_sha256=digest((backup / 'mutation.patch').read_bytes()),
                       handler_original_sha256=record['original_sha256'], handler_changed_sha256=record['changed_sha256'])
        stage = output / 'mutation'
        stage.mkdir()
        result['mutation'] = phase(args.workspace.resolve(), stage, target, args.role, 'incremental',
                                   command('incremental', args.role), altered)
    finally:
        try:
            recover(args.workspace.resolve(), backup)
            result['source_restored_clean'] = source(args.workspace.resolve(), args.source_sha) == identity
        finally:
            for sig, handler in old_handlers.items():
                signal.signal(sig, handler)
            atomic_json(output / 'incremental.json', result)
    return 0 if result['mutation'] and all(result['mutation']['checks'].values()) else 2
