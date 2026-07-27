'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  PROVIDER_CODES,
  assertRuntimeLoad,
  provenanceRow,
} = require('../runtime-provenance');

test('Root #1477 AC-010: runtime receipt binds all three official packages', () => {
  assert.deepEqual(PROVIDER_CODES, ['openai', 'anthropic', 'openai_compatible']);
  const digest = 'a'.repeat(64);
  const fingerprint = `sha256:${'b'.repeat(64)}`;
  assert.deepEqual(provenanceRow({
    packageSha256: digest,
    manifestFingerprint: fingerprint,
  }, {
    plugin_id: 'openai@1.0.0',
    plugin_version: '1.0.0',
    verified_artifact_receipt: {
      package_sha256: `sha256:${digest}`,
      manifest_fingerprint: fingerprint,
    },
  }), {
    built_package_sha256: digest,
    ready_package_sha256: digest,
    installed_receipt_sha256: digest,
    runtime_loaded_package_sha256: digest,
    plugin_id: 'openai@1.0.0',
    plugin_version: '1.0.0',
    manifest_fingerprint: fingerprint,
  });
});

test('Root #1477 AC-010: runtime load failures preserve the actual ownership boundary', () => {
  assert.throws(() => assertRuntimeLoad('openai', {
    response: { ok: false, status: 400 },
    body: {
      message: 'invalid provider package: unsupported runtime capability protocol_context',
    },
  }), {
    message: [
      'openai runtime load failed after artifact receipt verification (HTTP 400):',
      'invalid provider package: unsupported runtime capability protocol_context',
    ].join(' '),
  });

  assert.throws(() => assertRuntimeLoad('openai', {
    response: { ok: true, status: 200 },
    body: { provider_code: 'anthropic' },
  }), {
    message: 'openai runtime loaded mismatched provider identity: anthropic',
  });
});
