#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');

const { createGatewayFixture } = require('../ai-gateway-concurrency/gateway-fixture');

const OPTION_FIELDS = new Map([
  ['--database-url', 'databaseUrl'],
  ['--api-server-bin', 'apiServerBin'],
  ['--plugin-runner-bin', 'pluginRunnerBin'],
  ['--openai-package', 'openaiPackage'],
  ['--anthropic-package', 'anthropicPackage'],
  ['--openai-compatible-package', 'openaiCompatiblePackage'],
  ['--upstream-base-url', 'upstreamBaseUrl'],
  ['--ready-file', 'readyFile'],
]);

function usage() {
  return `Usage: node scripts/node/cli/ai-gateway-fixture.js \\
  --database-url <temporary-postgres-url> \\
  --api-server-bin <path> --plugin-runner-bin <path> \\
  --openai-package <archive> --anthropic-package <archive> \\
  --openai-compatible-package <archive> \\
  --upstream-base-url <loopback-url> [--ready-file <json>]

The required values may instead be supplied as AI_GATEWAY_FIXTURE_DATABASE_URL,
AI_GATEWAY_FIXTURE_API_SERVER_BIN, AI_GATEWAY_FIXTURE_PLUGIN_RUNNER_BIN,
AI_GATEWAY_FIXTURE_OPENAI_PACKAGE, AI_GATEWAY_FIXTURE_ANTHROPIC_PACKAGE,
AI_GATEWAY_FIXTURE_OPENAI_COMPATIBLE_PACKAGE, and
AI_GATEWAY_FIXTURE_UPSTREAM_BASE_URL. The process owns the real gateway stack until
SIGINT or SIGTERM and then removes only its own processes and temporary files.`;
}

function parseArgs(argv, env = process.env) {
  const options = {
    databaseUrl: env.AI_GATEWAY_FIXTURE_DATABASE_URL,
    apiServerBin: env.AI_GATEWAY_FIXTURE_API_SERVER_BIN,
    pluginRunnerBin: env.AI_GATEWAY_FIXTURE_PLUGIN_RUNNER_BIN,
    openaiPackage: env.AI_GATEWAY_FIXTURE_OPENAI_PACKAGE,
    anthropicPackage: env.AI_GATEWAY_FIXTURE_ANTHROPIC_PACKAGE,
    openaiCompatiblePackage: env.AI_GATEWAY_FIXTURE_OPENAI_COMPATIBLE_PACKAGE,
    upstreamBaseUrl: env.AI_GATEWAY_FIXTURE_UPSTREAM_BASE_URL,
    readyFile: env.AI_GATEWAY_FIXTURE_READY_FILE,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    if (flag === '--help' || flag === '-h') return { help: true };
    const field = OPTION_FIELDS.get(flag);
    if (!field) throw new Error(`unknown option ${flag}`);
    const value = argv[index + 1];
    if (!value || value.startsWith('--')) throw new Error(`${flag} requires a value`);
    options[field] = value;
    index += 1;
  }
  return options;
}

function writeReadyFile(filePath, value) {
  const resolved = path.resolve(filePath);
  const temporary = `${resolved}.${process.pid}.tmp`;
  try {
    fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
    fs.linkSync(temporary, resolved);
  } finally {
    fs.rmSync(temporary, { force: true });
  }
  return resolved;
}

function waitForStop() {
  return new Promise((resolve) => {
    process.once('SIGINT', resolve);
    process.once('SIGTERM', resolve);
  });
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    process.stdout.write(`${usage()}\n`);
    return;
  }
  let fixture = null;
  let readyFile = null;
  try {
    fixture = await createGatewayFixture(options);
    if (options.readyFile) readyFile = writeReadyFile(options.readyFile, fixture.result);
    process.stdout.write(`${JSON.stringify(fixture.result)}\n`);
    await waitForStop();
  } finally {
    await fixture?.close();
    if (readyFile) fs.rmSync(readyFile, { force: true });
  }
}

if (require.main === module) {
  main().catch((error) => {
    process.stderr.write(`[ai-gateway-fixture] ${error.message}\n`);
    process.exitCode = 1;
  });
}

module.exports = { parseArgs, usage, writeReadyFile };
