'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { PROVENANCE_FIDELITY_ORACLE, assertProvenanceFidelity } = require('../provenance');

test('Root #1477 AC-010: source, package, installed receipt, and runtime identity are one exact chain', () => {
  const source = 'a'.repeat(40);
  const digest = 'b'.repeat(64);
  const row = {
    built_package_sha256: digest,
    ready_package_sha256: digest,
    installed_receipt_sha256: digest,
    runtime_loaded_package_sha256: digest,
  };
  const evidence = {
    official_source_sha: source,
    paired_lock_revision: source,
    providers: {
      openai: { ...row },
      anthropic: { ...row },
      openai_compatible: { ...row },
    },
  };
  assert.deepEqual(PROVENANCE_FIDELITY_ORACLE.providers, [
    'openai', 'anthropic', 'openai_compatible',
  ]);
  assert.equal(assertProvenanceFidelity(evidence).verdict, 'PASS');
  evidence.providers.openai.runtime_loaded_package_sha256 = 'c'.repeat(64);
  assert.throws(() => assertProvenanceFidelity(evidence), /provenance diverged/u);
});
