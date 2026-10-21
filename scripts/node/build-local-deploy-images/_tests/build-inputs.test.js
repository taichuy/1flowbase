const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const projectRoot = path.resolve(__dirname, '../../../..');

function temporaryRoot(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'flowbase-build-inputs-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  return root;
}

test('API source build includes the local Cargo patch packages', (t) => {
  const root = temporaryRoot(t);
  const source = fs.readFileSync(path.join(projectRoot, 'docker/api-server.Dockerfile'), 'utf8');
  const builder = source.split(/^FROM /mu)[1];
  for (const line of builder.split('\n')) {
    if (!line.startsWith('COPY ')) continue;
    const entries = line.slice(5).trim().split(/\s+/u);
    const destination = entries.pop();
    for (const entry of entries) {
      const target = path.resolve(root, destination, destination.endsWith('/') ? path.basename(entry) : '');
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.cpSync(path.join(projectRoot, entry), target, { recursive: true });
    }
  }
  const manifest = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
  const patches = [...manifest.matchAll(/^tungstenite = \{ path = "([^"]+)" \}/gmu)];
  assert.equal(patches.length, 2);
  for (const [, relative] of patches) {
    assert.ok(fs.existsSync(path.join(root, relative, 'Cargo.toml')), `Missing patch manifest: ${relative}`);
    assert.ok(fs.existsSync(path.join(root, relative, 'src/lib.rs')), `Missing patch source: ${relative}`);
  }
});

test('Docker excludes private build inputs while retaining templates and vendor source', (t) => {
  const available = spawnSync('docker', ['buildx', 'version'], { encoding: 'utf8', timeout: 10000 });
  const daemon = available.status === 0
    ? spawnSync('docker', ['info', '--format', '{{.OSType}}'], { encoding: 'utf8', timeout: 10000 })
    : available;
  if (available.status !== 0 || daemon.status !== 0 || daemon.stdout.trim() !== 'linux') {
    t.skip('Requires a Linux Docker daemon and buildx');
    return;
  }
  const root = temporaryRoot(t);
  const context = path.join(root, 'context');
  const output = path.join(root, 'output');
  const excluded = [
    '.env', '.env.local', 'nested/.env.production.local', 'docker/middleware.env',
    '.memory/user-memory.md', 'nested/.memory/private.md',
  ];
  const included = [
    '.env.example', 'api/.env.production.example', 'web/app/.env.example',
    'docker/middleware.env.example', 'api/vendor/patched/Cargo.toml', 'web/app/src/main.ts',
  ];
  for (const filename of [...excluded, ...included]) {
    const target = path.join(context, filename);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, 'fixture');
  }
  fs.copyFileSync(path.join(projectRoot, '.dockerignore'), path.join(context, '.dockerignore'));
  fs.writeFileSync(path.join(context, 'Dockerfile'), 'FROM scratch\nCOPY . /\n');
  const result = spawnSync('docker', [
    'buildx', 'build', '--network=none', '--progress=plain',
    '--output', `type=local,dest=${output}`, context,
  ], { encoding: 'utf8', timeout: 60000 });
  assert.equal(result.status, 0, result.stderr || result.error?.message);
  for (const filename of excluded) assert.equal(fs.existsSync(path.join(output, filename)), false, filename);
  for (const filename of included) assert.equal(fs.existsSync(path.join(output, filename)), true, filename);
});
