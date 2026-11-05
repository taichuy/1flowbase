const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  buildCleanupPlan,
  parseCliArgs,
  runBuildCacheCleanup,
} = require('../core.js');

function writeFixtureFile(filePath, content = 'fixture\n') {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content, 'utf8');
}

test('parseCliArgs defaults to cleaning backend and frontend caches', () => {
  assert.deepEqual(parseCliArgs([]), {
    dryRun: false,
    help: false,
    scope: 'all',
  });
});

test('parseCliArgs supports backend-only and frontend-only scopes', () => {
  assert.equal(parseCliArgs(['--backend-only']).scope, 'backend');
  assert.equal(parseCliArgs(['backend']).scope, 'backend');
  assert.equal(parseCliArgs(['--frontend-only']).scope, 'frontend');
  assert.equal(parseCliArgs(['frontend']).scope, 'frontend');

  assert.throws(
    () => parseCliArgs(['--backend-only', '--frontend-only']),
    /不能同时指定/u
  );
});

test('buildCleanupPlan selects full backend target and frontend build caches', () => {
  const allTargets = buildCleanupPlan({
    repoRoot: '/repo',
    scope: 'all',
  }).targets.map((target) => target.relativePath);

  assert.deepEqual(allTargets, [
    path.join('api', 'target'),
    path.join('web', '.turbo'),
    path.join('web', 'app', '.turbo'),
    path.join('web', 'app', 'dist'),
  ]);

  assert.deepEqual(
    buildCleanupPlan({ repoRoot: '/repo', scope: 'backend' }).targets.map((target) => target.relativePath),
    [path.join('api', 'target')]
  );
  assert.deepEqual(
    buildCleanupPlan({ repoRoot: '/repo', scope: 'frontend' }).targets.map((target) => target.relativePath),
    [
      path.join('web', '.turbo'),
      path.join('web', 'app', '.turbo'),
      path.join('web', 'app', 'dist'),
    ]
  );
});

test('runBuildCacheCleanup dry-run keeps files and does not stop services', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-clean-build-cache-'));
  const backendFile = path.join(repoRoot, 'api', 'target', 'debug', 'deps', 'api_server');
  const frontendFile = path.join(repoRoot, 'web', 'app', 'dist', 'bundle.js');
  const calls = [];

  writeFixtureFile(backendFile);
  writeFixtureFile(frontendFile);

  const status = await runBuildCacheCleanup({
    repoRoot,
    options: parseCliArgs(['--dry-run']),
    stopBackendServicesImpl: async () => calls.push('stop'),
    writeStdout: () => {},
  });

  assert.equal(status, 0);
  assert.equal(fs.existsSync(backendFile), true);
  assert.equal(fs.existsSync(frontendFile), true);
  assert.deepEqual(calls, []);
});

test('runBuildCacheCleanup stops backend services before removing selected caches', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-clean-build-cache-'));
  const backendFile = path.join(repoRoot, 'api', 'target', 'debug', 'deps', 'api_server');
  const frontendFile = path.join(repoRoot, 'web', 'app', 'dist', 'bundle.js');
  const events = [];

  writeFixtureFile(backendFile);
  writeFixtureFile(frontendFile);

  const status = await runBuildCacheCleanup({
    repoRoot,
    options: parseCliArgs(['--backend-only']),
    removePathImpl(targetPath) {
      events.push(`rm:${path.relative(repoRoot, targetPath)}`);
      fs.rmSync(targetPath, { recursive: true, force: true });
    },
    stopBackendServicesImpl: async () => events.push('stop'),
    writeStdout: () => {},
  });

  assert.equal(status, 0);
  assert.deepEqual(events, ['stop', `rm:${path.join('api', 'target')}`]);
  assert.equal(fs.existsSync(path.join(repoRoot, 'api', 'target')), false);
  assert.equal(fs.existsSync(frontendFile), true);
});

test('ownership stop failure preserves every selected cache', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-owner-boundary-'));
  const backendFile = path.join(repoRoot, 'api', 'target', 'debug', 'api-server');
  const frontendFile = path.join(repoRoot, 'web', 'app', 'dist', 'bundle.js');
  writeFixtureFile(backendFile);
  writeFixtureFile(frontendFile);
  const removed = [];
  try {
    await assert.rejects(runBuildCacheCleanup({
      repoRoot,
      options: parseCliArgs([]),
      stopBackendServicesImpl: async () => { throw new Error('ownership not proven'); },
      removePathImpl: (value) => removed.push(value),
      writeStdout: () => {},
    }), /ownership/u);
    assert.deepEqual(removed, []);
    assert.equal(fs.readFileSync(backendFile, 'utf8'), 'fixture\n');
    assert.equal(fs.existsSync(frontendFile), true);
  } finally {
    fs.rmSync(repoRoot, { recursive: true, force: true });
  }
});
