'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { parseArgs, prepareManifest } = require('../prepare-manifest');

function digest(value) {
  return crypto.createHash('sha256').update(value).digest('hex');
}

test('Root #1477 seals current Gateway binary digests without changing source manifest', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'sealed-local-acceptance-'));
  try {
    const apiServer = path.join(root, 'api/target/release/api-server');
    const pluginRunner = path.join(root, 'api/target/release/plugin-runner');
    fs.mkdirSync(path.dirname(apiServer), { recursive: true });
    fs.writeFileSync(apiServer, 'api-candidate');
    fs.writeFileSync(pluginRunner, 'runner-candidate');
    const source = path.join(root, 'manifest.json');
    const sourceText = `${JSON.stringify({
      schema_version: '1flowbase.local-ai-gateway-acceptance/v1',
      repo: { host: { path: root, revision: 'HEAD' } },
      database: {
        container: 'docker-db-1', image: 'postgres:18-alpine', host: '127.0.0.1', port: 35432,
      },
      artifacts: {
        apiServer: { path: apiServer, sha256: '0'.repeat(64) },
        pluginRunner: { path: pluginRunner, sha256: '0'.repeat(64) },
      },
    }, null, 2)}\n`;
    fs.writeFileSync(source, sourceText);
    const output = path.join(root, 'tmp/test-governance/sealed.json');

    const sealed = prepareManifest({ source, output });

    assert.equal(sealed.artifacts.apiServer.sha256, digest('api-candidate'));
    assert.equal(sealed.artifacts.pluginRunner.sha256, digest('runner-candidate'));
    assert.equal(fs.readFileSync(source, 'utf8'), sourceText);
    assert.deepEqual(JSON.parse(fs.readFileSync(output, 'utf8')), sealed);
    assert.throws(
      () => prepareManifest({ source, output: path.join(root, 'outside.json') }),
      /tmp\/test-governance/u,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('Root #1477 prepare-manifest CLI accepts one source and one output', () => {
  assert.deepEqual(parseArgs(['--source', '/source.json', '--output', '/sealed.json']), {
    source: '/source.json', output: '/sealed.json',
  });
  assert.throws(() => parseArgs([]), /usage/u);
});
