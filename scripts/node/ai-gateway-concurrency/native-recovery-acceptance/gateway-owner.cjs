'use strict';

// Task-owned fixture only. No shared database, account, provider or listener.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const ROOT = path.resolve(__dirname, '../../../..');
const API_BINARY = process.env.NRA_API_BINARY || path.join(ROOT, 'api/target/debug/api-server');
const PACKAGE = process.env.NRA_PLUGIN_PACKAGE || path.join(ROOT, 'api/plugins/packages/openai/openai@0.2.71.1flowbasepkg');
const req = (file) => require(path.join(ROOT, 'scripts/node', file));
const { createDatabase } = req('ai-gateway-concurrency/local-acceptance/system.js');
const { reserveLoopbackPort, spawnOwned, waitForHealth, stopOwned } = req('ai-gateway-concurrency/gateway-fixture/process-owner.js');
const { OwnerHttpClient } = req('ai-gateway-concurrency/gateway-fixture/http-owner.js');
const { installProvider, createPublishedApplication } = req('ai-gateway-concurrency/gateway-fixture/bootstrap.js');
const { openTemporaryOwnerSession } = req('page-debug/auth.js');
const { persistServiceLogs } = req('ai-gateway-concurrency/gateway-fixture/service-logs.js');

async function createOwnedGateway({ upstreamBaseUrl, artifactRoot }) {
  const upstream = new URL(upstreamBaseUrl);
  if (upstream.protocol !== 'http:' || !['127.0.0.1', 'localhost'].includes(upstream.hostname)
      || upstream.username || upstream.password || upstream.search || upstream.hash || upstream.pathname !== '/') {
    throw new Error('upstreamBaseUrl must be a plain HTTP loopback mock origin');
  }
  if (!path.isAbsolute(artifactRoot)) throw new Error('artifactRoot must be absolute');
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'native409-repro-'));
  const account = `native409_${crypto.randomBytes(8).toString('hex')}`;
  const password = `Fixture-${crypto.randomBytes(20).toString('base64url')}`;
  const masterKey = crypto.randomBytes(32).toString('base64url');
  let database, service, session, published;
  let closed = false;
  const close = async () => {
    if (closed) return;
    closed = true;
    const errors = [];
    // Revocation must happen while our own API is still alive.
    try { await session?.dispose(); } catch (error) { errors.push(error); }
    try {
      persistServiceLogs({ artifactRoot, services: { 'api-server': service },
        secrets: [password, masterKey, database?.url, session?.cookie, session?.csrfToken,
          published?.api_key, 'fixture-openai-token'].filter(Boolean) });
    } catch (error) { errors.push(error); }
    try { await stopOwned(service); } catch (error) { errors.push(error); }
    if (service?.child?.exitCode === null && service.child.signalCode === null) {
      await new Promise((resolve) => service.child.once('exit', resolve));
    }
    try { await database?.close(); } catch (error) { errors.push(error); }
    try { fs.rmSync(scratch, { recursive: true, force: true }); } catch (error) { errors.push(error); }
    if (errors.length) throw new Error(`Owned gateway cleanup failed: ${errors.map((e) => e.message).join('; ')}`);
  };
  try {
    database = createDatabase({ container: 'docker-db-1', host: '127.0.0.1', port: 35432 });
    const port = await reserveLoopbackPort();
    if (port === 7600) throw new Error('Refusing shared API port');
    const baseUrl = `http://127.0.0.1:${port}`;
    service = spawnOwned(API_BINARY, {
      OPENAI_API_KEY: '', ANTHROPIC_API_KEY: '',
      API_ENV: 'development', API_SERVER_ADDR: `127.0.0.1:${port}`,
      API_DATABASE_URL: database.url, API_DATABASE_POOL_MAX_CONNECTIONS: '5',
      API_PROVIDER_INSTALL_ROOT: path.join(scratch, 'providers'),
      API_COOKIE_NAME: account, API_COOKIE_SECURE: 'false',
      API_PROVIDER_SECRET_MASTER_KEY: masterKey,
      API_PLUGIN_UPLOAD_MAX_BYTES: String(fs.statSync(PACKAGE).size + 4096),
      BOOTSTRAP_WORKSPACE_NAME: 'Native409 isolated fixture',
      BOOTSTRAP_ROOT_ACCOUNT: account, BOOTSTRAP_ROOT_EMAIL: `${account}@example.invalid`,
      BOOTSTRAP_ROOT_PASSWORD: password, BOOTSTRAP_ROOT_NAME: 'Native409 fixture',
      BOOTSTRAP_ROOT_NICKNAME: 'Native409 fixture', RUST_LOG: 'info',
    }, { cwd: scratch, parentEnv: { PATH: process.env.PATH, LANG: 'C.UTF-8' } });
    await waitForHealth(baseUrl, 'api-server', { processHandle: service, timeoutMs: 60000 });
    session = await openTemporaryOwnerSession({ apiBaseUrl: baseUrl, account, password });
    const client = new OwnerHttpClient(baseUrl);
    client.attachSession(session.cookie, session.csrfToken);
    const installation = await installProvider(client,
      PACKAGE, 'openai');
    const model = 'gpt-6-luna';
    const instance = await client.write('/api/console/settings/model-providers/instances', 'POST', {
      installation_id: installation.installation_id, display_name: account,
      configured_models: [{ model_id: model, enabled: true,
        context_window_override_tokens: null, supports_multimodal: false }],
      enabled_model_ids: [model], included_in_main: true, preview_token: null,
      config: { base_url: `${upstream.origin}/v1`, api_key: 'fixture-openai-token',
        validate_model: false, transport_mode: 'responses_websocket' },
    });
    if (!instance.data?.id) throw new Error('Provider instance omitted id');
    published = await createPublishedApplication(client, {
      ...installation, provider_instance_id: instance.data.id, model,
    }, model);
    const target = {
      application_id: published.application_id,
      provider_instance_id: published.provider_instance_id,
      installation_id: published.installation_id,
      model, reasoning_effort: 'max', publication_id: published.publication_id,
      gateway: { base_url: baseUrl, responses_url: `${baseUrl}/v1/responses`,
        websocket_url: `${baseUrl.replace('http:', 'ws:')}/v1/responses`,
        authorization: `Bearer ${published.api_key}` },
      durable: { query_run: { method: 'GET',
        url_template: `${baseUrl}/api/agent/v1/runs/{run_id}`,
        headers: { authorization: `Bearer ${published.api_key}` } } },
    };
    return { baseUrl, target, targets: { openai: target }, ownerClient: client,
      gatewayPid: service.child.pid, databaseName: database.name, close };
  } catch (error) {
    try { await close(); } catch (cleanupError) {
      throw new Error(`${error.message}; ${cleanupError.message}`);
    }
    throw error;
  }
}

module.exports = { createOwnedGateway };
