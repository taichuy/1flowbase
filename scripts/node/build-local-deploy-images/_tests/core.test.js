const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  ensureDeployEnv,
  readImageVersions,
  runLocalDeployImageBuild,
} = require('../core.js');

function createFixture() {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), '1flowbase-local-images-'));
  const deployDir = path.join(repoRoot, 'deploy', 'docker');
  fs.mkdirSync(deployDir, { recursive: true });
  fs.writeFileSync(
    path.join(deployDir, '.env.example'),
    'FLOWBASE_WEB_VERSION=latest\nFLOWBASE_API_SERVER_VERSION=latest\n',
  );
  return { deployDir, repoRoot };
}

test('AC-001 ensureDeployEnv preserves an existing deployment environment file', () => {
  const { deployDir } = createFixture();
  const envPath = path.join(deployDir, '.env');
  fs.writeFileSync(envPath, 'FLOWBASE_WEB_VERSION=custom\nKEEP_SECRET=unchanged\n');

  const result = ensureDeployEnv(deployDir);

  assert.equal(result.created, false);
  assert.equal(fs.readFileSync(envPath, 'utf8'), 'FLOWBASE_WEB_VERSION=custom\nKEEP_SECRET=unchanged\n');
});

test('AC-002 ensureDeployEnv creates a missing environment file from the example', () => {
  const { deployDir } = createFixture();

  const result = ensureDeployEnv(deployDir);

  assert.equal(result.created, true);
  assert.equal(
    fs.readFileSync(result.envPath, 'utf8'),
    fs.readFileSync(path.join(deployDir, '.env.example'), 'utf8'),
  );
});

test('AC-003 readImageVersions uses deployment tags without rewriting them', () => {
  const { deployDir } = createFixture();
  const envPath = path.join(deployDir, '.env');
  fs.writeFileSync(
    envPath,
    'FLOWBASE_WEB_VERSION=local-web\nFLOWBASE_API_SERVER_VERSION=local-api\n',
  );

  assert.deepEqual(readImageVersions(envPath), {
    apiServer: 'local-api',
    web: 'local-web',
  });
});

test('AC-004 build creates only the configured API and web images and never starts Compose', () => {
  const { deployDir, repoRoot } = createFixture();
  fs.writeFileSync(
    path.join(deployDir, '.env'),
    'FLOWBASE_WEB_VERSION=latest\nFLOWBASE_API_SERVER_VERSION=latest\n',
  );
  const calls = [];
  const runCommand = (command, args, options) => {
    calls.push({ command, args, options });
    if (args[0] === 'buildx' && args[1] === 'version') {
      return { status: 1, stdout: '', stderr: '' };
    }
    return { status: 0, stdout: '', stderr: '' };
  };

  const status = runLocalDeployImageBuild({ deployDir, repoRoot, runCommand });

  assert.equal(status, 0);
  assert.deepEqual(
    calls.map(({ command, args }) => [command, ...args]),
    [
      ['docker', 'buildx', 'version'],
      [
        'docker',
        'build',
        '--target',
        'runtime',
        '-f',
        'docker/api-server.Dockerfile',
        '-t',
        'ghcr.io/taichuy/1flowbase-api-server:latest',
        '.',
      ],
      [
        'docker',
        'build',
        '--target',
        'runtime',
        '-f',
        'docker/web.Dockerfile',
        '-t',
        'ghcr.io/taichuy/1flowbase-web:latest',
        '.',
      ],
    ],
  );
  assert.equal(calls.some(({ command }) => command.includes('compose')), false);
  assert.equal(calls[1].options.env.DOCKER_BUILDKIT, '1');
});

test('AC-005 buildx builds loadable runtime images when available', () => {
  const { deployDir, repoRoot } = createFixture();
  const calls = [];
  const runCommand = (command, args, options) => {
    calls.push({ command, args, options });
    return { status: 0, stdout: '', stderr: '' };
  };

  const status = runLocalDeployImageBuild({ deployDir, repoRoot, runCommand });

  assert.equal(status, 0);
  assert.deepEqual(calls[1].args.slice(0, 5), [
    'buildx',
    'build',
    '--load',
    '--target',
    'runtime',
  ]);
});
