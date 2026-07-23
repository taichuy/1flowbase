'use strict';

const fs = require('node:fs');
const path = require('node:path');

class CliSmokeError extends Error {
  constructor(message) {
    super(message);
    this.name = 'CliSmokeError';
  }
}

function fail(message) {
  throw new CliSmokeError(message);
}

function requireFile(value, label, executable = false) {
  if (typeof value !== 'string' || value.trim() === '') fail(`${label} is required`);
  const resolved = path.resolve(value);
  let stat;
  try {
    stat = fs.statSync(resolved);
  } catch {
    fail(`${label} does not exist`);
  }
  if (!stat.isFile()) fail(`${label} must be a file`);
  if (executable) {
    try {
      fs.accessSync(resolved, fs.constants.X_OK);
    } catch {
      fail(`${label} must be executable`);
    }
  }
  return resolved;
}

function loopbackUrl(value, label) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail(`${label} is invalid`);
  }
  if (
    parsed.protocol !== 'http:'
    || !['127.0.0.1', '::1', 'localhost'].includes(parsed.hostname.toLowerCase())
    || parsed.username
    || parsed.password
    || parsed.search
    || parsed.hash
  ) {
    fail(`${label} must be credential-free loopback HTTP`);
  }
  return parsed;
}

function providerTarget(manifest, code) {
  const target = manifest.targets?.[code];
  if (!target || typeof target !== 'object') fail(`ready manifest omitted ${code} target`);
  for (const field of ['application_id', 'model', 'api_key']) {
    if (typeof target[field] !== 'string' || target[field] === '') {
      fail(`ready manifest ${code} target omitted ${field}`);
    }
  }
  return target;
}

function readReadyManifest(filePath) {
  const resolved = requireFile(filePath, 'WP3 ready manifest');
  let manifest;
  try {
    manifest = JSON.parse(fs.readFileSync(resolved, 'utf8'));
  } catch {
    fail('WP3 ready manifest is not valid JSON');
  }
  if (manifest.schema_version !== '1flowbase.ai-gateway-fixture/v1') {
    fail('WP3 ready manifest schema mismatch');
  }
  const gateway = loopbackUrl(manifest.gateway_base_url, 'gateway base URL');
  if (gateway.pathname !== '/') fail('gateway base URL must not include a path');
  const openai = providerTarget(manifest, 'openai');
  const anthropic = providerTarget(manifest, 'anthropic');
  for (const [code, target] of Object.entries({ openai, anthropic })) {
    const targetGateway = loopbackUrl(target.gateway?.base_url, `${code} gateway base URL`);
    if (targetGateway.origin !== gateway.origin) fail(`${code} target gateway origin mismatch`);
  }
  return { path: resolved, gatewayBaseUrl: gateway.origin, openai, anthropic };
}

function normalizeInputs(options) {
  const barrierReleaseUrl = options.barrierReleaseUrl
    ? loopbackUrl(options.barrierReleaseUrl, 'barrier release URL').href
    : null;
  return {
    manifest: readReadyManifest(options.readyManifest),
    codexExecutable: requireFile(options.codexExecutable, 'codex executable', true),
    claudeExecutable: requireFile(options.claudeExecutable, 'claude executable', true),
    opencodeExecutable: options.opencodeExecutable
      ? requireFile(options.opencodeExecutable, 'opencode executable', true)
      : null,
    barrierReleaseUrl,
  };
}

module.exports = { CliSmokeError, loopbackUrl, normalizeInputs, readReadyManifest };
