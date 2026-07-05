#!/usr/bin/env node

const path = require('node:path');

const { main: runCapacityReport } = require('../capacity-report/core.js');
const { main: runCheckStyleBoundary } = require('../check-style-boundary/core.js');
const { main: runCheckRustBackend } = require('../check-rust-backend/core.js');
const { main: runGateRouter } = require('../gate-router/core.js');
const { main: runGrowthTableReport } = require('../growth-table-report/core.js');
const { main: runHotspotReview } = require('../hotspot-review/core.js');
const { main: runI18nHygiene } = require('../i18n-hygiene/core.js');
const { main: runLogQueryContractReport } = require('../log-query-contract-report/core.js');
const { main: runRepoHygiene } = require('../repo-hygiene/core.js');
const {
  main: runConsoleRouteRegistryHygiene,
} = require('../console-route-registry-hygiene/core.js');
const { main: runSchemaHygiene } = require('../schema-hygiene/core.js');
const { main: runSecurityRisk } = require('../security-risk/core.js');
const { main: runViteLazyDepsGate } = require('../vite-lazy-deps-gate/core.js');
const { main: runPageDebug } = require('../page-debug/core.js');
const { main: runApiDebug } = require('../api-debug/core.js');
const { main: runMockUiSync } = require('../mock-ui-sync/core.js');
const { main: runRawJsonbReport } = require('../raw-jsonb-report/core.js');
const { main: runClaudeSkillSync } = require('../claude-skill-sync/core.js');
const {
  getRepoRoot,
  runCommandSequence,
} = require('../testing/warning-capture.js');
const { resolveNodeBinaryFromPath } = require('../testing/node-runtime.js');

const TOOLING_COMMANDS = new Set([
  'check-style-boundary',
  'api-debug',
  'capacity-report',
  'check-rust-backend',
  'claude-skill-sync',
  'console-route-registry-hygiene',
  'gate-router',
  'growth-table-report',
  'hotspot-review',
  'i18n-hygiene',
  'log-query-contract-report',
  'mock-ui-sync',
  'page-debug',
  'raw-jsonb-report',
  'repo-hygiene',
  'runtime-gate',
  'schema-hygiene',
  'security-risk',
  'vite-lazy-deps-gate',
]);

function resolveScriptsNodeEntry(repoRoot, entryName) {
  return path.join(repoRoot, 'scripts', 'node', entryName);
}

function resolveScriptsNodeCliEntry(repoRoot, entryName) {
  return `${resolveScriptsNodeEntry(repoRoot, entryName)}.js`;
}

function buildRuntimeGateCommand({ argv, repoRoot, env = process.env }) {
  return {
    label: 'runtime-page-debug',
    command: resolveNodeBinaryFromPath(env),
    args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'page-debug', ...argv],
    cwd: repoRoot,
  };
}

function usageRuntimeGate(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout('Usage: node scripts/node/cli/runtime-gate.js <page-debug args>\n');
}

function runRuntimeGate(argv = [], deps = {}) {
  if (argv.length === 0 || argv.includes('-h') || argv.includes('--help')) {
    usageRuntimeGate(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();

  return runCommandSequence({
    repoRoot,
    env: deps.env || process.env,
    scope: 'runtime-gate',
    commands: [buildRuntimeGateCommand({ argv, repoRoot, env: deps.env || process.env })],
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

function parseToolingCliArgs(argv) {
  if (argv.length === 0 || argv[0] === '-h' || argv[0] === '--help') {
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
    'Usage: node scripts/node/tooling <api-debug|capacity-report|check-rust-backend|check-style-boundary|claude-skill-sync|console-route-registry-hygiene|gate-router|growth-table-report|hotspot-review|i18n-hygiene|log-query-contract-report|mock-ui-sync|page-debug|raw-jsonb-report|repo-hygiene|runtime-gate|schema-hygiene|security-risk|vite-lazy-deps-gate> [args]\n'
  );
}

async function main(argv = [], deps = {}) {
  const options = parseToolingCliArgs(argv);

  if (options.help) {
    usage(deps.writeStdout);
    return 0;
  }

  if (!TOOLING_COMMANDS.has(options.command)) {
    throw new Error(`Unknown tooling command: ${options.command}`);
  }

  if (options.command === 'check-style-boundary') {
    return (deps.runCheckStyleBoundaryImpl || runCheckStyleBoundary)(options.rest);
  }

  if (options.command === 'api-debug') {
    return (deps.runApiDebugImpl || runApiDebug)(options.rest);
  }

  if (options.command === 'capacity-report') {
    return (deps.runCapacityReportImpl || runCapacityReport)(options.rest, deps);
  }

  if (options.command === 'check-rust-backend') {
    return (deps.runCheckRustBackendImpl || runCheckRustBackend)(options.rest, deps);
  }

  if (options.command === 'claude-skill-sync') {
    return (deps.runClaudeSkillSyncImpl || runClaudeSkillSync)(options.rest);
  }

  if (options.command === 'console-route-registry-hygiene') {
    return (deps.runConsoleRouteRegistryHygieneImpl || runConsoleRouteRegistryHygiene)(
      options.rest,
      deps
    );
  }

  if (options.command === 'gate-router') {
    return (deps.runGateRouterImpl || runGateRouter)(options.rest, deps);
  }

  if (options.command === 'growth-table-report') {
    return (deps.runGrowthTableReportImpl || runGrowthTableReport)(options.rest, deps);
  }

  if (options.command === 'hotspot-review') {
    return (deps.runHotspotReviewImpl || runHotspotReview)(options.rest, deps);
  }

  if (options.command === 'i18n-hygiene') {
    return (deps.runI18nHygieneImpl || runI18nHygiene)(options.rest, deps);
  }

  if (options.command === 'log-query-contract-report') {
    return (deps.runLogQueryContractReportImpl || runLogQueryContractReport)(options.rest, deps);
  }

  if (options.command === 'mock-ui-sync') {
    return (deps.runMockUiSyncImpl || runMockUiSync)(options.rest);
  }

  if (options.command === 'page-debug') {
    return (deps.runPageDebugImpl || runPageDebug)(options.rest);
  }

  if (options.command === 'raw-jsonb-report') {
    return (deps.runRawJsonbReportImpl || runRawJsonbReport)(options.rest, deps);
  }

  if (options.command === 'repo-hygiene') {
    return (deps.runRepoHygieneImpl || runRepoHygiene)(options.rest, deps);
  }

  if (options.command === 'schema-hygiene') {
    return (deps.runSchemaHygieneImpl || runSchemaHygiene)(options.rest, deps);
  }

  if (options.command === 'security-risk') {
    return (deps.runSecurityRiskImpl || runSecurityRisk)(options.rest, deps);
  }

  if (options.command === 'vite-lazy-deps-gate') {
    return (deps.runViteLazyDepsGateImpl || runViteLazyDepsGate)(options.rest, deps);
  }

  return (deps.runRuntimeGateImpl || runRuntimeGate)(options.rest, deps);
}

module.exports = {
  buildRuntimeGateCommand,
  main,
  parseToolingCliArgs,
  runRuntimeGate,
};
