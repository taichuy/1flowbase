'use strict';

const PROVIDERS = Object.freeze(['openai', 'anthropic']);
const SHA256_PATTERN = /^[a-f0-9]{64}$/u;
const SOURCE_SHA_PATTERN = /^[a-f0-9]{40}$/u;

const PROVENANCE_FIDELITY_ORACLE = Object.freeze({
  schema_version: '1flowbase.ai-gateway-provenance-oracle/v1',
  providers: PROVIDERS,
  source: 'official_source_sha-equals-paired_lock_revision',
  artifact: 'built_package_sha256-equals-ready_package_sha256',
  installed: 'ready_package_sha256-equals-installed_receipt_sha256',
  runtime: 'installed_receipt_sha256-equals-runtime_loaded_package_sha256',
});

function assertDigest(value, label, pattern) {
  if (!pattern.test(value ?? '')) throw new Error(`${label} is not a full lowercase digest`);
}

function assertProvenanceFidelity(evidence) {
  assertDigest(evidence.official_source_sha, 'official source SHA', SOURCE_SHA_PATTERN);
  if (evidence.official_source_sha !== evidence.paired_lock_revision) {
    throw new Error('official source SHA did not match paired lock revision');
  }
  for (const provider of PROVIDERS) {
    const row = evidence.providers?.[provider];
    if (!row) throw new Error(`provenance evidence omitted ${provider}`);
    for (const [field, value] of Object.entries({
      built_package_sha256: row.built_package_sha256,
      ready_package_sha256: row.ready_package_sha256,
      installed_receipt_sha256: row.installed_receipt_sha256,
      runtime_loaded_package_sha256: row.runtime_loaded_package_sha256,
    })) assertDigest(value, `${provider} ${field}`, SHA256_PATTERN);
    if (new Set([
      row.built_package_sha256,
      row.ready_package_sha256,
      row.installed_receipt_sha256,
      row.runtime_loaded_package_sha256,
    ]).size !== 1) throw new Error(`${provider} package/install/runtime provenance diverged`);
  }
  return {
    schema_version: '1flowbase.ai-gateway-provenance-result/v1',
    verdict: 'PASS', providers: PROVIDERS.length,
  };
}

module.exports = { PROVENANCE_FIDELITY_ORACLE, assertProvenanceFidelity };
