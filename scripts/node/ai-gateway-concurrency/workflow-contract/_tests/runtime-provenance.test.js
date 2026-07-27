'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { PROVIDER_CODES, provenanceRow } = require('../runtime-provenance');

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
