'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  FORBIDDEN_ACTION_WORDS,
  LOCAL_ACTIONS,
  loadManifest,
  resolveArtifactInventory,
  verifyChecksums,
} = require('../manifest');

function writeFixtureManifest(root) {
  const artifact = path.join(root, 'artifact');
  fs.writeFileSync(artifact, 'fixed-artifact');
  const manifestPath = path.join(root, 'manifest.json');
  fs.writeFileSync(manifestPath, JSON.stringify({
    schema_version: '1flowbase.local-ai-gateway-acceptance/v1',
    database: { container: 'docker-db-1', image: 'postgres:18-alpine', host: '127.0.0.1', port: 35432 },
    artifacts: {
      fixture: {
        path: artifact,
        sha256: 'cb62ed74e30a3b3936d51ed8b8f28f878e1ad69c67c3cc5797f72920e28b770b',
      },
    },
  }));
  return manifestPath;
}

test('AC-027/028: the local manifest fixes the existing container, port, paths, and checksums', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'local-acceptance-manifest-'));
  try {
    const manifest = loadManifest(writeFixtureManifest(root));
    assert.equal(manifest.database.container, 'docker-db-1');
    assert.equal(manifest.database.image, 'postgres:18-alpine');
    assert.equal(manifest.database.host, '127.0.0.1');
    assert.equal(manifest.database.port, 35432);
    assert.deepEqual(verifyChecksums(manifest).map((entry) => entry.name), ['fixture']);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('AC-028 controlled negative: checksum mismatch fails closed', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'local-acceptance-checksum-'));
  try {
    const manifest = loadManifest(writeFixtureManifest(root));
    fs.appendFileSync(manifest.artifacts.fixture.path, '-changed');
    assert.throws(() => verifyChecksums(manifest), /checksum mismatch/u);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('AC-028: generated Provider packages resolve from one filename-bound digest', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'local-acceptance-package-'));
  try {
    const content = 'current-provider-package';
    const digest = require('node:crypto').createHash('sha256').update(content).digest('hex');
    fs.writeFileSync(path.join(root, `provider@${digest}.1flowbasepkg`), content);
    const manifest = {
      artifacts: {
        provider: {
          directory: root,
          filename_pattern: '^provider@([a-f0-9]{64})\\.1flowbasepkg$',
        },
      },
    };
    const resolved = resolveArtifactInventory(manifest);
    assert.equal(resolved.artifacts.provider.sha256, digest);
    assert.deepEqual(verifyChecksums(resolved).map((entry) => entry.name), ['provider']);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('AC-028 controlled negative: generated Provider package discovery fails on ambiguity', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'local-acceptance-package-'));
  try {
    for (const digest of ['a'.repeat(64), 'b'.repeat(64)]) {
      fs.writeFileSync(path.join(root, `provider@${digest}.1flowbasepkg`), digest);
    }
    assert.throws(() => resolveArtifactInventory({
      artifacts: { provider: {
        directory: root,
        filename_pattern: '^provider@([a-f0-9]{64})\\.1flowbasepkg$',
      } },
    }), /exactly one verified package/u);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('AC-028: the executable action inventory contains no network, install, or build action', () => {
  const serialized = JSON.stringify(LOCAL_ACTIONS).toLowerCase();
  for (const word of FORBIDDEN_ACTION_WORDS) assert.equal(serialized.includes(word), false, word);
  assert.equal(serialized.includes('detached-worktree'), false);
  assert.equal(serialized.includes('run-eight-client-attempts'), true);
});

test('AC-028: the repository default binds all three existing clients and local source repositories', () => {
  const manifest = loadManifest();
  assert.equal(manifest.artifacts.codex.path, '/home/linuxbrew/.linuxbrew/bin/codex');
  assert.equal(manifest.artifacts.claude.path, '/home/taichuy/.nvm/versions/node/v24.18.0/bin/claude');
  assert.equal(manifest.artifacts.opencode.path, '/home/taichuy/.local/bin/opencode');
  assert.equal(manifest.sources.codex.repository, '/home/taichuy/git/codex');
  assert.equal(manifest.sources.opencode.repository, '/home/taichuy/git/opencode');
});
