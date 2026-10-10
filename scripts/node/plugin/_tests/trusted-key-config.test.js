const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

const root = path.resolve(__dirname, '../../../..');
const keyId = '1flowbase-signing-20261010-e7a2c47a';
const publicBody = 'MCowBQYDK2VwAyEAOMjRrNChRtu4aR+yVpFUwvOVxonoNizhWuQ4+DwJjrg=';

test('host default trusts the rotated Ed25519 key under its new identity', () => {
  const source = fs.readFileSync(path.join(root, 'api/apps/api-server/src/config.rs'), 'utf8');
  const body = source.match(/fn default_official_plugin_trusted_public_keys_json\(\) -> String \{\s*r#"(.*?)"#/s);
  assert.ok(body, 'host default must be inspectable');
  const keys = JSON.parse(body[1]);
  assert.deepEqual(keys.map(key => key.key_id), [keyId]);
  assert.equal(keys[0].algorithm, 'ed25519');
  const key = crypto.createPublicKey(keys[0].public_key_pem);
  assert.equal(key.asymmetricKeyType, 'ed25519');
  assert.equal(key.export({ type: 'spki', format: 'der' }).toString('base64'), publicBody);
});

test('shipped environment and Compose defaults agree with the rotated host trust', () => {
  for (const relative of [
    'api/apps/api-server/.env.example',
    'api/apps/api-server/.env.production.example',
    'docker/.env.example',
    'docker/docker-compose.yaml',
    'docker/docker-compose.external-db.yaml',
    'docker/deployment/compose.yaml',
  ]) {
    const source = fs.readFileSync(path.join(root, relative), 'utf8');
    assert.ok(source.includes(`"key_id":"${keyId}"`), relative);
    assert.ok(source.includes(publicBody), relative);
    assert.ok(!source.includes('official-key-2026-04'), relative);
  }
});
