'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { runGatewayCharacterize } = require('../characterize/engine');
const { runCliSmoke } = require('../cli-smoke');
const { createGatewayFixture } = require('../gateway-fixture');
const { createMockUpstream } = require('../mock-upstream');
const { runWireAudit } = require('../wire-audit/runner');
const { characterizeOptions, wireAuditManifest } = require('../workflow-contract/runner');
const { loadManifest } = require('./manifest');
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
    preflight: dependencies.preflight || system.preflight,
    createEvidenceRoot: dependencies.createEvidenceRoot || system.createEvidenceRoot,
    createDetachedSource: dependencies.createDetachedSource || system.createDetachedSource,
    createDatabase: dependencies.createDatabase || system.createDatabase,
    probeDatabase: dependencies.probeDatabase || system.probeDatabase,
    createMockUpstream: dependencies.createMockUpstream || createMockUpstream,
    createGatewayFixture: dependencies.createGatewayFixture || createGatewayFixture,
    writeReadyManifest: dependencies.writeReadyManifest || system.writeReadyManifest,
    runWireAudit: dependencies.runWireAudit || runWireAudit,
    runGatewayCharacterize: dependencies.runGatewayCharacterize || runGatewayCharacterize,
    runCliSmoke: dependencies.runCliSmoke || runCliSmoke,
    writeResult: dependencies.writeResult || system.writeResult,
    writeSnapshot: dependencies.writeSnapshot || ((root, snapshot) => {
      system.writeJson(path.join(root, 'controlled-upstream-finally.json'), snapshot);
    }),
    cleanupTmux: dependencies.cleanupTmux || system.cleanupTmux,
  };
  let manifest;
  let evidenceRoot = null;
  let preflightEvidence = null;
  let codexSource = null;
  let opencodeSource = null;
  let database = null;
  let mock = null;
  let fixture = null;
  let readyFile = null;
  let smoke = null;
  let clientDiagnosticError = null;
  let protocol = null;
  let executionError = null;
  const cleanupErrors = [];

  try {
    manifest = deps.loadManifest(rawOptions.manifest);
    preflightEvidence = await deps.preflight(manifest);
    evidenceRoot = deps.createEvidenceRoot(manifest.repo.host.path);
    codexSource = deps.createDetachedSource('codex', manifest.sources.codex, evidenceRoot);
    opencodeSource = deps.createDetachedSource('opencode', manifest.sources.opencode, evidenceRoot);
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
    });
    readyFile = deps.writeReadyManifest(evidenceRoot, fixture.result);
    const secretCanary = 'sk-1flowbase-controlled-secret-canary';
    const wireAudit = await deps.runWireAudit({ manifest: wireAuditManifest(fixture.result) }, { secretCanary });
    system.writeJson(path.join(evidenceRoot, 'wire-audit.json'), wireAudit);
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
        contract_failures: characterize.summary.totals.contractFailures,
        durable_convergence: characterize.summary.durableConvergence ?? null,
      },
    };
    try {
      smoke = await deps.runCliSmoke({
        readyManifest: readyFile,
        evidenceRoot: path.join(evidenceRoot, 'clients'),
        tmuxTiming: true,
        skipWireAudit: true,
        codexExecutable: manifest.artifacts.codex.path,
        codexSourceRoot: codexSource.path,
        codexSourceIdentity: manifest.sources.codex.identity,
        codexBuildCommand: manifest.clients.codex.buildCommand,
        claudeExecutable: manifest.artifacts.claude.path,
        claudePackageManifest: manifest.artifacts.claudeManifest.path,
        claudePackageName: manifest.clients.claude.packageName,
        claudePackageVersion: manifest.clients.claude.packageVersion,
        claudePackageIntegrity: manifest.clients.claude.packageIntegrity,
        claudeInstallCommand: manifest.clients.claude.installCommand,
        opencodeExecutable: manifest.artifacts.opencode.path,
        opencodeSourceRoot: opencodeSource.path,
        opencodeSourceIdentity: manifest.sources.opencode.identity,
        opencodeBuildCommand: manifest.clients.opencode.buildCommand,
      });
    } catch (error) {
      clientDiagnosticError = error;
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
    await closeOwned('opencode-source', opencodeSource, cleanupErrors);
    await closeOwned('codex-source', codexSource, cleanupErrors);
    try { await deps.cleanupTmux(); } catch (error) { cleanupErrors.push({ owner: 'tmux', ...publicError(error) }); }
  }

  const finalError = executionError || (cleanupErrors[0]
    ? new Error(`${cleanupErrors[0].owner} cleanup failed: ${cleanupErrors[0].message}`)
    : null);
  const result = {
    schema_version: '1flowbase.local-ai-gateway-acceptance-result/v1',
    gate_role: 'non_blocking_client_diagnostic',
    status: finalError ? 'fail' : 'pass',
    runtime_attempts: fixture || mock ? 1 : 0,
    database_attempts: database ? 1 : 0,
    preflight: preflightEvidence,
    protocol,
    clients: smoke
      ? { status: smoke.status, event_counts: smoke.event_counts }
      : clientDiagnosticError ? { status: 'fail', error: publicError(clientDiagnosticError) } : null,
    cleanup: { status: cleanupErrors.length ? 'fail' : 'pass', errors: cleanupErrors },
    error: finalError ? publicError(finalError) : null,
    evidence_root: evidenceRoot,
  };
  if (evidenceRoot) deps.writeResult(evidenceRoot, result);
  return result;
}

module.exports = { runLocalAcceptance };
