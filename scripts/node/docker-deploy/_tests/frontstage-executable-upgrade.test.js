const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');

test('frontstage upgrade precedes every runtime switch and failure is terminal', () => {
  const shell = read('scripts/shell/docker-deploy.sh');
  const powerShell = read('scripts/powershell/docker-deploy.ps1');

  for (const source of [shell, powerShell]) {
    const internalUpgrade = source.indexOf('frontstage-executable-upgrade');
    assert.notEqual(internalUpgrade, -1);
    if (source === powerShell) {
      const externalUpgrade = source.indexOf(
        'docker-compose.external-db.yaml", "run"'
      );
      const runtimeSwitch = source.lastIndexOf('up", "-d"');
      assert.ok(externalUpgrade > 0);
      assert.match(source, /upgrade failed; the new runtime was not started/u);
      assert.ok(runtimeSwitch > internalUpgrade);
      assert.ok(runtimeSwitch > externalUpgrade);
    } else {
      assert.match(source, /^set -eu$/mu);
      assert.ok(source.lastIndexOf('compose up -d') > internalUpgrade);
    }
  }
});

test('runtime image locks Node 24 and compiler artifact identity without runtime install', () => {
  const dockerfile = read('docker/api-server.Dockerfile');
  assert.match(dockerfile, /FROM node:24-bookworm-slim AS runtime-base/u);
  assert.match(
    dockerfile,
    /603eb3ed18b81b7de3ce3f0e1f6f599dc1c6d58e246b6f567bad59e2a4d0a704/u
  );
  assert.match(
    dockerfile,
    /db8e4ecacf25ed2a926cbd5e8dfb4d5abeaf9db6bfe7025cd5a8fdaabed7efaf/u
  );
  const runtimeBase = dockerfile.slice(dockerfile.indexOf('AS runtime-base'));
  assert.doesNotMatch(runtimeBase, /pnpm install|npm install/u);
});

test('runtime image reuses the Node base principal for the flowbase user', () => {
  const dockerfile = read('docker/api-server.Dockerfile');
  const runtimeBase = dockerfile.slice(dockerfile.indexOf('AS runtime-base'));

  assert.match(
    runtimeBase,
    /groupmod --gid "\$\{APP_GID\}" --new-name flowbase node/u
  );
  assert.match(
    runtimeBase,
    /usermod --uid "\$\{APP_UID\}" --gid "\$\{APP_GID\}" --login flowbase/u
  );
  assert.doesNotMatch(runtimeBase, /groupadd|useradd/u);
});

test('compose upgrade service is isolated from unrelated runtime services', () => {
  for (const relativePath of [
    'docker/docker-compose.yaml',
    'docker/docker-compose.external-db.yaml',
    'docker/docker-compose.dev.yaml'
  ]) {
    const compose = read(relativePath);
    assert.match(compose, /frontstage-executable-upgrade:/u);
    assert.match(compose, /profiles: \["upgrade"\]/u);
    assert.match(
      compose,
      /entrypoint: \["\/usr\/local\/bin\/frontstage_executable_upgrade"\]/u
    );
  }
});

test('api startup requires exact cutover before bootstrap and runtime assembly', () => {
  const api = read('api/apps/api-server/src/lib.rs');
  const cutover = api.indexOf('frontstage_executable_upgrade::require_cutover');
  const bootstrap = api.indexOf('BootstrapService::new', cutover);
  const runtime = api.indexOf('ApiRuntimeServices::new', cutover);
  assert.ok(cutover > 0);
  assert.ok(bootstrap > cutover);
  assert.ok(runtime > bootstrap);
});
