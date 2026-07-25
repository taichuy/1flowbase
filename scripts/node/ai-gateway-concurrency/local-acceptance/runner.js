'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { runClientCompatibilityCommand } = require('../client-compatibility');
const { createGatewayFixture } = require('../gateway-fixture');
const { createMockUpstream } = require('../mock-upstream');
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
    createDatabase: dependencies.createDatabase || system.createDatabase,
    probeDatabase: dependencies.probeDatabase || system.probeDatabase,
    createMockUpstream: dependencies.createMockUpstream || createMockUpstream,
    createGatewayFixture: dependencies.createGatewayFixture || createGatewayFixture,
    writeReadyManifest: dependencies.writeReadyManifest || system.writeReadyManifest,
    runClientCompatibility: dependencies.runClientCompatibility || runClientCompatibilityCommand,
    writeResult: dependencies.writeResult || system.writeResult,
    writeSnapshot: dependencies.writeSnapshot || ((root, snapshot) => {
      system.writeJson(path.join(root, 'controlled-upstream-finally.json'), snapshot);
    }),
  };
  let manifest;
  let evidenceRoot = null;
  let preflightEvidence = null;
  let database = null;
  let mock = null;
  let fixture = null;
  let readyFile = null;
  let smoke = null;
  let executionError = null;
  const cleanupErrors = [];

  try {
    manifest = deps.loadManifest(rawOptions.manifest);
    preflightEvidence = await deps.preflight(manifest);
    evidenceRoot = deps.createEvidenceRoot(manifest.repo.host.path);
    database = deps.createDatabase(manifest.database);
    await deps.probeDatabase(database.url, manifest);

    mock = deps.createMockUpstream({ barrierEnabled: true });
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
    smoke = await deps.runClientCompatibility({
      readyManifest: readyFile,
      evidenceRoot: path.join(evidenceRoot, 'clients'),
      runtimeRoot: path.join(manifest.repo.host.path, 'scripts/node/ai-gateway-concurrency/client-compatibility/runtime'),
    });
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
  }

  const finalError = executionError || (cleanupErrors[0]
    ? new Error(`${cleanupErrors[0].owner} cleanup failed: ${cleanupErrors[0].message}`)
    : null);
  const result = {
    schema_version: '1flowbase.local-ai-gateway-acceptance-result/v1',
    status: finalError ? 'fail' : 'pass',
    runtime_attempts: fixture || mock ? 1 : 0,
    database_attempts: database ? 1 : 0,
    preflight: preflightEvidence,
    clients: smoke ? { status: smoke.status, clients: smoke.clients } : null,
    cleanup: { status: cleanupErrors.length ? 'fail' : 'pass', errors: cleanupErrors },
    error: finalError ? publicError(finalError) : null,
    evidence_root: evidenceRoot,
  };
  if (evidenceRoot) deps.writeResult(evidenceRoot, result);
  return result;
}

module.exports = { runLocalAcceptance };
