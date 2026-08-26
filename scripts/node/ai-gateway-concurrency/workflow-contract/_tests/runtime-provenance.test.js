'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  PROVIDER_CODES,
  provenanceRow,
} = require('../runtime-provenance');

test('Root #1477 AC-010: validated package receipts bind all three official packages', () => {
  assert.deepEqual(PROVIDER_CODES, ['openai', 'anthropic', 'openai_compatible']);
  const digest = 'a'.repeat(64);
  const fingerprint = `sha256:${'b'.repeat(64)}`;
  assert.deepEqual(provenanceRow({
    packageSha256: digest,
    manifestFingerprint: fingerprint,
  }, 'openai'), {
    built_package_sha256: digest,
    ready_package_sha256: digest,
    validated_package_sha256: digest,
    provider_code: 'openai',
    manifest_fingerprint: fingerprint,
  });
});
