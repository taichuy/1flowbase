#!/usr/bin/env node

const { BACKEND_CONSISTENCY_GROUPS, BACKEND_CONSISTENCY_TARGETS, BACKEND_CI_TEST_SHARDS, BACKEND_SHARDS, BACKEND_TEST_SHARDS, IMAGE_LLM_VISION_GATE_TARGETS } = require('./backend-targets.js');
const { buildBackendConsistencyCommands, runBackendConsistencyCommandSequence, runBackendConsistency } = require('./backend-consistency.js');
const { buildStateProtocolCommands, parseStateProtocolsCliArgs, runStateProtocols } = require('./state-protocols.js');
const { main: runConsoleOperationRegistryHygiene } = require('../console-operation-registry-hygiene/cli.js');
const { buildImageLlmVisionGateCommands, buildBackendCommands, parseBackendCliArgs, runBackend } = require("./backend.js");
const { buildCiCommands, runCi } = require("./ci.js");
const { parseCoverageCliArgs, buildCoverageFrontendCommand, buildCoverageFrontendPageRuntimeCommand, buildCoverageBackendCommands, buildCoverageBackendCleanupCommands, ensureCargoLlvmCovInstalled, runCoverage } = require("./coverage.js");
const { collectFrontendCoverageFailures, collectBackendCoverageFailures } = require("./coverage-report.js");
const { parseRepoCliArgs, buildRepoCommands, runRepo } = require("./repo.js");

const VERIFY_COMMANDS = new Set([
  'backend',
  'backend-consistency',
  'ci',
  'console-operation-registry-hygiene',
  'coverage',
  'repo',
  'state-protocols',
]);

function parseVerifyCliArgs(argv) {
  if (argv.includes('-h') || argv.includes('--help') || argv.length === 0) {
    return {
      help: true,
      command: null,
      rest: [],
    };
  }

  const [command, ...rest] = argv;
  return {
    help: false,
    command,
    rest,
  };
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/verify '
      + '<backend|backend-consistency|ci|console-operation-registry-hygiene|coverage|repo|state-protocols> [args]\n'
  );
}

async function main(argv = [], deps = {}) {
  const options = parseVerifyCliArgs(argv);

  if (options.help) {
    usage(deps.writeStdout);
    return 0;
  }

  if (!VERIFY_COMMANDS.has(options.command)) {
    throw new Error(`Unknown verify command: ${options.command}`);
  }

  if (options.command === 'backend') {
    return (deps.runBackendImpl || runBackend)(options.rest, deps);
  }

  if (options.command === 'backend-consistency') {
    return (deps.runBackendConsistencyImpl || runBackendConsistency)(options.rest, deps);
  }

  if (options.command === 'ci') {
    return (deps.runCiImpl || runCi)(options.rest, deps);
  }

  if (options.command === 'console-operation-registry-hygiene') {
    return (deps.runConsoleOperationRegistryHygieneImpl || runConsoleOperationRegistryHygiene)(
      options.rest,
      deps
    );
  }

  if (options.command === 'coverage') {
    return (deps.runCoverageImpl || runCoverage)(options.rest, deps);
  }

  if (options.command === 'state-protocols') {
    return (deps.runStateProtocolsImpl || runStateProtocols)(options.rest, deps);
  }

  return (deps.runRepoImpl || runRepo)(options.rest, deps);
}

module.exports = {
  BACKEND_CONSISTENCY_GROUPS,
  BACKEND_CONSISTENCY_TARGETS,
  BACKEND_CI_TEST_SHARDS,
  BACKEND_SHARDS,
  BACKEND_TEST_SHARDS,
  buildBackendCommands,
  buildBackendConsistencyCommands,
  buildImageLlmVisionGateCommands,
  runBackendConsistencyCommandSequence,
  buildCiCommands,
  buildCoverageBackendCleanupCommands,
  buildCoverageBackendCommands,
  buildCoverageFrontendCommand,
  buildCoverageFrontendPageRuntimeCommand,
  buildRepoCommands,
  buildStateProtocolCommands,
  collectBackendCoverageFailures,
  collectFrontendCoverageFailures,
  ensureCargoLlvmCovInstalled,
  IMAGE_LLM_VISION_GATE_TARGETS,
  main,
  parseBackendCliArgs,
  parseCoverageCliArgs,
  parseRepoCliArgs,
  parseStateProtocolsCliArgs,
  parseVerifyCliArgs,
  runBackend,
  runBackendConsistency,
  runCi,
  runCoverage,
  runRepo,
  runStateProtocols,
};
