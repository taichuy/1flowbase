'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const {
  reserveLoopbackPort,
  spawnOwned,
  stopOwned,
  waitForHealth,
} = require('../gateway-fixture/process-owner');
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

async function requestLoad(baseUrl, receipt, manifestFingerprint) {
  const response = await fetch(`${baseUrl}/providers/load`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      package_root: receipt.packageRoot,
      artifact_receipt: {
        package_path: receipt.packagePath,
        expected_artifact_sha256: receipt.packageSha256,
        expected_manifest_fingerprint: manifestFingerprint,
      },
    }),
    signal: AbortSignal.timeout(30_000),
  });
  const body = await response.json();
  return { response, body };
}

function provenanceRow(receipt, loaded) {
  const verified = loaded.verified_artifact_receipt;
  if (verified?.manifest_fingerprint !== receipt.manifestFingerprint) {
    throw new Error('runtime-loaded manifest fingerprint diverged');
  }
  const runtimeDigest = verified?.package_sha256?.replace(/^sha256:/u, '');
  return {
    built_package_sha256: receipt.packageSha256,
    ready_package_sha256: receipt.packageSha256,
    installed_receipt_sha256: runtimeDigest,
    runtime_loaded_package_sha256: runtimeDigest,
    plugin_id: loaded.plugin_id,
    plugin_version: loaded.plugin_version,
    manifest_fingerprint: verified.manifest_fingerprint,
  };
}

async function verifyRuntimeProvenance(options) {
  const scratchRoot = fs.mkdtempSync(path.join(os.tmpdir(), '1flowbase-runtime-provenance-'));
  const receipts = Object.fromEntries(PROVIDER_CODES.map((providerCode) => [
    providerCode,
    packageReceipt(providerCode, options.packages[providerCode], scratchRoot),
  ]));
  let runner = null;
  try {
    const port = await reserveLoopbackPort();
    const baseUrl = `http://127.0.0.1:${port}`;
    runner = spawnOwned(options.pluginRunnerBin, {
      PLUGIN_RUNNER_ADDR: `127.0.0.1:${port}`,
      RUST_LOG: process.env.RUST_LOG || 'info',
    }, { cwd: scratchRoot });
    await waitForHealth(baseUrl, 'plugin-runner', { processHandle: runner });

    const stale = await requestLoad(
      baseUrl,
      receipts.openai,
      `sha256:${'0'.repeat(64)}`
    );
    if (stale.response.ok || !String(stale.body?.message).includes('manifest_fingerprint_mismatch')) {
      throw new Error('stale provider artifact receipt did not fail closed');
    }

    const providers = {};
    for (const providerCode of PROVIDER_CODES) {
      const loaded = await requestLoad(
        baseUrl,
        receipts[providerCode],
        receipts[providerCode].manifestFingerprint
      );
      if (!loaded.response.ok || loaded.body?.provider_code !== providerCode) {
        throw new Error(`${providerCode} runtime rejected the paired package receipt`);
      }
      providers[providerCode] = provenanceRow(receipts[providerCode], loaded.body);
    }
    const evidence = {
      schema_version: '1flowbase.ai-gateway-runtime-provenance/v1',
      official_source_sha: options.officialSourceSha,
      paired_lock_revision: options.pairedLockRevision,
      providers,
    };
    evidence.verdict = assertProvenanceFidelity(evidence).verdict;
    return evidence;
  } finally {
    await stopOwned(runner);
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
