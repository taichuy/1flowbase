'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { OwnerHttpClient } = require('../gateway-fixture/http-owner');
const { pinnedClaudeProvenance } = require('../cli-smoke/provenance');
const { buildClientPlan } = require('../local-client-acceptance/contract');
const { clientPaths, writeConfigs } = require('../local-client-acceptance/driver');
const { OwnedResources, executeTmux } = require('../local-client-acceptance/lifecycle');
const { clientSurface } = require('../local-client-acceptance/client-surface');
const { redact } = require('../local-client-acceptance/artifacts');
const {
  buildCountTokensUpgradeEvidence,
  loadCountTokensUpgradeFixture,
} = require('./count-tokens-upgrade');

const APPLICATION_ID = '019f5443-5b8e-74b2-90e3-c867dbddd37b';
const RUN_SCHEMA = '1flowbase.local-count-tokens-upgrade-run/v1';
const FORBIDDEN_PORTS = new Set([3100, 7800, 7801]);

class UnavailableError extends Error {
  constructor(message) {
    super(message);
    this.name = 'UnavailableError';
    this.code = 'configuration_unavailable';
  }
}

function requireValue(value, label) {
  if (typeof value !== 'string' || !value.trim()) throw new UnavailableError(`${label} is unavailable`);
  return value.trim();
}

function requireFile(value, label, executable = false) {
  const resolved = path.resolve(requireValue(value, label));
  let stat;
  try { stat = fs.statSync(resolved); } catch { throw new UnavailableError(`${label} is unavailable`); }
  if (!stat.isFile()) throw new UnavailableError(`${label} is unavailable`);
  if (executable) {
    try { fs.accessSync(resolved, fs.constants.X_OK); }
    catch { throw new UnavailableError(`${label} is unavailable`); }
  }
  return resolved;
}

function envName(value, label) {
  const name = requireValue(value, label);
  if (!/^[A-Z][A-Z0-9_]*$/u.test(name)) throw new Error(`${label} must be a safe environment name`);
  return name;
}

function safeClaim(value, label) {
  const claim = requireValue(value, label);
  if (/api[_-]?key|token|secret|credential|authorization|cookie/iu.test(claim)
    || /:\/\/[^/\s]+:[^/@\s]+@/u.test(claim)) {
    throw new Error(`${label} must not contain credentials`);
  }
  return claim;
}

function isolatedBaseUrl(value, label) {
  let url;
  try { url = new URL(value); } catch { throw new UnavailableError(`${label} is unavailable`); }
  const port = Number(url.port || 80);
  if (url.protocol !== 'http:' || !['127.0.0.1', 'localhost', '::1', '[::1]'].includes(url.hostname)
    || url.username || url.password || url.pathname !== '/' || url.search || url.hash) {
    throw new Error(`${label} must be a credential-free loopback HTTP origin`);
  }
  if (FORBIDDEN_PORTS.has(port)) throw new Error(`${label} must not use protected port ${port}`);
  return url.origin;
}

function loadRunManifest(filePath) {
  let value;
  try { value = JSON.parse(fs.readFileSync(path.resolve(filePath), 'utf8')); }
  catch (error) { throw new UnavailableError(`CountTokens upgrade run manifest is unavailable: ${error.message}`); }
  if (value.schema_version !== RUN_SCHEMA) throw new Error('CountTokens upgrade run manifest schema mismatch');
  if (value.application_id !== APPLICATION_ID) throw new Error(`CountTokens upgrade application must be ${APPLICATION_ID}`);
  const consoleBaseUrl = isolatedBaseUrl(value.endpoints?.console_base_url, 'console endpoint');
  const gatewayBaseUrl = isolatedBaseUrl(value.endpoints?.gateway_base_url, 'gateway endpoint');
  if (consoleBaseUrl === gatewayBaseUrl) throw new Error('console and gateway endpoints must be isolated origins');
  const artifact = path.resolve(requireValue(value.artifact, 'artifact path'));
  if (!artifact.includes(`${path.sep}tmp${path.sep}test-governance${path.sep}`)) {
    throw new Error('CountTokens upgrade artifact must be under tmp/test-governance');
  }
  return {
    applicationId: APPLICATION_ID,
    consoleBaseUrl,
    gatewayBaseUrl,
    model: requireValue(value.model, 'published model'),
    afterPackage: requireFile(value.upgrade?.after_package, 'local DeepSeek upgrade package'),
    claudeExecutable: requireFile(value.claude?.executable, 'Claude executable', true),
    claude: {
      packageManifest: requireFile(value.claude?.provenance?.package_manifest, 'Claude package manifest'),
      packageName: requireValue(value.claude?.provenance?.package_name, 'Claude package name'),
      packageVersion: requireValue(value.claude?.provenance?.package_version, 'Claude package version'),
      packageIntegrity: requireValue(value.claude?.provenance?.package_integrity, 'Claude package integrity'),
      installCommand: safeClaim(value.claude?.provenance?.install_command, 'Claude install command claim'),
    },
    env: {
      apiKey: envName(value.environment?.application_api_key, 'application API key env name'),
      apiKeyId: envName(value.environment?.application_api_key_id, 'application API key id env name'),
      ownerCookie: envName(value.environment?.owner_cookie, 'owner cookie env name'),
      ownerCsrf: envName(value.environment?.owner_csrf, 'owner CSRF env name'),
    },
    artifact,
  };
}

function envValue(sourceEnv, name, label) {
  return requireValue(sourceEnv?.[name], `${label} (${name})`);
}

function publicError(error) {
  return {
    name: error?.name || 'Error',
    code: error?.code || 'execution_failed',
    message: error?.message || String(error),
  };
}

function sha256File(filePath) {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function checksum(value) {
  return typeof value === 'string' ? value.replace(/^sha256:/u, '') : null;
}

function familyFromResponse(data) {
  const family = data?.entries?.find((entry) => entry.provider_code === 'deepseek');
  if (!family) throw new Error('DeepSeek plugin family is unavailable');
  const localChecksum = checksum(family.current_local_artifact?.local_checksum);
  if (!family.current_installation_id || !family.current_version || !localChecksum) {
    throw new Error('DeepSeek current installation provenance is incomplete');
  }
  return {
    installation_id: family.current_installation_id,
    version: family.current_version,
    package_sha256: localChecksum,
  };
}

async function readDeepSeekFamily(owner) {
  const { data } = await owner.read('/api/console/settings/model-providers/plugins/families');
  return familyFromResponse(data);
}

async function activePublication(owner, applicationId) {
  const { data } = await owner.read(`/api/console/applications/${applicationId}/api-publication`);
  if (data?.application_id !== applicationId || typeof data.id !== 'string' || !data.active || !data.api_enabled) {
    throw new Error('active application publication is unavailable');
  }
  return data.id;
}

async function assertTokenBinding(owner, applicationId, apiKeyId, apiKey) {
  const { data } = await owner.read(`/api/console/applications/${applicationId}/api-keys`);
  if (!Array.isArray(data) || !data.some((key) => key.id === apiKeyId
    && key.enabled === true
    && typeof key.token_prefix === 'string'
    && apiKey.startsWith(key.token_prefix))) {
    throw new Error(`application API key is not enabled for ${applicationId}`);
  }
}

async function countTokens(gatewayBaseUrl, apiKey, model, fetchImpl) {
  const response = await fetchImpl(`${gatewayBaseUrl}/v1/messages/count_tokens`, {
    method: 'POST',
    headers: {
      accept: 'application/json',
      'content-type': 'application/json',
      'x-api-key': apiKey,
      'anthropic-version': '2023-06-01',
    },
    body: JSON.stringify({
      model,
      messages: [{ role: 'user', content: 'Count this local acceptance prompt.' }],
    }),
    signal: AbortSignal.timeout(30_000),
  });
  const raw = await response.text();
  let data;
  try { data = raw ? JSON.parse(raw) : null; }
  catch { throw new Error(`CountTokens returned invalid JSON (${response.status})`); }
  if (!response.ok) throw new Error(`CountTokens failed (${response.status})`);
  if (!Number.isSafeInteger(data?.input_tokens) || data.input_tokens < 0) {
    throw new Error('CountTokens response omitted input_tokens');
  }
  return { input_tokens: data.input_tokens };
}

function conversationVector() {
  return Object.freeze({
    id: 'count-tokens-upgrade-conversation',
    kind: 'conversation',
    turns: Object.freeze([
      Object.freeze({ prompt: 'Reply briefly to confirm the initial DeepSeek conversation turn.' }),
      Object.freeze({ prompt: 'Continue this same conversation after the local plugin upgrade.' }),
    ]),
  });
}

function conversationSummary(result) {
  const surface = clientSurface('claude', result, 'success');
  if (result.exit_code !== 0 || result.timed_out || !surface.terminal.observed || surface.assistantTexts.length === 0) {
    throw new Error('Claude conversation turn did not complete');
  }
  const text = surface.assistantTexts.map((entry) => entry.text).join('');
  return {
    status: 'pass',
    exit_code: result.exit_code,
    assistant_sha256: crypto.createHash('sha256').update(text).digest('hex'),
    assistant_utf8_bytes: Buffer.byteLength(text),
  };
}

async function runClaudeTurn({ manifest, apiKey, paths, registry, vector, turnIndex, sessionId, sourceEnv, dependencies }) {
  const plan = buildClientPlan('claude', manifest.claudeExecutable, {
    provider: 'deepseek', model: manifest.model, apiKey, gatewayBaseUrl: manifest.gatewayBaseUrl,
  }, paths, vector, 'anthropic_sse', { turnIndex, sessionId });
  writeConfigs(plan);
  const result = await (dependencies.executeTmux || executeTmux)(plan, {
    registry,
    parentEnv: sourceEnv,
    tmuxExecutable: dependencies.tmuxExecutable || 'tmux',
  });
  return conversationSummary(result);
}

async function installLocalUpgrade(owner, archivePath) {
  const uploaded = await owner.uploadPackage(archivePath);
  const installation = uploaded.installation;
  if (installation?.provider_code !== 'deepseek' || typeof installation.id !== 'string') {
    throw new Error('local upgrade package is not a DeepSeek installation');
  }
  await owner.write(`/api/console/plugins/${installation.id}/enable`);
  await owner.write(`/api/console/plugins/${installation.id}/assign`);
  return {
    installation_id: installation.id,
    version: installation.plugin_version,
    package_sha256: uploaded.archive_sha256,
  };
}

function writeRunArtifact(filePath, value, secrets) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true, mode: 0o700 });
  fs.writeFileSync(filePath, `${JSON.stringify(redact(value, secrets), null, 2)}\n`, { mode: 0o600 });
}

async function runCountTokensUpgrade(rawOptions, dependencies = {}) {
  const sourceEnv = dependencies.sourceEnv || process.env;
  const registry = dependencies.registry || new OwnedResources(dependencies);
  const fetchImpl = dependencies.fetchImpl || globalThis.fetch;
  let manifest = null;
  let owner = null;
  let primaryError = null;
  let observed = null;
  let cleanupErrors = [];
  let secrets = [];
  let artifactPath = null;
  try {
    manifest = loadRunManifest(rawOptions.manifest);
    artifactPath = manifest.artifact;
    const apiKey = envValue(sourceEnv, manifest.env.apiKey, 'application API key');
    const apiKeyId = envValue(sourceEnv, manifest.env.apiKeyId, 'application API key id');
    const ownerCookie = envValue(sourceEnv, manifest.env.ownerCookie, 'owner cookie');
    const ownerCsrf = envValue(sourceEnv, manifest.env.ownerCsrf, 'owner CSRF token');
    secrets = [apiKey, ownerCookie, ownerCsrf];
    owner = new (dependencies.OwnerHttpClient || OwnerHttpClient)(manifest.consoleBaseUrl, fetchImpl);
    owner.attachSession(ownerCookie, ownerCsrf);
    await assertTokenBinding(owner, manifest.applicationId, apiKeyId, apiKey);
    const publicationId = await activePublication(owner, manifest.applicationId);
    const beforePlugin = await readDeepSeekFamily(owner);
    const tokenResult = await countTokens(manifest.gatewayBaseUrl, apiKey, manifest.model, fetchImpl);
    const tempRoot = registry.addTempRoot(fs.mkdtempSync(path.join(os.tmpdir(), '1flowbase-count-tokens-upgrade-')));
    const paths = clientPaths(tempRoot, 'claude-deepseek-upgrade', null);
    const vector = conversationVector();
    const sessionId = crypto.randomUUID();
    const initial = await runClaudeTurn({
      manifest, apiKey, paths, registry, vector, turnIndex: 0, sessionId, sourceEnv, dependencies,
    });
    const uploaded = await installLocalUpgrade(owner, manifest.afterPackage);
    const afterPlugin = await readDeepSeekFamily(owner);
    if (beforePlugin.version === afterPlugin.version
      || checksum(beforePlugin.package_sha256) === checksum(afterPlugin.package_sha256)) {
      throw new Error('local DeepSeek package did not upgrade version and checksum');
    }
    if (afterPlugin.installation_id !== uploaded.installation_id
      || afterPlugin.version !== uploaded.version
      || checksum(afterPlugin.package_sha256) !== checksum(uploaded.package_sha256)
      || checksum(uploaded.package_sha256) !== sha256File(manifest.afterPackage)) {
      throw new Error('installed DeepSeek upgrade provenance does not match the uploaded package');
    }
    const afterPublicationId = await activePublication(owner, manifest.applicationId);
    const followup = await runClaudeTurn({
      manifest, apiKey, paths, registry, vector, turnIndex: 1, sessionId, sourceEnv, dependencies,
    });
    observed = {
      application_id: manifest.applicationId,
      after_upgrade_application_id: manifest.applicationId,
      publication_id: publicationId,
      after_upgrade_publication_id: afterPublicationId,
      before_plugin: beforePlugin,
      after_plugin: afterPlugin,
      republish_events: publicationId === afterPublicationId ? 0 : 1,
      network_installs: 0,
      count_tokens_application_id: manifest.applicationId,
      count_tokens: tokenResult,
      claude: {
        application_id: manifest.applicationId,
        surface: 'tmux',
        turns: 2,
        continued_session: true,
        initial,
        followup,
        provenance: pinnedClaudeProvenance(manifest.claudeExecutable, manifest.claude),
      },
    };
  } catch (error) {
    primaryError = error;
  } finally {
    try { cleanupErrors = await registry.close(); }
    catch (error) { cleanupErrors = [{ owner: 'owned-resources', message: error.message }]; }
  }

  const cleanup = { status: cleanupErrors.length ? 'fail' : 'pass', errors: cleanupErrors };
  if (observed) observed.cleanup = cleanup;
  let evidence = null;
  if (!primaryError && cleanup.status === 'pass') {
    try { evidence = buildCountTokensUpgradeEvidence(loadCountTokensUpgradeFixture(), observed); }
    catch (error) { primaryError = error; }
  }
  const result = {
    schema_version: RUN_SCHEMA,
    status: primaryError || cleanupErrors.length ? 'fail' : 'pass',
    availability: primaryError instanceof UnavailableError ? 'unavailable' : 'available',
    application_id: APPLICATION_ID,
    evidence,
    primary_error: primaryError ? publicError(primaryError) : null,
    cleanup,
  };
  const safeResult = redact(result, secrets);
  if (artifactPath) writeRunArtifact(artifactPath, safeResult, secrets);
  return safeResult;
}

module.exports = {
  APPLICATION_ID,
  RUN_SCHEMA,
  UnavailableError,
  isolatedBaseUrl,
  loadRunManifest,
  runCountTokensUpgrade,
};
