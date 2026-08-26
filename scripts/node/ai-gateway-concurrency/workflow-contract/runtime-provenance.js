'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { assertProvenanceFidelity } = require('./provenance');

const PROVIDER_CODES = Object.freeze(['openai', 'anthropic', 'openai_compatible']);

function sha256File(filePath) {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function extractPackage(packagePath, destination) {
  fs.mkdirSync(destination, { recursive: true });
  const result = spawnSync('tar', ['-xzf', packagePath, '-C', destination], {
    encoding: 'utf8', maxBuffer: 64 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error('provider package extraction failed');
}

function packageReceipt(providerCode, packagePath, scratchRoot) {
  const packageRoot = path.join(scratchRoot, providerCode);
  extractPackage(packagePath, packageRoot);
  const manifestPath = path.join(packageRoot, 'manifest.yaml');
  if (!fs.statSync(manifestPath).isFile()) {
    throw new Error(`${providerCode} package omitted manifest.yaml`);
  }
  return {
    packagePath,
    packageRoot,
    packageSha256: sha256File(packagePath),
    manifestFingerprint: `sha256:${sha256File(manifestPath)}`,
  };
}

function provenanceRow(receipt, providerCode) {
  return {
    built_package_sha256: receipt.packageSha256,
    ready_package_sha256: receipt.packageSha256,
    validated_package_sha256: receipt.packageSha256,
    provider_code: providerCode,
    manifest_fingerprint: receipt.manifestFingerprint,
  };
}

async function verifyRuntimeProvenance(options) {
  const scratchRoot = fs.mkdtempSync(path.join(os.tmpdir(), '1flowbase-runtime-provenance-'));
  const receipts = Object.fromEntries(PROVIDER_CODES.map((providerCode) => [
    providerCode,
    packageReceipt(providerCode, options.packages[providerCode], scratchRoot),
  ]));
  try {
    const providers = Object.fromEntries(PROVIDER_CODES.map((providerCode) => [
      providerCode,
      provenanceRow(receipts[providerCode], providerCode),
    ]));
    const evidence = {
      schema_version: '1flowbase.ai-gateway-runtime-provenance/v1',
      official_source_sha: options.officialSourceSha,
      paired_lock_revision: options.pairedLockRevision,
      providers,
    };
    evidence.verdict = assertProvenanceFidelity(evidence).verdict;
    return evidence;
  } finally {
    fs.rmSync(scratchRoot, { recursive: true, force: true });
  }
}

module.exports = {
  PROVIDER_CODES,
  packageReceipt,
  provenanceRow,
  sha256File,
  verifyRuntimeProvenance,
};
