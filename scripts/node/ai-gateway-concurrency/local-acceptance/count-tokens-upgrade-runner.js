'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const { OwnerHttpClient } = require('../gateway-fixture/http-owner');
const {
  reserveLoopbackPort,
  spawnOwned,
  stopOwned,
  waitForHealth,
} = require('../gateway-fixture/process-owner');
const { pinnedClaudeProvenance } = require('../cli-smoke/provenance');
const { buildClientPlan } = require('../local-client-acceptance/contract');
const { clientPaths, writeConfigs } = require('../local-client-acceptance/driver');
const { OwnedResources, executeTmux } = require('../local-client-acceptance/lifecycle');
const { clientSurface } = require('../local-client-acceptance/client-surface');
const { redact } = require('../local-client-acceptance/artifacts');
const { openTemporaryOwnerSession } = require('../../page-debug/auth');
const { parseEnvFile } = require('../../dev-up/env');
const {
  buildCountTokensUpgradeEvidence,
  loadCountTokensUpgradeFixture,
} = require('./count-tokens-upgrade');

const APPLICATION_ID = '019f5443-5b8e-74b2-90e3-c867dbddd37b';
const RUN_SCHEMA = '1flowbase.local-count-tokens-upgrade-run/v5';
const FORBIDDEN_PORTS = new Set([3100, 7800, 7801]);
const SHA_PATTERN = /^[0-9a-f]{40}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;

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

function requireDirectory(value, label) {
  const resolved = path.resolve(requireValue(value, label));
  try {
    if (fs.statSync(resolved).isDirectory()) return resolved;
  } catch {}
  throw new UnavailableError(`${label} is unavailable`);
}

function envName(value, label) {
  const name = requireValue(value, label);
  if (!/^[A-Z][A-Z0-9_]*$/u.test(name)) throw new Error(`${label} must be a safe environment name`);
  return name;
}

function optionalEnvName(value, label) {
  return value === undefined || value === null ? null : envName(value, label);
}

function safeClaim(value, label) {
  const claim = requireValue(value, label);
  if (/api[_-]?key|token|secret|credential|authorization|cookie/iu.test(claim)
    || /:\/\/[^/\s]+:[^/@\s]+@/u.test(claim)) {
    throw new Error(`${label} must not contain credentials`);
  }
  return claim;
}

function fullSha(value, label) {
  const sha = requireValue(value, label);
  if (!SHA_PATTERN.test(sha)) throw new Error(`${label} must be a full lowercase source SHA`);
  return sha;
}

function sha256(value, label) {
  const digest = requireValue(value, label).replace(/^sha256:/u, '');
  if (!SHA256_PATTERN.test(digest)) throw new Error(`${label} must be a lowercase SHA-256`);
  return digest;
}

function databaseUrlFromEnv(sourceEnv, name) {
  const value = envValue(sourceEnv, name, 'database URL');
  let url;
  try { url = new URL(value); } catch { throw new UnavailableError(`database URL (${name}) is unavailable`); }
  if (!['postgres:', 'postgresql:'].includes(url.protocol) || !url.hostname || !url.pathname.slice(1)) {
    throw new Error('database URL must name a PostgreSQL database');
  }
  return value;
}

function verifyMainSourceReceipt(receiptPath, expectedSha) {
  const receipt = fs.readFileSync(receiptPath, 'utf8').trim();
  if (receipt !== expectedSha) {
    throw new UnavailableError('gate main-source receipt does not match main_source_sha');
  }
}

function binaryReceipt(contract, label, mainSourceSha) {
  const filePath = requireFile(contract?.path, `${label} binary`, true);
  const expected = sha256(contract?.sha256, `${label} binary digest`);
  const actual = sha256File(filePath);
  if (actual !== expected) throw new UnavailableError(`${label} binary digest mismatch`);
  if (fullSha(contract?.source_sha, `${label} source SHA`) !== mainSourceSha) {
    throw new UnavailableError(`${label} binary source SHA mismatch`);
  }
  const stat = fs.statSync(filePath);
  return { path: filePath, sha256: actual, source_sha: mainSourceSha, bytes: stat.size };
}

function git(root, args) {
  return execFileSync('git', ['-C', root, ...args], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  }).trim();
}

function verifiedSourceCwd(value, mainSourceSha, dependencies = {}) {
  const sourceRoot = fs.realpathSync(requireDirectory(value.main_source_root, 'main source root'));
  const apiServerCwd = fs.realpathSync(requireDirectory(value.api_server_cwd, 'api-server cwd'));
  const expectedCwd = fs.realpathSync(requireDirectory(
    path.join(sourceRoot, 'api/apps/api-server'),
    'main source api/apps/api-server directory',
  ));
  if (apiServerCwd !== expectedCwd) {
    throw new UnavailableError('api-server cwd must be main_source_root/api/apps/api-server');
  }
  const runGit = dependencies.git || git;
  let topLevel;
  let head;
  try {
    topLevel = fs.realpathSync(runGit(sourceRoot, ['rev-parse', '--show-toplevel']));
    head = runGit(sourceRoot, ['rev-parse', 'HEAD']);
  } catch {
    throw new UnavailableError('main source Git provenance is unavailable');
  }
  if (topLevel !== sourceRoot || head !== mainSourceSha) {
    throw new UnavailableError('main source root HEAD does not match main_source_sha');
  }
  return { sourceRoot, apiServerCwd };
}

function loadRunManifest(filePath, dependencies = {}) {
  let value;
  try { value = JSON.parse(fs.readFileSync(path.resolve(filePath), 'utf8')); }
  catch (error) { throw new UnavailableError(`CountTokens upgrade run manifest is unavailable: ${error.message}`); }
  if (value.schema_version !== RUN_SCHEMA) throw new Error('CountTokens upgrade run manifest schema mismatch');
  if (value.application_id !== APPLICATION_ID) throw new Error(`CountTokens upgrade application must be ${APPLICATION_ID}`);
  const mainSourceSha = fullSha(value.main_source_sha, 'main source SHA');
  const mainSourceReceipt = requireFile(value.main_source_receipt, 'gate main-source receipt');
  verifyMainSourceReceipt(mainSourceReceipt, mainSourceSha);
  const source = verifiedSourceCwd(value, mainSourceSha, dependencies);
  const apiServer = binaryReceipt(value.api_server_binary, 'api-server', mainSourceSha);
  const pluginRunner = binaryReceipt(value.plugin_runner_binary, 'plugin-runner', mainSourceSha);
  const artifact = path.resolve(requireValue(value.artifact, 'artifact path'));
  if (!artifact.includes(`${path.sep}tmp${path.sep}test-governance${path.sep}`)) {
    throw new Error('CountTokens upgrade artifact must be under tmp/test-governance');
  }
  return {
    applicationId: APPLICATION_ID,
    mainSourceSha,
    mainSourceReceipt,
    mainSourceRoot: source.sourceRoot,
    apiServerCwd: source.apiServerCwd,
    apiServer,
    pluginRunner,
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
      ownerUsername: envName(value.environment?.owner_username, 'owner username env name'),
      ownerPassword: envName(value.environment?.owner_password, 'owner password env name'),
      databaseUrl: envName(value.environment?.database_url, 'database URL env name'),
      providerSecretMasterKey: optionalEnvName(
        value.environment?.provider_secret_master_key,
        'provider secret master key env name',
      ),
      providerInstallRoot: optionalEnvName(
        value.environment?.provider_install_root,
        'provider install root env name',
      ),
    },
    cookieName: safeClaim(value.api_cookie_name || 'flowbase_console_session', 'API cookie name'),
    artifact,
  };
}

function envValue(sourceEnv, name, label) {
  return requireValue(sourceEnv?.[name], `${label} (${name})`);
}

function optionalEnvValue(sourceEnv, name) {
  if (!name || typeof sourceEnv?.[name] !== 'string' || !sourceEnv[name].trim()) return null;
  return sourceEnv[name].trim();
}

function publicError(error) {
  return {
    name: error?.name || 'Error',
    code: error?.code || 'execution_failed',
    message: error?.message || String(error),
    ...(error?.diagnostic ? { diagnostic: error.diagnostic } : {}),
  };
}

function sha256File(filePath) {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function loadApiFileEnvironment(apiServerCwd, dependencies = {}) {
  const filePath = path.join(apiServerCwd, '.env');
  const values = (dependencies.parseEnvFile || parseEnvFile)(filePath);
  return {
    values,
    source: {
      path: filePath,
      ...(fs.existsSync(filePath) ? { sha256: sha256File(filePath) } : {}),
    },
  };
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

async function reserveOwnedPort(reservePort, excluded = new Set()) {
  for (let attempt = 0; attempt < 32; attempt += 1) {
    const port = await reservePort();
    if (!FORBIDDEN_PORTS.has(port) && !excluded.has(port)) return port;
  }
  throw new Error('could not reserve a safe ephemeral loopback port');
}

async function startFrozenServices({
  manifest, apiFileEnv, databaseUrl, providerSecretMasterKey, providerInstallRoot, scratchRoot,
  dependencies, services, sourceEnv,
}) {
  const reservePort = dependencies.reserveLoopbackPort || reserveLoopbackPort;
  const spawnProcess = dependencies.spawnOwned || spawnOwned;
  const health = dependencies.waitForHealth || waitForHealth;
  const pluginPort = await reserveOwnedPort(reservePort);
  const apiPort = await reserveOwnedPort(reservePort, new Set([pluginPort]));
  const pluginRunnerBaseUrl = `http://127.0.0.1:${pluginPort}`;
  const apiBaseUrl = `http://127.0.0.1:${apiPort}`;
  const scrubbed = {
    OPENAI_API_KEY: '', OPENAI_BASE_URL: '', ANTHROPIC_API_KEY: '',
    ANTHROPIC_AUTH_TOKEN: '', ANTHROPIC_BASE_URL: '', CLAUDE_CODE_OAUTH_TOKEN: '',
  };
  const apiParentEnv = { ...sourceEnv };
  delete apiParentEnv.API_PROVIDER_INSTALL_ROOT;
  delete apiParentEnv.API_PROVIDER_SECRET_MASTER_KEY;
  services.pluginProcess = spawnProcess(manifest.pluginRunner.path, {
    ...scrubbed,
    PLUGIN_RUNNER_ADDR: `127.0.0.1:${pluginPort}`,
  }, { cwd: scratchRoot, parentEnv: sourceEnv });
  await health(pluginRunnerBaseUrl, 'plugin-runner', {
    fetchImpl: dependencies.fetchImpl || globalThis.fetch,
    processHandle: services.pluginProcess,
  });
  services.apiProcess = spawnProcess(manifest.apiServer.path, {
    ...apiFileEnv,
    ...scrubbed,
    API_ENV: 'development',
    API_SERVER_ADDR: `127.0.0.1:${apiPort}`,
    API_DATABASE_URL: databaseUrl,
    API_DATABASE_POOL_MAX_CONNECTIONS: '5',
    API_PLUGIN_RUNNER_INTERNAL_BASE_URL: pluginRunnerBaseUrl,
    ...(providerInstallRoot ? { API_PROVIDER_INSTALL_ROOT: providerInstallRoot } : {}),
    ...(providerSecretMasterKey ? { API_PROVIDER_SECRET_MASTER_KEY: providerSecretMasterKey } : {}),
    API_COOKIE_NAME: manifest.cookieName,
    API_COOKIE_SECURE: 'false',
  }, { cwd: manifest.apiServerCwd, parentEnv: apiParentEnv });
  await health(apiBaseUrl, 'api-server', {
    fetchImpl: dependencies.fetchImpl || globalThis.fetch,
    processHandle: services.apiProcess,
  });
  return Object.assign(services, {
    apiBaseUrl,
    apiPort,
    pluginPort,
    pluginRunnerBaseUrl,
  });
}

async function runCountTokensUpgrade(rawOptions, dependencies = {}) {
  const sourceEnv = dependencies.sourceEnv || process.env;
  const registry = dependencies.registry || new OwnedResources(dependencies);
  const fetchImpl = dependencies.fetchImpl || globalThis.fetch;
  let manifest = null;
  let owner = null;
  let temporarySession = null;
  let services = null;
  let primaryError = null;
  let observed = null;
  let cleanupErrors = [];
  const secrets = { descriptors: [] };
  let artifactPath = null;
  try {
    manifest = loadRunManifest(rawOptions.manifest, dependencies);
    artifactPath = manifest.artifact;
    const apiKey = envValue(sourceEnv, manifest.env.apiKey, 'application API key');
    const apiKeyId = envValue(sourceEnv, manifest.env.apiKeyId, 'application API key id');
    const ownerUsername = envValue(sourceEnv, manifest.env.ownerUsername, 'owner username');
    const ownerPassword = envValue(sourceEnv, manifest.env.ownerPassword, 'owner password');
    const databaseUrl = databaseUrlFromEnv(sourceEnv, manifest.env.databaseUrl);
    const providerSecretMasterKey = optionalEnvValue(sourceEnv, manifest.env.providerSecretMasterKey);
    const providerInstallRootValue = optionalEnvValue(sourceEnv, manifest.env.providerInstallRoot);
    const providerInstallRoot = providerInstallRootValue
      ? requireDirectory(providerInstallRootValue, 'provider install root override')
      : null;
    const apiEnvironment = loadApiFileEnvironment(manifest.apiServerCwd, dependencies);
    secrets.descriptors.push(
      { kind: 'credential', value: apiKey },
      { kind: 'credential', value: ownerPassword },
      { kind: 'credential', value: providerSecretMasterKey },
      ...Object.entries(apiEnvironment.values)
        .map(([key, value]) => ({ kind: 'env', key, value })),
      { kind: 'credential_url', value: databaseUrl },
    );
    const tempRoot = registry.addTempRoot(fs.mkdtempSync(path.join(os.tmpdir(), '1flowbase-count-tokens-upgrade-')));
    services = {};
    services = await startFrozenServices({
      manifest, apiFileEnv: apiEnvironment.values, databaseUrl,
      providerSecretMasterKey, providerInstallRoot,
      scratchRoot: tempRoot, dependencies, services, sourceEnv,
    });
    manifest.gatewayBaseUrl = services.apiBaseUrl;
    temporarySession = await (dependencies.openTemporaryOwnerSession || openTemporaryOwnerSession)({
      apiBaseUrl: services.apiBaseUrl,
      account: ownerUsername,
      password: ownerPassword,
      fetchImpl,
    });
    secrets.descriptors.push(
      { kind: 'credential', value: temporarySession.cookie },
      { kind: 'credential', value: temporarySession.csrfToken },
    );
    if (!temporarySession.cookie.startsWith(`${manifest.cookieName}=`)) {
      throw new Error('owner session cookie does not match the frozen API cookie name');
    }
    owner = new (dependencies.OwnerHttpClient || OwnerHttpClient)(services.apiBaseUrl, fetchImpl);
    owner.attachSession(temporarySession.cookie, temporarySession.csrfToken);
    await assertTokenBinding(owner, manifest.applicationId, apiKeyId, apiKey);
    const publicationId = await activePublication(owner, manifest.applicationId);
    const beforePlugin = await readDeepSeekFamily(owner);
    const tokenResult = await countTokens(manifest.gatewayBaseUrl, apiKey, manifest.model, fetchImpl);
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
      runtime: {
        main_source_sha: manifest.mainSourceSha,
        main_source_receipt: manifest.mainSourceReceipt,
        main_source_root: manifest.mainSourceRoot,
        api_server_cwd: manifest.apiServerCwd,
        api_env_source: apiEnvironment.source,
        api_server: { ...manifest.apiServer, port: services.apiPort },
        plugin_runner: { ...manifest.pluginRunner, port: services.pluginPort },
      },
    };
  } catch (error) {
    primaryError = error;
  } finally {
    const stopProcess = dependencies.stopOwned || stopOwned;
    if (temporarySession) {
      try { await temporarySession.dispose(); }
      catch (error) { cleanupErrors.push({ owner: 'owner-session', message: error.message }); }
    }
    for (const [ownerName, processHandle] of [
      ['api-server', services?.apiProcess],
      ['plugin-runner', services?.pluginProcess],
    ]) {
      if (!processHandle) continue;
      try { await stopProcess(processHandle); }
      catch (error) { cleanupErrors.push({ owner: ownerName, message: error.message }); }
    }
    try { cleanupErrors.push(...await registry.close()); }
    catch (error) { cleanupErrors.push({ owner: 'owned-resources', message: error.message }); }
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
  loadRunManifest,
  loadApiFileEnvironment,
  reserveOwnedPort,
  runCountTokensUpgrade,
  verifiedSourceCwd,
};
