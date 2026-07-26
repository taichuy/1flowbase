'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { runGatewayCharacterize } = require('../characterize/engine');
const { createGatewayFixture } = require('../gateway-fixture');
const { targetsFromReady } = require('../local-client-acceptance/contract');
const { runLocalClientAcceptance } = require('../local-client-acceptance/driver');
const { createMockUpstream } = require('../mock-upstream');
const { runWireAudit } = require('../wire-audit/runner');
const { characterizeOptions, wireAuditManifest } = require('../workflow-contract/runner');
const { loadManifest, resolveArtifactInventory } = require('./manifest');
const system = require('./system');

function publicError(error) {
  return { name: error?.name || 'Error', message: error?.message || String(error) };
}

async function closeOwned(label, owned, cleanupErrors) {
  if (!owned?.close) return;
  try {
    await owned.close();
  } catch (error) {
    cleanupErrors.push({ owner: label, ...publicError(error) });
  }
}

async function runLocalAcceptance(rawOptions = {}, dependencies = {}) {
  const deps = {
    loadManifest: dependencies.loadManifest || loadManifest,
    resolveArtifactInventory: dependencies.resolveArtifactInventory || resolveArtifactInventory,
    preflight: dependencies.preflight || system.preflight,
    createEvidenceRoot: dependencies.createEvidenceRoot || system.createEvidenceRoot,
    createDatabase: dependencies.createDatabase || system.createDatabase,
    probeDatabase: dependencies.probeDatabase || system.probeDatabase,
    createMockUpstream: dependencies.createMockUpstream || createMockUpstream,
    createGatewayFixture: dependencies.createGatewayFixture || createGatewayFixture,
    writeReadyManifest: dependencies.writeReadyManifest || system.writeReadyManifest,
    runWireAudit: dependencies.runWireAudit || runWireAudit,
    runGatewayCharacterize: dependencies.runGatewayCharacterize || runGatewayCharacterize,
    runLocalClientAcceptance: dependencies.runLocalClientAcceptance || runLocalClientAcceptance,
    writeProtocolEvidence: dependencies.writeProtocolEvidence || ((root, name, value) => {
      system.writeJson(path.join(root, name), value);
    }),
    writeResult: dependencies.writeResult || system.writeResult,
    writeSnapshot: dependencies.writeSnapshot || ((root, snapshot) => {
      system.writeJson(path.join(root, 'controlled-upstream-finally.json'), snapshot);
    }),
    cleanupTmux: dependencies.cleanupTmux || system.cleanupTmux,
  };
  let manifest;
  let evidenceRoot = null;
  let preflightEvidence = null;
  let database = null;
  let mock = null;
  let fixture = null;
  let readyFile = null;
  let clientResult = null;
  let protocol = null;
  let executionError = null;
  const cleanupErrors = [];

  try {
    manifest = deps.resolveArtifactInventory(deps.loadManifest(rawOptions.manifest));
    preflightEvidence = await deps.preflight(manifest);
    evidenceRoot = deps.createEvidenceRoot(manifest.repo.host.path);
    database = deps.createDatabase(manifest.database);
    await deps.probeDatabase(database.url, manifest);

    mock = deps.createMockUpstream({ barrierEnabled: true, barrierMarker: 'marker-1' });
    const endpoints = await mock.start();
    fixture = await deps.createGatewayFixture({
      databaseUrl: database.url,
      apiServerBin: manifest.artifacts.apiServer.path,
      pluginRunnerBin: manifest.artifacts.pluginRunner.path,
      openaiPackage: manifest.artifacts.openaiPackage.path,
      anthropicPackage: manifest.artifacts.anthropicPackage.path,
      upstreamBaseUrl: endpoints.httpBaseUrl,
      artifactRoot: evidenceRoot,
      apiPort: 7800,
    });
    readyFile = deps.writeReadyManifest(evidenceRoot, fixture.result);
    const secretCanary = 'sk-1flowbase-controlled-secret-canary';
    const wireAudit = await deps.runWireAudit({ manifest: wireAuditManifest(fixture.result) }, { secretCanary });
    deps.writeProtocolEvidence(evidenceRoot, 'wire-audit.json', wireAudit);
    const characterize = await deps.runGatewayCharacterize(characterizeOptions({
      repoRoot: evidenceRoot,
      ready: fixture.result,
      websocketBaseUrl: endpoints.websocketBaseUrl,
      mockSnapshot: mock.snapshot,
    }));
    if (characterize.summary.verdict !== 'PASS') {
      throw new Error(`gateway protocol characterize verdict was ${characterize.summary.verdict}`);
    }
    protocol = {
      status: 'pass',
      wire_audit: wireAudit,
      characterize: {
        verdict: characterize.summary.verdict,
        requests: characterize.summary.totals.requests,
        blocking_requests: characterize.summary.totals.blockingRequests,
        performance_requests: characterize.summary.totals.advisoryRequests,
        contract_failures: characterize.summary.totals.contractFailures,
        performance_and_observability_advisories: characterize.summary.totals.advisoryFailures,
        durable_convergence: characterize.summary.durableConvergence ?? null,
      },
    };
    clientResult = await deps.runLocalClientAcceptance({
      artifactRoot: path.join(evidenceRoot, 'clients'),
      surface: 'tmux',
      targets: targetsFromReady(fixture.result),
      mockSnapshot: async () => mock.snapshot(),
      releaseBarrier: async () => mock.releaseBarrier(),
      discovery: {
        binaries: {
          codex: manifest.artifacts.codex.path,
          claude: manifest.artifacts.claude.path,
          opencode: manifest.artifacts.opencode.path,
        },
        env: process.env,
      },
    });
    if (clientResult.status !== 'pass') {
      throw new Error(`local client acceptance status was ${clientResult.status}`);
    }
  } catch (error) {
    executionError = error;
  } finally {
    if (readyFile) fs.rmSync(readyFile, { force: true });
    if (evidenceRoot && mock?.snapshot) {
      try { deps.writeSnapshot(evidenceRoot, mock.snapshot()); } catch (error) {
        cleanupErrors.push({ owner: 'mock-evidence', ...publicError(error) });
      }
    }
    await closeOwned('fixture', fixture, cleanupErrors);
    if (mock?.stop) {
      try { await mock.stop(); } catch (error) { cleanupErrors.push({ owner: 'mock', ...publicError(error) }); }
    }
    await closeOwned('database', database, cleanupErrors);
    try { await deps.cleanupTmux(); } catch (error) { cleanupErrors.push({ owner: 'tmux', ...publicError(error) }); }
  }

  const finalError = executionError || (cleanupErrors[0]
    ? new Error(`${cleanupErrors[0].owner} cleanup failed: ${cleanupErrors[0].message}`)
    : null);
  const result = {
    schema_version: '1flowbase.local-ai-gateway-acceptance-result/v1',
    gate_role: 'mock_backed_local_client_acceptance',
    status: finalError ? 'fail' : 'pass',
    runtime_attempts: fixture || mock ? 1 : 0,
    database_attempts: database ? 1 : 0,
    preflight: preflightEvidence,
    protocol,
    clients: clientResult ? {
      status: clientResult.status,
      clients: clientResult.clients,
      final_reconciliation: clientResult.final_reconciliation,
    } : null,
    cleanup: { status: cleanupErrors.length ? 'fail' : 'pass', errors: cleanupErrors },
    error: finalError ? publicError(finalError) : null,
    evidence_root: evidenceRoot,
  };
  if (evidenceRoot) deps.writeResult(evidenceRoot, result);
  return result;
}

module.exports = { runLocalAcceptance };
