'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { verifyChecksums } = require('./manifest');

function command(executable, args, options = {}) {
  try {
    return execFileSync(executable, args, {
      cwd: options.cwd,
      env: options.env,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      maxBuffer: 1024 * 1024,
    }).trim();
  } catch (error) {
    const label = options.label || path.basename(executable);
    throw new Error(`${label} failed with exit code ${error.status ?? 'unknown'}`);
  }
}

function git(repository, args, options = {}) {
  return command('git', ['-C', repository, ...args], options);
}

function requireRepositoryState(name, contract) {
  const revision = git(contract.path, ['rev-parse', 'HEAD'], { label: `${name} revision check` });
  const expected = contract.revision === 'HEAD'
    ? revision
    : git(contract.path, ['rev-parse', contract.revision], { label: `${name} expected revision check` });
  if (revision !== expected) throw new Error(`${name} revision mismatch`);
  if (git(contract.path, ['status', '--porcelain', '--untracked-files=all'], { label: `${name} clean check` })) {
    throw new Error(`${name} worktree must be clean`);
  }
  return { name, path: contract.path, revision, clean: true };
}

function requireRepositoryRevision(name, contract) {
  const revision = git(contract.path, ['rev-parse', 'HEAD'], { label: `${name} revision check` });
  const expected = contract.revision === 'HEAD'
    ? revision
    : git(contract.path, ['rev-parse', contract.revision], { label: `${name} expected revision check` });
  if (revision !== expected) throw new Error(`${name} revision mismatch`);
  return { name, path: contract.path, revision, clean: null };
}

function requireSourceObject(name, contract) {
  git(contract.repository, ['cat-file', '-e', `${contract.revision}^{commit}`], {
    label: `${name} local source object check`,
  });
  return { name, repository: contract.repository, revision: contract.revision };
}

function verifyDockerDatabase(database) {
  const inspected = JSON.parse(command('docker', ['inspect', database.container], {
    label: 'PostgreSQL container inspection',
  }))[0];
  if (!inspected?.State?.Running) throw new Error('PostgreSQL container is not running');
  if (inspected?.Config?.Image !== database.image) throw new Error('PostgreSQL container image mismatch');
  const ports = inspected?.NetworkSettings?.Ports?.['5432/tcp'] || [];
  if (!ports.some((entry) => entry.HostIp === '0.0.0.0' && Number(entry.HostPort) === database.port)) {
    throw new Error('PostgreSQL host port mismatch');
  }
  command('docker', ['image', 'inspect', database.image], { label: 'local PostgreSQL probe image inspection' });
  return { container: database.container, image: database.image, endpoint: `${database.host}:${database.port}` };
}

async function verifyFsWatch() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), '1flowbase-local-acceptance-watch-'));
  const target = path.join(root, 'ready');
  let watcher;
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Node fs.watch readiness preflight timed out')), 2000);
      watcher = fs.watch(root, (_event, filename) => {
        if (filename !== 'ready') return;
        clearTimeout(timer);
        resolve();
      });
      fs.writeFileSync(target, 'ready\n', { mode: 0o600 });
    });
    return { fs_watch: 'pass' };
  } finally {
    watcher?.close();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function preflight(manifest) {
  const repositories = Object.entries(manifest.repo).map(([name, value]) => value.require_clean === false
    ? requireRepositoryRevision(name, value)
    : requireRepositoryState(name, value));
  const sources = Object.entries(manifest.sources).map(([name, value]) => requireSourceObject(name, value));
  const artifacts = verifyChecksums(manifest);
  for (const name of ['apiServer', 'pluginRunner', 'codex', 'claude', 'opencode']) {
    fs.accessSync(manifest.artifacts[name].path, fs.constants.X_OK);
  }
  return {
    repositories,
    sources,
    artifacts,
    database: verifyDockerDatabase(manifest.database),
    readiness: await verifyFsWatch(),
  };
}

function createEvidenceRoot(repoRoot) {
  const stamp = new Date().toISOString().replaceAll(/[:.]/gu, '-');
  const root = path.join(repoRoot, 'tmp/test-governance/compatible-stream-e2e', `local-acceptance-${stamp}`);
  fs.mkdirSync(root, { recursive: true, mode: 0o700 });
  return root;
}

function createDetachedSource(client, contract, evidenceRoot) {
  const sourceRoot = path.join(evidenceRoot, 'source', client);
  fs.mkdirSync(path.dirname(sourceRoot), { recursive: true, mode: 0o700 });
  git(contract.repository, ['worktree', 'add', '--detach', sourceRoot, contract.revision], {
    label: `${client} detached worktree creation`,
  });
  let closed = false;
  return {
    path: sourceRoot,
    async close() {
      if (closed) return;
      closed = true;
      try {
        git(contract.repository, ['worktree', 'remove', '--force', sourceRoot], {
          label: `${client} detached worktree cleanup`,
        });
      } finally {
        fs.rmSync(sourceRoot, { recursive: true, force: true });
        git(contract.repository, ['worktree', 'prune'], { label: `${client} worktree metadata cleanup` });
      }
    },
  };
}

function randomAlphanumeric(prefix) {
  return `${prefix}${crypto.randomBytes(8).toString('hex')}`;
}

function createDatabase(database) {
  const role = randomAlphanumeric('qauser');
  const name = randomAlphanumeric('qadb');
  const password = randomAlphanumeric('qapass');
  const exec = (args, label) => command('docker', ['exec', database.container, ...args], { label });
  exec(['psql', '-U', 'postgres', '-v', 'ON_ERROR_STOP=1', '-c', `CREATE ROLE ${role} LOGIN PASSWORD '${password}'`], 'temporary role creation');
  try {
    exec(['createdb', '-U', 'postgres', '-O', role, name], 'temporary database creation');
  } catch (error) {
    try { exec(['dropuser', '-U', 'postgres', '--if-exists', role], 'temporary role rollback'); } catch {}
    throw error;
  }
  let closed = false;
  return {
    role,
    name,
    url: `postgres://${role}:${password}@${database.host}:${database.port}/${name}`,
    async close() {
      if (closed) return;
      closed = true;
      const errors = [];
      for (const [args, label] of [
        [['psql', '-U', 'postgres', '-v', 'ON_ERROR_STOP=1', '-c', `SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '${name}' AND pid <> pg_backend_pid()`], 'temporary connection cleanup'],
        [['dropdb', '-U', 'postgres', '--if-exists', name], 'temporary database cleanup'],
        [['dropuser', '-U', 'postgres', '--if-exists', role], 'temporary role cleanup'],
      ]) {
        try { exec(args, label); } catch (error) { errors.push(error); }
      }
      if (errors.length) throw errors[0];
    },
  };
}

async function probeDatabase(url, manifest) {
  const parsed = new URL(url);
  const output = command('docker', [
    'run', '--rm', '--network', 'host', manifest.database.image,
    'psql', url, '-v', 'ON_ERROR_STOP=1', '-Atqc',
    "SELECT current_user || '|' || current_database()",
  ], { label: 'same-URL PostgreSQL host-network probe' });
  if (output !== `${decodeURIComponent(parsed.username)}|${parsed.pathname.slice(1)}`) {
    throw new Error('same-URL PostgreSQL host-network probe identity mismatch');
  }
  return { status: 'pass', role: parsed.username, database: parsed.pathname.slice(1) };
}

function writeJson(filePath, value, mode = 0o600) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, { mode });
}

function writeReadyManifest(evidenceRoot, manifest) {
  const filePath = path.join(
    os.tmpdir(),
    `1flowbase-local-acceptance-ready-${process.pid}-${crypto.randomBytes(8).toString('hex')}.json`,
  );
  fs.writeFileSync(filePath, `${JSON.stringify(manifest, null, 2)}\n`, { mode: 0o600, flag: 'wx' });
  return filePath;
}

function writeResult(evidenceRoot, result) {
  writeJson(path.join(evidenceRoot, 'local-acceptance-result.json'), result);
  const checksums = (result.preflight?.artifacts || [])
    .map((artifact) => `${artifact.sha256}  ${artifact.path}`)
    .join('\n');
  fs.writeFileSync(path.join(evidenceRoot, 'artifact-checksums.sha256'), `${checksums}\n`, { mode: 0o600 });
  fs.writeFileSync(path.join(evidenceRoot, 'local-acceptance-report.md'), [
    '# Local AI Gateway Client Diagnostics',
    '',
    `- Status: ${result.status.toUpperCase()}`,
    `- Gate role: ${result.gate_role}`,
    `- Runtime attempts: ${result.runtime_attempts}`,
    `- Database attempts: ${result.database_attempts}`,
    `- Protocol gate: ${result.protocol?.status || 'not-run'}`,
    `- Client diagnostic: ${result.clients?.status || 'not-run'} (non-blocking)`,
    `- Cleanup: ${result.cleanup.status}`,
    `- Error: ${result.error?.message || 'none'}`,
    '',
  ].join('\n'), { mode: 0o600 });
}

async function cleanupTmux() {
  const socketRoot = path.join(os.tmpdir(), `tmux-${process.getuid()}`);
  if (!fs.existsSync(socketRoot)) return;
  const prefix = `oneflowbase-stream-${process.pid}-`;
  for (const socket of fs.readdirSync(socketRoot).filter((name) => name.startsWith(prefix))) {
    try { command('tmux', ['-L', socket, 'kill-server'], { label: 'owned tmux cleanup' }); } catch {}
  }
}

module.exports = {
  cleanupTmux,
  command,
  createDatabase,
  createDetachedSource,
  createEvidenceRoot,
  preflight,
  probeDatabase,
  requireRepositoryState,
  requireRepositoryRevision,
  requireSourceObject,
  writeJson,
  writeReadyManifest,
  writeResult,
};
