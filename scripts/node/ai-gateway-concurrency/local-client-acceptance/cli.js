#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');
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
  return {
    artifactRoot: path.resolve(values['artifact-root']),
    surface: values.surface || 'auto',
    timeoutMs: values['timeout-ms'] ? Number(values['timeout-ms']) : undefined,
    targets: manifest.targets,
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
  if (result.status === 'fail') process.exitCode = 1;
}

if (require.main === module) {
  main().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}

module.exports = { loadOptions, parseArguments };
