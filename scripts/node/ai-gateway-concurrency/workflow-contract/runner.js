'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { TRANSPORT } = require('../contracts');
const { runGatewayCharacterize } = require('../characterize/engine');
const { createGatewayFixture } = require('../gateway-fixture');
const { createMockUpstream } = require('../mock-upstream');
const { loadPinnedInventory } = require('../wire-audit/inventory');
const { runWireAudit } = require('../wire-audit/runner');
const { normalizeRunInputs } = require('./inputs');
const {
  prepareEvidence,
  publicError,
  workflowResultBase,
  writeJson,
} = require('./evidence');

const SECRET_CANARY = 'sk-1flowbase-controlled-secret-canary';

function requireReadyEndpoint(value, provider, pathname) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error(`WP3 ready manifest ${provider} endpoint is invalid`);
  }
  if (
    url.protocol !== 'http:'
    || !['127.0.0.1', '::1', 'localhost'].includes(url.hostname.toLowerCase())
    || url.username
    || url.password
    || url.search
    || url.hash
    || url.pathname !== pathname
  ) {
    throw new Error(`WP3 ready manifest ${provider} endpoint must be credential-free loopback ${pathname}`);
  }
  return url.href;
}

function readReadyManifest(filePath) {
  const manifest = JSON.parse(fs.readFileSync(filePath, 'utf8'));
  if (manifest.schema_version !== '1flowbase.ai-gateway-fixture/v1') {
    throw new Error('WP3 ready manifest schema mismatch');
  }
  for (const provider of ['openai', 'anthropic']) {
    const target = manifest.targets?.[provider];
    if (typeof target?.api_key !== 'string' || !target.api_key.trim()) {
      throw new Error(`WP3 ready manifest omitted ${provider} Application API key`);
    }
    if (typeof target.model !== 'string' || !target.model.trim()) {
      throw new Error(`WP3 ready manifest omitted ${provider} published model`);
    }
    for (const key of ['query_run', 'list_runs']) {
      if (typeof target.durable?.[key]?.url !== 'string' && typeof target.durable?.[key]?.url_template !== 'string') {
        throw new Error(`WP3 ready manifest omitted ${provider} durable ${key} endpoint`);
      }
    }
    if (typeof target.runtime_activity?.url !== 'string') {
      throw new Error(`WP3 ready manifest omitted ${provider} runtime activity endpoint`);
    }
    if (typeof target.plugin_runner_active_streams?.url !== 'string') {
      throw new Error(`WP3 ready manifest omitted ${provider} plugin active streams endpoint`);
    }
  }
  const anthropicPool = manifest.pools?.anthropic;
  if (!Array.isArray(anthropicPool) || anthropicPool.length !== 2) {
    throw new Error('WP3 ready manifest requires exactly two Anthropic pool targets');
  }
  for (const [index, target] of anthropicPool.entries()) {
    for (const field of ['application_id', 'provider_instance_id', 'api_key', 'model', 'publication_id']) {
      if (typeof target?.[field] !== 'string' || !target[field].trim()) {
        throw new Error(`WP3 ready manifest Anthropic pool target ${index} omitted ${field}`);
      }
    }
    for (const key of ['query_run', 'list_runs']) {
      if (typeof target.durable?.[key]?.url !== 'string' && typeof target.durable?.[key]?.url_template !== 'string') {
        throw new Error(`WP3 ready manifest Anthropic pool target ${index} omitted durable ${key}`);
      }
    }
    if (typeof target.runtime_activity?.url !== 'string' || typeof target.plugin_runner_active_streams?.url !== 'string') {
      throw new Error(`WP3 ready manifest Anthropic pool target ${index} omitted runtime endpoints`);
    }
    requireReadyEndpoint(
      target.gateway?.anthropic_messages_url,
      `Anthropic pool target ${index}`,
      '/v1/messages'
    );
  }
  for (const field of ['application_id', 'provider_instance_id', 'api_key']) {
    if (new Set(anthropicPool.map((target) => target[field])).size !== 2) {
      throw new Error(`WP3 ready manifest Anthropic pool reused ${field}`);
    }
  }
  for (const field of ['application_id', 'provider_instance_id', 'api_key', 'model']) {
    if (manifest.targets.anthropic[field] !== anthropicPool[0][field]) {
      throw new Error(`WP3 ready manifest Anthropic primary target mismatched pool ${field}`);
    }
  }
  if (manifest.targets.anthropic.gateway.anthropic_messages_url !== anthropicPool[0].gateway.anthropic_messages_url) {
    throw new Error('WP3 ready manifest Anthropic primary target mismatched pool endpoint');
  }
  if (manifest.targets.openai.api_key === manifest.targets.anthropic.api_key) {
    throw new Error('WP3 ready manifest reused an Application API key');
  }
  requireReadyEndpoint(
    manifest.targets.openai.gateway?.responses_url,
    'OpenAI Responses',
    '/v1/responses'
  );
  requireReadyEndpoint(
    manifest.targets.anthropic.gateway?.anthropic_messages_url,
    'Anthropic Messages',
    '/v1/messages'
  );
  return manifest;
}

function wireAuditManifest(ready) {
  const controlled = ready.controlled_upstream;
  return {
    gatewayBaseUrl: new URL(ready.targets.openai.gateway.responses_url).origin,
    openai: ready.targets.openai,
    anthropic: ready.targets.anthropic,
    controlledUpstream: controlled ? {
      snapshotUrl: controlled.snapshot_url,
      barrierReleaseUrl: controlled.barrier_release_url,
      networkObserverUrl: controlled.network_observer_url,
      gatewayExecutorObserverUrl: controlled.gateway_executor_observer_url,
    } : null,
  };
}

function characterizeOptions({ repoRoot, ready, websocketBaseUrl, mockSnapshot }) {
  return {
    repoRoot,
    endpointSet: {
      [TRANSPORT.RESPONSES_SSE]: ready.targets.openai.gateway.responses_url,
      [TRANSPORT.RESPONSES_WEBSOCKET]: `${websocketBaseUrl}/v1/responses`,
      [TRANSPORT.ANTHROPIC_SSE]: ready.targets.anthropic.gateway.anthropic_messages_url,
    },
    authorizationTokenByTransport: {
      [TRANSPORT.RESPONSES_SSE]: ready.targets.openai.api_key,
      [TRANSPORT.ANTHROPIC_SSE]: ready.targets.anthropic.api_key,
    },
    modelByTransport: {
      [TRANSPORT.RESPONSES_SSE]: ready.targets.openai.model,
      [TRANSPORT.ANTHROPIC_SSE]: ready.targets.anthropic.model,
    },
    mockSnapshot,
    durableTargetsByTransport: {
      [TRANSPORT.RESPONSES_SSE]: ready.targets.openai,
      [TRANSPORT.ANTHROPIC_SSE]: ready.targets.anthropic,
    },
    anthropicTargetPool: ready.pools.anthropic,
  };
}

async function runWorkflowContract(rawOptions, dependencies = {}) {
  const inputs = normalizeRunInputs(rawOptions);
  const paths = prepareEvidence(inputs.repoRoot);
  const createMock = dependencies.createMockUpstream ?? createMockUpstream;
  const createFixture = dependencies.createGatewayFixture ?? createGatewayFixture;
  const wireAuditRunner = dependencies.runWireAudit ?? runWireAudit;
  const characterize = dependencies.runGatewayCharacterize ?? runGatewayCharacterize;
  let mock = null;
  let fixture = null;
  let ready = null;
  let wireAudit = null;
  let characterizeResult = null;
  let executionError = null;
  const cleanupErrors = [];

  try {
    mock = createMock();
    const mockEndpoints = await mock.start();
    fixture = await createFixture({
      databaseUrl: inputs.databaseUrl,
      apiServerBin: inputs.apiServerBin,
      pluginRunnerBin: inputs.pluginRunnerBin,
      openaiPackage: inputs.openaiPackage,
      anthropicPackage: inputs.anthropicPackage,
      upstreamBaseUrl: mockEndpoints.httpBaseUrl,
      artifactRoot: paths.root,
    });
    writeJson(paths.readyFile, fixture.result, 0o600);
    ready = readReadyManifest(paths.readyFile);
    const compatibilityManifest = wireAuditManifest(ready);
    writeJson(path.join(paths.root, 'wire-inventory.json'), loadPinnedInventory());
    wireAudit = await wireAuditRunner({ manifest: compatibilityManifest }, {
      secretCanary: SECRET_CANARY,
    });
    writeJson(path.join(paths.root, 'wire-audit.json'), wireAudit);

    characterizeResult = await characterize(characterizeOptions({
      repoRoot: inputs.repoRoot,
      ready,
      websocketBaseUrl: mockEndpoints.websocketBaseUrl,
      mockSnapshot: mock.snapshot,
    }));
    if (characterizeResult.summary.verdict !== 'PASS') {
      throw new Error(`gateway characterize verdict was ${characterizeResult.summary.verdict}`);
    }
  } catch (error) {
    executionError = error;
  } finally {
    fs.rmSync(paths.readyFile, { force: true });
    try {
      await fixture?.close();
    } catch (error) {
      cleanupErrors.push(error);
    }
    try {
      await mock?.stop();
    } catch (error) {
      cleanupErrors.push(error);
    }
  }

  const secrets = [
    ready?.targets?.openai?.api_key,
    ...(ready?.pools?.anthropic ?? []).map((target) => target.api_key),
    SECRET_CANARY,
  ].filter(Boolean);
  const finalError = executionError ?? cleanupErrors[0] ?? null;
  const result = {
    ...workflowResultBase(inputs),
    status: finalError ? 'fail' : 'pass',
    protocol_conformance: wireAudit ? { status: 'pass', wire_audit: wireAudit } : null,
    targets: ready ? {
      openai: {
        model: ready.targets.openai.model,
        package_sha256: ready.packages?.openai?.sha256 ?? null,
      },
      anthropic: {
        model: ready.targets.anthropic.model,
        package_sha256: ready.packages?.anthropic?.sha256 ?? null,
      },
      anthropic_pool: ready.pools.anthropic.map((target) => ({
        application_id: target.application_id,
        provider_instance_id: target.provider_instance_id,
        model: target.model,
      })),
    } : null,
    characterize: characterizeResult ? {
      verdict: characterizeResult.summary.verdict,
      requests: characterizeResult.summary.totals.requests,
      blocking_requests: characterizeResult.summary.totals.blockingRequests,
      performance_requests: characterizeResult.summary.totals.advisoryRequests,
      contract_failures: characterizeResult.summary.totals.contractFailures,
      performance_and_observability_advisories: characterizeResult.summary.totals.advisoryFailures,
      durable_convergence: characterizeResult.summary.durableConvergence ?? null,
      artifact_root: path.relative(inputs.repoRoot, characterizeResult.artifacts.outputDirectory),
    } : null,
    cleanup: {
      status: cleanupErrors.length === 0 ? 'pass' : 'fail',
      errors: cleanupErrors.map((error) => publicError(error, secrets)),
    },
    error: finalError ? publicError(finalError, secrets) : null,
  };
  writeJson(paths.resultFile, result);
  return result;
}

module.exports = {
  characterizeOptions,
  readReadyManifest,
  requireReadyEndpoint,
  runWorkflowContract,
  wireAuditManifest,
};
