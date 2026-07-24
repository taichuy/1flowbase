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
  const controlled = manifest.controlled_upstream;
  const controlledUpstream = controlled ? {
    snapshotUrl: loopbackUrl(controlled.snapshot_url, 'controlled upstream snapshot URL').href,
    barrierReleaseUrl: loopbackUrl(controlled.barrier_release_url, 'controlled upstream barrier URL').href,
    networkObserverUrl: loopbackUrl(controlled.network_observer_url, 'network observer URL').href,
    gatewayExecutorObserverUrl: loopbackUrl(
      controlled.gateway_executor_observer_url, 'gateway executor observer URL'
    ).href,
  } : null;
  return { path: resolved, gatewayBaseUrl: gateway.origin, openai, anthropic, controlledUpstream };
}

function normalizeInputs(options) {
  const requiredText = (value, label) => {
    if (typeof value !== 'string' || value.trim() === '') fail(`${label} is required`);
    return value;
  };
  const sourceRoot = (value, label) => {
    const resolved = path.resolve(requiredText(value, label));
    if (!fs.existsSync(resolved) || !fs.statSync(resolved).isDirectory()) fail(`${label} must be a directory`);
    return resolved;
  };
  const manifest = readReadyManifest(options.readyManifest);
  if (options.tmuxTiming && !manifest.controlledUpstream) {
    fail('tmux client tool chronology requires controlled upstream observation URLs');
  }
  return {
    manifest,
    codexExecutable: requireFile(options.codexExecutable, 'codex executable', true),
    claudeExecutable: requireFile(options.claudeExecutable, 'claude executable', true),
    opencodeExecutable: options.opencodeExecutable
      ? requireFile(options.opencodeExecutable, 'opencode executable', true)
      : null,
    provenance: {
      codex: {
        sourceRoot: sourceRoot(options.codexSourceRoot, 'codex source root'),
        sourceIdentity: requiredText(options.codexSourceIdentity, 'codex source identity'),
        buildCommand: requiredText(options.codexBuildCommand, 'codex build command'),
      },
      claude: {
        packageManifest: requireFile(options.claudePackageManifest, 'claude package manifest'),
        packageName: requiredText(options.claudePackageName, 'claude package name'),
        packageVersion: requiredText(options.claudePackageVersion, 'claude package version'),
        packageIntegrity: requiredText(options.claudePackageIntegrity, 'claude package integrity'),
        installCommand: requiredText(options.claudeInstallCommand, 'claude install command'),
      },
      ...(options.opencodeExecutable ? {
        opencode: {
          sourceRoot: sourceRoot(options.opencodeSourceRoot, 'opencode source root'),
          sourceIdentity: requiredText(options.opencodeSourceIdentity, 'opencode source identity'),
          buildCommand: requiredText(options.opencodeBuildCommand, 'opencode build command'),
        },
      } : {}),
    },
  };
}

module.exports = { CliSmokeError, loopbackUrl, normalizeInputs, readReadyManifest };
