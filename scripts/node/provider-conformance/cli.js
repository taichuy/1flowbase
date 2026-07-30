#!/usr/bin/env node
'use strict';

const path = require('node:path');

const { ConformanceError, runConformance } = require('./core.js');

function usage() {
  return `Usage: node scripts/node/provider-conformance/cli.js \\
  --main-root <path> --official-root <path> \\
  --main-sha <full-sha> --official-sha <full-sha> \\
  --package-dir <directory> --plugin-runner-bin <file> \\
  --fixture <matrix.json> --artifact <paired-sha.json> \\
  [--expected-pair-artifact <paired-sha.json>]

Runs the actual .1flowbasepkg provider artifacts through the actual plugin-runner
against a loopback fake upstream. The runner refuses dirty or SHA-mismatched source
pairs and writes the sole paired-SHA provenance artifact only after all six source,
package, installed-manifest, runtime-identity, and wire cases pass.
`;
}

function parseArgs(argv) {
  const options = {
    mainRoot: null,
    officialRoot: null,
    mainSha: null,
    officialSha: null,
    packageDir: null,
    pluginRunnerBin: null,
    fixture: null,
    artifact: null,
    expectedPairArtifact: null,
  };
  const names = new Map([
    ['--main-root', 'mainRoot'],
    ['--official-root', 'officialRoot'],
    ['--main-sha', 'mainSha'],
    ['--official-sha', 'officialSha'],
    ['--package-dir', 'packageDir'],
    ['--plugin-runner-bin', 'pluginRunnerBin'],
    ['--fixture', 'fixture'],
    ['--artifact', 'artifact'],
    ['--expected-pair-artifact', 'expectedPairArtifact'],
  ]);

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--help' || arg === '-h') {
      return { help: true };
    }
    const field = names.get(arg);
    if (!field || !argv[index + 1]) {
      throw new ConformanceError(`invalid argument: ${arg}`);
    }
    options[field] = argv[index + 1];
    index += 1;
  }

  for (const [field, value] of Object.entries(options)) {
    if (!value && field !== 'expectedPairArtifact') {
      throw new ConformanceError(`missing required argument: --${field.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`);
    }
  }
  for (const field of ['mainRoot', 'officialRoot', 'packageDir', 'pluginRunnerBin', 'fixture', 'artifact', 'expectedPairArtifact']) {
    if (options[field]) {
      options[field] = path.resolve(options[field]);
    }
  }
  return options;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    process.stdout.write(usage());
    return;
  }
  const result = await runConformance(options);
  process.stdout.write(
    `[provider-conformance] paired SHA artifact written: ${options.artifact} (${result.package_count} packages)\n`
  );
}

main().catch((error) => {
  const message = error instanceof ConformanceError ? error.message : 'provider conformance failed';
  process.stderr.write(`[provider-conformance] ${message}\n`);
  process.exitCode = 1;
});
