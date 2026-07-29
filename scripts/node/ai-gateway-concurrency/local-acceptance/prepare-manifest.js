#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');

const { loadManifest, sha256File } = require('./manifest');

function parseArgs(argv) {
  if (argv.length !== 4 || argv[0] !== '--source' || argv[2] !== '--output') {
    throw new Error('usage: prepare-manifest.js --source <path> --output <tmp/test-governance/path>');
  }
  return { source: argv[1], output: argv[3] };
}

function prepareManifest({ source, output }) {
  const manifest = loadManifest(source);
  const repoRoot = path.resolve(manifest.repo.host.path);
  const outputPath = path.resolve(output);
  const evidenceRoot = path.join(repoRoot, 'tmp/test-governance');
  const relativeOutput = path.relative(evidenceRoot, outputPath);
  if (!relativeOutput || relativeOutput.startsWith('..') || path.isAbsolute(relativeOutput)) {
    throw new Error('sealed local acceptance manifest must be written under tmp/test-governance');
  }

  const artifacts = { ...manifest.artifacts };
  for (const [name, relativePath] of [
    ['apiServer', 'api/target/release/api-server'],
    ['pluginRunner', 'api/target/release/plugin-runner'],
  ]) {
    const artifactPath = path.join(repoRoot, relativePath);
    if (!fs.existsSync(artifactPath) || !fs.statSync(artifactPath).isFile()) {
      throw new Error(`${name} build artifact is missing: ${artifactPath}`);
    }
    artifacts[name] = { path: artifactPath, sha256: sha256File(artifactPath) };
  }

  const sealed = { ...manifest, artifacts };
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(sealed, null, 2)}\n`, { mode: 0o600 });
  return sealed;
}

if (require.main === module) {
  try {
    const sealed = prepareManifest(parseArgs(process.argv.slice(2)));
    process.stdout.write(`${JSON.stringify({
      output: path.resolve(process.argv.at(-1)),
      api_server_sha256: sealed.artifacts.apiServer.sha256,
      plugin_runner_sha256: sealed.artifacts.pluginRunner.sha256,
    })}\n`);
  } catch (error) {
    process.stderr.write(`${error.stack || error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = { parseArgs, prepareManifest };
