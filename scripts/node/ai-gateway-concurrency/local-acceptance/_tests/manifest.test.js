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
  verifyChecksums,
} = require('../manifest');

function writeFixtureManifest(root) {
  const artifacts = {};
  for (const name of ['apiServer', 'pluginRunner', 'openaiPackage', 'anthropicPackage']) {
    const artifact = path.join(root, name);
    fs.writeFileSync(artifact, 'fixed-artifact');
    artifacts[name] = {
      path: artifact,
      sha256: 'cb62ed74e30a3b3936d51ed8b8f28f878e1ad69c67c3cc5797f72920e28b770b',
    };
  }
  const manifestPath = path.join(root, 'manifest.json');
  fs.writeFileSync(manifestPath, JSON.stringify({
    schema_version: '1flowbase.local-ai-gateway-acceptance/v1',
    database: { container: 'docker-db-1', image: 'postgres:16-alpine', host: '127.0.0.1', port: 35432 },
    artifacts,
  }));
  return manifestPath;
}

test('AC-027/028: the local manifest fixes the existing container, port, paths, and checksums', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'local-acceptance-manifest-'));
  try {
    const manifest = loadManifest(writeFixtureManifest(root));
    assert.equal(manifest.database.container, 'docker-db-1');
    assert.equal(manifest.database.image, 'postgres:16-alpine');
    assert.equal(manifest.database.host, '127.0.0.1');
    assert.equal(manifest.database.port, 35432);
    assert.deepEqual(verifyChecksums(manifest).map((entry) => entry.name), [
      'apiServer', 'pluginRunner', 'openaiPackage', 'anthropicPackage',
    ]);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('AC-028 controlled negative: checksum mismatch fails closed', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'local-acceptance-checksum-'));
  try {
    const manifest = loadManifest(writeFixtureManifest(root));
    fs.appendFileSync(manifest.artifacts.apiServer.path, '-changed');
    assert.throws(() => verifyChecksums(manifest), /checksum mismatch/u);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('AC-028: the executable action inventory contains no network, install, or build action', () => {
  const serialized = JSON.stringify(LOCAL_ACTIONS).toLowerCase();
  for (const word of FORBIDDEN_ACTION_WORDS) assert.equal(serialized.includes(word), false, word);
});

test('D7-AC-001/007: the repository default keeps host artifacts separate from the portable client lock', () => {
  const manifest = loadManifest();
  assert.deepEqual(Object.keys(manifest.artifacts), ['apiServer', 'pluginRunner', 'openaiPackage', 'anthropicPackage']);
  assert.equal(Object.hasOwn(manifest, 'sources'), false);
  assert.equal(Object.hasOwn(manifest, 'clients'), false);
});
