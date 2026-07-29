#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { targetMatrixFromReady, targetsFromReady } = require('./contract');
const { runLocalClientAcceptance } = require('./driver');

function parseArguments(argv) {
  const values = {};
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith('--') || value === undefined) throw new Error('arguments must use --name value pairs');
    values[key.slice(2)] = value;
  }
  for (const required of ['manifest', 'artifact-root']) {
    if (!values[required]) throw new Error(`--${required} is required`);
  }
  return values;
}

function loadOptions(values) {
  const manifestPath = path.resolve(values.manifest);
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const snapshotUrl = manifest.controlled_upstream?.snapshot_url;
  const barrierUrl = manifest.controlled_upstream?.barrier_release_url;
  if (!snapshotUrl) throw new Error('Gateway fixture manifest omitted controlled upstream snapshot URL');
  if (!barrierUrl) throw new Error('Gateway fixture manifest omitted controlled upstream barrier URL');
  return {
    artifactRoot: path.resolve(values['artifact-root']),
    surface: values.surface || 'auto',
    timeoutMs: values['timeout-ms'] ? Number(values['timeout-ms']) : undefined,
    targets: targetsFromReady(manifest),
    targetMatrix: targetMatrixFromReady(manifest),
    requireCrossTargetMatrix: true,
    async mockSnapshot() {
      const response = await fetch(snapshotUrl);
      if (!response.ok) throw new Error(`controlled upstream snapshot returned HTTP ${response.status}`);
      return response.json();
    },
    async releaseBarrier() {
      const response = await fetch(barrierUrl, { method: 'POST' });
      if (!response.ok) throw new Error(`controlled upstream barrier returned HTTP ${response.status}`);
    },
    discovery: {
      binaries: manifest.binaries,
      configs: manifest.configs,
      env: process.env,
    },
  };
}

async function main() {
  const result = await runLocalClientAcceptance(loadOptions(parseArguments(process.argv.slice(2))));
  process.stdout.write(`${JSON.stringify({
    status: result.status,
    artifact_path: result.artifact_path,
    gate_role: result.gate_role,
  })}\n`);
  if (result.status !== 'pass') process.exitCode = 1;
}

if (require.main === module) {
  main().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}

module.exports = { loadOptions, parseArguments };
