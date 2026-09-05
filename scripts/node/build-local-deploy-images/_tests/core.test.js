const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  ensureDeployEnv,
  readImageVersions,
  removeBuildKitRunMounts,
  runLocalDeployImageBuild,
} = require('../core.js');

const projectRoot = path.resolve(__dirname, '..', '..', '..', '..');

function createFixture() {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), '1flowbase-local-images-'));
  const deployDir = path.join(repoRoot, 'deploy', 'docker');
  const dockerDir = path.join(repoRoot, 'docker');
  fs.mkdirSync(deployDir, { recursive: true });
  fs.mkdirSync(dockerDir, { recursive: true });
  fs.writeFileSync(
    path.join(deployDir, '.env.example'),
    'FLOWBASE_WEB_VERSION=latest\nFLOWBASE_API_SERVER_VERSION=latest\n',
  );
  fs.writeFileSync(
    path.join(dockerDir, 'api-server.Dockerfile'),
    '# syntax=docker/dockerfile:1.7\nFROM scratch AS runtime\nRUN --mount=type=cache,target=/cache \\\n+    echo api\n',
  );
  fs.writeFileSync(
    path.join(dockerDir, 'web.Dockerfile'),
    '# syntax=docker/dockerfile:1.7\nFROM scratch AS runtime\nRUN --mount=type=cache,target=/cache \\\n+    echo web\n',
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

test('AC-004 missing buildx uses temporary legacy-compatible Dockerfiles without installing anything', () => {
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
    const dockerfileIndex = args.indexOf('-f');
    if (dockerfileIndex !== -1) {
      const dockerfile = fs.readFileSync(args[dockerfileIndex + 1], 'utf8');
      assert.doesNotMatch(dockerfile, /--mount=/u);
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
        '--build-arg',
        'TARGETOS=linux',
        '--build-arg',
        `TARGETARCH=${process.arch === 'arm64' ? 'arm64' : 'amd64'}`,
        '-f',
        calls[1].args[calls[1].args.indexOf('-f') + 1],
        '-t',
        'ghcr.io/taichuy/1flowbase-api-server:latest',
        '.',
      ],
      [
        'docker',
        'build',
        '--target',
        'runtime',
        '--build-arg',
        'TARGETOS=linux',
        '--build-arg',
        `TARGETARCH=${process.arch === 'arm64' ? 'arm64' : 'amd64'}`,
        '-f',
        calls[2].args[calls[2].args.indexOf('-f') + 1],
        '-t',
        'ghcr.io/taichuy/1flowbase-web:latest',
        '.',
      ],
    ],
  );
  assert.equal(calls.some(({ command }) => command.includes('compose')), false);
  assert.equal(calls[1].options.env.DOCKER_BUILDKIT, '0');
  assert.match(calls[1].args[calls[1].args.indexOf('-f') + 1], /tmp\/local-deploy-images-/u);
  assert.equal(fs.existsSync(calls[1].args[calls[1].args.indexOf('-f') + 1]), false);
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

test('AC-006 Docker context excludes deployment data that the local user may not own', () => {
  const dockerIgnore = fs.readFileSync(path.join(projectRoot, '.dockerignore'), 'utf8');

  assert.match(dockerIgnore, /^deploy$/mu);
  assert.match(dockerIgnore, /^docker\/volumes$/mu);
});

test('AC-007 legacy API build caches downloads separately and limits Cargo compile concurrency', () => {
  const source = `FROM rust:1-slim AS builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \\
    --mount=type=cache,target=/workspace/api/target-cache \\
    CARGO_TARGET_DIR=/workspace/api/target-cache \\
      cargo build --release -p api-server
`;

  const transformed = removeBuildKitRunMounts(source, 'api-server.Dockerfile', { cargoJobs: 2 });

  assert.match(transformed, /RUN cargo fetch --locked\n\nRUN \\\n/u);
  assert.match(transformed, /CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/u);
  assert.doesNotMatch(transformed, /--mount=/u);
});
