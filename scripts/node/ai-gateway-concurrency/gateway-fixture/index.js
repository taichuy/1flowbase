'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { bootstrapGateway } = require('./bootstrap');
const { OwnerHttpClient } = require('./http-owner');
const { normalizeOptions } = require('./inputs');
const { reserveLoopbackPort, spawnOwned, stopOwned, waitForHealth } = require('./process-owner');

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
        url: `${baseUrl}/api/console/applications/${provider.application_id}/logs/runs?page=1&page_size=100&cache_mode=bypass`,
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
  const spawnProcess = dependencies.spawnOwned || spawnOwned;
  const health = dependencies.waitForHealth || waitForHealth;
  const stopProcess = dependencies.stopOwned || stopOwned;
  const Client = dependencies.OwnerHttpClient || OwnerHttpClient;
  let apiProcess = null;
  let runnerProcess = null;
  let closed = false;
  const cleanup = async () => {
    let firstError = null;
    for (const handle of [apiProcess, runnerProcess]) {
      try {
        await stopProcess(handle);
      } catch (error) {
        firstError ||= error;
      }
    }
    try {
      fs.rmSync(scratchRoot, { recursive: true, force: true });
    } catch (error) {
      firstError ||= error;
    }
    if (firstError) throw firstError;
  };
  try {
    const runnerPort = await reservePort();
    let apiPort = await reservePort();
    while (apiPort === runnerPort) apiPort = await reservePort();
    const pluginRunnerBaseUrl = `http://127.0.0.1:${runnerPort}`;
    const gatewayBaseUrl = `http://127.0.0.1:${apiPort}`;
    const scrubbedCredentials = { OPENAI_API_KEY: '', ANTHROPIC_API_KEY: '' };
    runnerProcess = spawnProcess(options.pluginRunnerBin, {
      ...scrubbedCredentials,
      PLUGIN_RUNNER_ADDR: `127.0.0.1:${runnerPort}`,
      RUST_LOG: process.env.RUST_LOG || 'info',
    }, { cwd: scratchRoot });
    await health(pluginRunnerBaseUrl, 'plugin-runner');

    const rootAccount = `gateway_fixture_${crypto.randomBytes(4).toString('hex')}`;
    const rootPassword = `Fixture-${crypto.randomBytes(18).toString('base64url')}`;
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
      API_PROVIDER_SECRET_MASTER_KEY: crypto.randomBytes(32).toString('base64url'),
      BOOTSTRAP_WORKSPACE_NAME: 'Gateway Fixture Workspace',
      BOOTSTRAP_ROOT_ACCOUNT: rootAccount,
      BOOTSTRAP_ROOT_EMAIL: `${rootAccount}@example.invalid`,
      BOOTSTRAP_ROOT_PASSWORD: rootPassword,
      BOOTSTRAP_ROOT_NAME: 'Gateway Fixture Root',
      BOOTSTRAP_ROOT_NICKNAME: 'Gateway Fixture',
      RUST_LOG: process.env.RUST_LOG || 'info',
    }, { cwd: scratchRoot });
    await health(gatewayBaseUrl, 'api-server');

    const client = new Client(gatewayBaseUrl, dependencies.fetchImpl || globalThis.fetch);
    const model = 'gateway-fixture-model';
    const providers = await bootstrapGateway(client, {
      ...options,
      rootAccount,
      rootPassword,
      model,
    });
    const currentDigests = {
      openai: sha256File(options.openaiPackage),
      anthropic: sha256File(options.anthropicPackage),
    };
    for (const code of ['openai', 'anthropic']) {
      if (providers[code].package_sha256 !== currentDigests[code]) {
        throw new Error(`official ${code} package archive changed during bootstrap`);
      }
    }
    const targets = Object.fromEntries(
      Object.entries(providers).map(([code, provider]) => [
        code,
        providerTarget(gatewayBaseUrl, pluginRunnerBaseUrl, client, provider),
      ])
    );
    const result = {
      schema_version: '1flowbase.ai-gateway-fixture/v1',
      gateway_base_url: gatewayBaseUrl,
      plugin_runner_base_url: pluginRunnerBaseUrl,
      model,
      packages: {
        openai: { path: options.openaiPackage, sha256: currentDigests.openai },
        anthropic: { path: options.anthropicPackage, sha256: currentDigests.anthropic },
      },
      targets,
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
    throw error;
  }
}

module.exports = { createGatewayFixture, providerTarget, sha256File };
