const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  createLegacyDockerfiles,
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

  const status = runLocalDeployImageBuild({ deployDir, repoRoot, runCommand, env: {} });

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

  const status = runLocalDeployImageBuild({ deployDir, repoRoot, runCommand, env: {} });

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

for (const useBuildx of [false, true]) {
  test(`AC-NET-1 ${useBuildx ? 'buildx' : 'legacy'} forwards proxy names and selected network`, (t) => {
    const { deployDir, repoRoot } = createFixture();
    t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
    const calls = [];
    const env = {
      HTTPS_PROXY: 'http://user:secret@127.0.0.1:7897',
      no_proxy: 'localhost,127.0.0.1',
      FLOWBASE_DOCKER_BUILD_NETWORK: 'host',
    };
    runLocalDeployImageBuild({
      deployDir, repoRoot, env, writeStdout: () => {},
      runCommand: (command, args, options) => {
        calls.push({ args, options });
        return { status: args[0] === 'buildx' && args[1] === 'version' && !useBuildx ? 1 : 0 };
      },
    });
    for (const call of calls.filter(({ args }) => args.includes('-f'))) {
      assert.equal(call.args[call.args.indexOf('--network') + 1], 'host');
      assert.ok(call.args.includes('HTTPS_PROXY'));
      assert.ok(call.args.includes('no_proxy'));
      assert.ok(!call.args.some((arg) => arg.includes('secret')));
      assert.equal(call.options.env.HTTPS_PROXY, env.HTTPS_PROXY);
      if (useBuildx) assert.ok(call.args.includes('network.host'));
    }
  });
}

test('AC-NET-2 local Linux loopback proxy uses host network, remote daemon does not', (t) => {
  if (process.platform !== 'linux') return t.skip('Linux host networking');
  const { deployDir, repoRoot } = createFixture();
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  for (const endpoint of ['unix:///var/run/docker.sock', 'tcp://remote:2376']) {
    const calls = [];
    runLocalDeployImageBuild({
      deployDir, repoRoot, env: { https_proxy: 'http://127.0.0.1:7897' }, writeStdout: () => {},
      runCommand: (command, args) => {
        calls.push(args);
        return { status: 0, stdout: endpoint };
      },
    });
    const builds = calls.filter((args) => args.includes('-f'));
    assert.equal(builds.length, 2);
    for (const args of builds) assert.equal(args.includes('--network'), endpoint.startsWith('unix:'));
  }
});

test('AC-WEB-1 image dependency layer can execute the real preinstall checks', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'flowbase-web-preinstall-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const source = fs.readFileSync(path.join(projectRoot, 'docker/web.Dockerfile'), 'utf8');
  for (const line of source.split('pnpm --dir web install')[0].split('\n')) {
    if (!line.startsWith('COPY ')) continue;
    const entries = line.slice(5).trim().split(/\s+/u);
    const destination = entries.pop();
    for (const entry of entries) {
      const target = path.join(root, destination, destination.endsWith('/') ? path.basename(entry) : '');
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.cpSync(path.join(projectRoot, entry), target, { recursive: true });
    }
  }
  const manifest = JSON.parse(fs.readFileSync(path.join(root, 'web/package.json'), 'utf8'));
  const result = require('node:child_process').spawnSync('sh', ['-c', manifest.scripts.preinstall], {
    cwd: path.join(root, 'web'), encoding: 'utf8',
  });
  assert.equal(result.status, 0, result.stderr);
});

test('AC-WEB-2 legacy Dockerfiles omit CI named build contexts while retaining source builds', (t) => {
  const { dockerfiles, tempDir } = createLegacyDockerfiles(projectRoot);
  t.after(() => fs.rmSync(tempDir, { recursive: true, force: true }));
  for (const filename of Object.values(dockerfiles)) {
    const source = fs.readFileSync(filename, 'utf8');
    assert.doesNotMatch(source, /runtime-prebuilt|--from=web_dist|--from=api_server_binaries/u);
    assert.match(source, /FROM runtime-base AS runtime/u);
    assert.match(source, /COPY --from=builder/u);
  }
});
