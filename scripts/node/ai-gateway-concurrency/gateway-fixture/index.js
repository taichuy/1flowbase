'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { bootstrapGateway } = require('./bootstrap');
const { OwnerHttpClient } = require('./http-owner');
const { normalizeOptions } = require('./inputs');
const {
  assertLoopbackPortAvailable,
  reserveLoopbackPort,
  spawnOwned,
  stopOwned,
  waitForHealth,
} = require('./process-owner');
const { persistServiceLogs, redactServiceLog } = require('./service-logs');
const { assertNoArtifactSecrets } = require('../cli-smoke/artifact-scan');

function sha256File(filePath) {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function providerTarget(baseUrl, pluginRunnerBaseUrl, client, provider) {
  const ownerHeaders = { cookie: client.cookie };
  return {
    application_id: provider.application_id,
    provider_instance_id: provider.provider_instance_id,
    installation_id: provider.installation_id,
    model: provider.model,
    api_key_id: provider.api_key_id,
    api_key: provider.api_key,
    publication_id: provider.publication_id,
    gateway: {
      base_url: baseUrl,
      responses_url: `${baseUrl}/v1/responses`,
      chat_completions_url: `${baseUrl}/v1/chat/completions`,
      anthropic_messages_url: `${baseUrl}/v1/messages`,
      authorization: `Bearer ${provider.api_key}`,
    },
    durable: {
      query_run: {
        method: 'GET',
        url_template: `${baseUrl}/api/agent/v1/runs/{run_id}`,
        headers: { authorization: `Bearer ${provider.api_key}` },
      },
      cancel_run: {
        method: 'POST',
        url_template: `${baseUrl}/api/agent/v1/runs/{run_id}/cancel`,
        headers: { authorization: `Bearer ${provider.api_key}` },
      },
      list_runs: {
        method: 'GET',
        url: `${baseUrl}/api/console/applications/${provider.application_id}/logs/runs?page=1&page_size=100&cache_mode=refresh`,
        headers: ownerHeaders,
      },
    },
    runtime_activity: {
      method: 'GET',
      url: `${baseUrl}/api/console/applications/${provider.application_id}/monitoring/runtime-activity`,
      headers: ownerHeaders,
    },
    plugin_runner_active_streams: {
      method: 'GET',
      url: `${pluginRunnerBaseUrl}/providers/active-streams`,
    },
  };
}

async function createGatewayFixture(rawOptions, dependencies = {}) {
  const options = normalizeOptions(rawOptions);
  const scratchRoot = fs.mkdtempSync(path.join(os.tmpdir(), '1flowbase-ai-gateway-fixture-'));
  const reservePort = dependencies.reserveLoopbackPort || reserveLoopbackPort;
  const assertPortAvailable = dependencies.assertLoopbackPortAvailable || assertLoopbackPortAvailable;
  const spawnProcess = dependencies.spawnOwned || spawnOwned;
  const health = dependencies.waitForHealth || waitForHealth;
  const stopProcess = dependencies.stopOwned || stopOwned;
  const persistLogs = dependencies.persistServiceLogs || persistServiceLogs;
  const removeScratch = dependencies.removeScratch || ((target) => fs.rmSync(target, { recursive: true, force: true }));
  const Client = dependencies.OwnerHttpClient || OwnerHttpClient;
  let apiProcess = null;
  let runnerProcess = null;
  let rootPassword = null;
  let providerSecretMasterKey = null;
  let ownerClient = null;
  let providers = null;
  let closed = false;
  const fixtureSecrets = () => {
    const databasePassword = (() => {
      try { return new URL(options.databaseUrl).password; } catch { return ''; }
    })();
    const applicationKeys = providers ? [
      providers.openai.api_key,
      providers.openai_compatible.api_key,
      ...providers.anthropic.map((provider) => provider.api_key),
    ] : [];
    return [
      options.databaseUrl,
      databasePassword,
      rootPassword,
      providerSecretMasterKey,
      ownerClient?.cookie,
      'fixture-openai-token',
      'fixture-anthropic-token',
      'fixture-openai_compatible-token',
      ...applicationKeys,
    ];
  };
  const cleanup = async () => {
    let firstError = null;
    try {
      persistLogs({
        artifactRoot: options.artifactRoot,
        services: { 'api-server': apiProcess, 'plugin-runner': runnerProcess },
        secrets: fixtureSecrets(),
      });
      assertNoArtifactSecrets([options.artifactRoot], fixtureSecrets());
    } catch (error) {
      firstError ||= error;
    }
    for (const handle of [apiProcess, runnerProcess]) {
      try {
        await stopProcess(handle);
      } catch (error) {
        firstError ||= error;
      }
    }
    try {
      removeScratch(scratchRoot);
    } catch (error) {
      firstError ||= error;
    }
    if (firstError) {
      firstError.message = redactServiceLog(firstError.message, fixtureSecrets());
      throw firstError;
    }
  };
  try {
    let runnerPort = await reservePort();
    while (options.apiPort !== null && runnerPort === options.apiPort) runnerPort = await reservePort();
    let apiPort = options.apiPort;
    if (apiPort === null) {
      apiPort = await reservePort();
      while (apiPort === runnerPort) apiPort = await reservePort();
    }
    if (options.apiPort !== null) await assertPortAvailable(apiPort);
    const pluginRunnerBaseUrl = `http://127.0.0.1:${runnerPort}`;
    const gatewayBaseUrl = `http://127.0.0.1:${apiPort}`;
    const scrubbedCredentials = { OPENAI_API_KEY: '', ANTHROPIC_API_KEY: '' };
    runnerProcess = spawnProcess(options.pluginRunnerBin, {
      ...scrubbedCredentials,
      PLUGIN_RUNNER_ADDR: `127.0.0.1:${runnerPort}`,
      RUST_LOG: process.env.RUST_LOG || 'info',
    }, { cwd: scratchRoot });
    await health(pluginRunnerBaseUrl, 'plugin-runner', { processHandle: runnerProcess });

    const rootAccount = `gateway_fixture_${crypto.randomBytes(4).toString('hex')}`;
    rootPassword = `Fixture-${crypto.randomBytes(18).toString('base64url')}`;
    providerSecretMasterKey = crypto.randomBytes(32).toString('base64url');
    apiProcess = spawnProcess(options.apiServerBin, {
      ...scrubbedCredentials,
      API_ENV: 'development',
      API_SERVER_ADDR: `127.0.0.1:${apiPort}`,
      API_DATABASE_URL: options.databaseUrl,
      API_DATABASE_POOL_MAX_CONNECTIONS: '5',
      API_PLUGIN_RUNNER_INTERNAL_BASE_URL: pluginRunnerBaseUrl,
      API_PROVIDER_INSTALL_ROOT: path.join(scratchRoot, 'providers'),
      API_COOKIE_NAME: `gateway_fixture_${crypto.randomBytes(5).toString('hex')}`,
      API_COOKIE_SECURE: 'false',
      API_PROVIDER_SECRET_MASTER_KEY: providerSecretMasterKey,
      BOOTSTRAP_WORKSPACE_NAME: 'Gateway Fixture Workspace',
      BOOTSTRAP_ROOT_ACCOUNT: rootAccount,
      BOOTSTRAP_ROOT_EMAIL: `${rootAccount}@example.invalid`,
      BOOTSTRAP_ROOT_PASSWORD: rootPassword,
      BOOTSTRAP_ROOT_NAME: 'Gateway Fixture Root',
      BOOTSTRAP_ROOT_NICKNAME: 'Gateway Fixture',
      RUST_LOG: process.env.RUST_LOG || 'info',
    }, { cwd: scratchRoot });
    await health(gatewayBaseUrl, 'api-server', { processHandle: apiProcess });

    const client = new Client(gatewayBaseUrl, dependencies.fetchImpl || globalThis.fetch);
    ownerClient = client;
    const model = 'gateway-fixture-model';
    providers = await bootstrapGateway(client, {
      ...options,
      rootAccount,
      rootPassword,
      model,
    });
    const currentDigests = {
      openai: sha256File(options.openaiPackage),
      anthropic: sha256File(options.anthropicPackage),
      openai_compatible: sha256File(options.openaiCompatiblePackage),
    };
    for (const code of ['openai', 'anthropic', 'openai_compatible']) {
      const candidates = Array.isArray(providers[code]) ? providers[code] : [providers[code]];
      if (candidates.some((provider) => provider.package_sha256 !== currentDigests[code])) {
        throw new Error(`official ${code} package archive changed during bootstrap`);
      }
    }
    const anthropicPool = providers.anthropic.map(
      (provider) => providerTarget(gatewayBaseUrl, pluginRunnerBaseUrl, client, provider)
    );
    const targets = {
      openai: providerTarget(gatewayBaseUrl, pluginRunnerBaseUrl, client, providers.openai),
      openai_compatible: providerTarget(
        gatewayBaseUrl,
        pluginRunnerBaseUrl,
        client,
        providers.openai_compatible
      ),
      anthropic: anthropicPool[0],
    };
    const result = {
      schema_version: '1flowbase.ai-gateway-fixture/v1',
      artifact_root: options.artifactRoot,
      gateway_base_url: gatewayBaseUrl,
      plugin_runner_base_url: pluginRunnerBaseUrl,
      model,
      packages: {
        openai: { path: options.openaiPackage, sha256: currentDigests.openai },
        anthropic: { path: options.anthropicPackage, sha256: currentDigests.anthropic },
        openai_compatible: {
          path: options.openaiCompatiblePackage,
          sha256: currentDigests.openai_compatible,
        },
      },
      targets,
      pools: { anthropic: anthropicPool },
      controlled_upstream: {
        snapshot_url: `${options.upstreamBaseUrl}/__control/snapshot`,
        barrier_release_url: `${options.upstreamBaseUrl}/__control/barrier/release`,
        network_observer_url: `${options.upstreamBaseUrl}/__observer/mcp-network`,
        gateway_executor_observer_url: `${options.upstreamBaseUrl}/__observer/gateway-executor`,
      },
    };

    return {
      result,
      async close() {
        if (closed) return;
        closed = true;
        await cleanup();
      },
    };
  } catch (error) {
    try {
      await cleanup();
    } catch (cleanupError) {
      error.message = `${error.message}; cleanup failed: ${cleanupError.message}`;
    }
    const output = `${apiProcess?.output?.() || ''}${runnerProcess?.output?.() || ''}`.trim();
    if (output) error.message = `${error.message}; owned process output: ${output.slice(-4000)}`;
    error.message = redactServiceLog(error.message, fixtureSecrets());
    throw error;
  }
}

module.exports = { createGatewayFixture, providerTarget, sha256File };
