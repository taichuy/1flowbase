'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { TRANSPORT } = require('../contracts');
const { runGatewayCharacterize } = require('../characterize/engine');
const { runCliSmoke } = require('../cli-smoke');
const { createGatewayFixture } = require('../gateway-fixture');
const { createMockUpstream } = require('../mock-upstream');
const { normalizeRunInputs } = require('./inputs');
const {
  prepareEvidence,
  publicError,
  workflowResultBase,
  writeJson,
} = require('./evidence');

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

async function runWorkflowContract(rawOptions, dependencies = {}) {
  const inputs = normalizeRunInputs(rawOptions);
  const paths = prepareEvidence(inputs.repoRoot);
  const createMock = dependencies.createMockUpstream ?? createMockUpstream;
  const createFixture = dependencies.createGatewayFixture ?? createGatewayFixture;
  const smoke = dependencies.runCliSmoke ?? runCliSmoke;
  const characterize = dependencies.runGatewayCharacterize ?? runGatewayCharacterize;
  let mock = null;
  let fixture = null;
  let ready = null;
  let cliSmoke = null;
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
    });
    writeJson(paths.readyFile, fixture.result, 0o600);
    ready = readReadyManifest(paths.readyFile);

    cliSmoke = await smoke({
      readyManifest: paths.readyFile,
      codexExecutable: inputs.codexExecutable,
      claudeExecutable: inputs.claudeExecutable,
    });

    characterizeResult = await characterize({
      repoRoot: inputs.repoRoot,
      endpointSet: {
        [TRANSPORT.RESPONSES_SSE]: ready.targets.openai.gateway.responses_url,
        [TRANSPORT.RESPONSES_WEBSOCKET]: mockEndpoints.websocketBaseUrl + '/v1/responses',
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
      mockSnapshot: mock.snapshot,
    });
    if (characterizeResult.summary.verdict !== 'PASS') {
      throw new Error(`gateway characterize verdict was ${characterizeResult.summary.verdict}`);
    }
  } catch (error) {
    executionError = error;
  } finally {
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
    fs.rmSync(paths.readyFile, { force: true });
  }

  const secrets = [ready?.targets?.openai?.api_key, ready?.targets?.anthropic?.api_key].filter(Boolean);
  const finalError = executionError ?? cleanupErrors[0] ?? null;
  const result = {
    ...workflowResultBase(inputs),
    status: finalError ? 'fail' : 'pass',
    cli_smoke: cliSmoke ? {
      status: cliSmoke.status,
      codex_event_count: cliSmoke.codex_event_count,
      claude_event_count: cliSmoke.claude_event_count,
    } : null,
    targets: ready ? {
      openai: {
        model: ready.targets.openai.model,
        package_sha256: ready.packages?.openai?.sha256 ?? null,
      },
      anthropic: {
        model: ready.targets.anthropic.model,
        package_sha256: ready.packages?.anthropic?.sha256 ?? null,
      },
    } : null,
    characterize: characterizeResult ? {
      verdict: characterizeResult.summary.verdict,
      requests: characterizeResult.summary.totals.requests,
      contract_failures: characterizeResult.summary.totals.contractFailures,
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

module.exports = { readReadyManifest, requireReadyEndpoint, runWorkflowContract };
