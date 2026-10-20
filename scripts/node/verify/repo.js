const { getRepoRoot, runManagedCommandSequence } = require('../testing/warning-capture.js');
const { loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');
const { resolveNodeBinaryFromPath } = require('../testing/node-runtime.js');
const { resolveScriptsNodeCliEntry } = require("./command-paths.js");

const VALID_REPO_TARGETS = new Set(['tooling', 'frontend', 'frontend-pr', 'backend', 'all']);

function parseRepoCliArgs(argv) {
  if (argv.includes('-h') || argv.includes('--help')) {
    return { help: true, target: 'all' };
  }

  const [target = 'all'] = argv;

  if (!VALID_REPO_TARGETS.has(target)) {
    throw new Error(`Unknown repo target: ${target}`);
  }

  return { help: false, target };
}

function buildRepoCommands({ repoRoot, env = process.env, target = 'all' }) {
  const nodeBinary = resolveNodeBinaryFromPath(env);

  const toolingCommands = [
    {
      label: 'repo-gate-router',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'gate-router'],
      cwd: repoRoot,
    },
    {
      label: 'repo-hygiene',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'repo-hygiene'],
      cwd: repoRoot,
    },
    {
      label: 'repo-i18n-hygiene',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'i18n-hygiene'],
      cwd: repoRoot,
    },
    {
      label: 'repo-schema-hygiene',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'schema-hygiene'],
      cwd: repoRoot,
    },
    {
      label: 'repo-frontstage-governance-hygiene',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'frontstage-governance-hygiene'],
      cwd: repoRoot,
    },
    {
      label: 'repo-raw-jsonb-report',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'raw-jsonb-report'],
      cwd: repoRoot,
    },
    {
      label: 'repo-log-query-contract-report',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'log-query-contract-report'],
      cwd: repoRoot,
    },
    {
      label: 'repo-capacity-report',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'capacity-report'],
      cwd: repoRoot,
    },
    {
      label: 'repo-security-risk',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'security-risk'],
      cwd: repoRoot,
    },
    {
      label: 'repo-script-tests',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'test'), 'scripts'],
      cwd: repoRoot,
    },
    {
      label: 'repo-contract-tests',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'test'), 'contracts'],
      cwd: repoRoot,
    },
  ];
  const viteLazyDepsCommand = {
    label: 'repo-vite-lazy-deps-gate',
    command: nodeBinary,
    args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'vite-lazy-deps-gate'],
    cwd: repoRoot,
  };
  const devExperienceCommand = {
    label: 'repo-dev-experience',
    command: nodeBinary,
    args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'dev-experience'],
    cwd: repoRoot,
  };
  const frontendCommands = [
    devExperienceCommand,
    viteLazyDepsCommand,
    {
      label: 'repo-frontend-full',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'test'), 'frontend', 'full'],
      cwd: repoRoot,
    },
    {
      label: 'repo-frontend-page-regression',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'test'), 'frontend', 'page-regression'],
      cwd: repoRoot,
    },
  ];
  const frontendPrCommands = [
    devExperienceCommand,
    viteLazyDepsCommand,
    {
      label: 'repo-frontend-pr',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'test'), 'frontend', 'pr'],
      cwd: repoRoot,
    },
  ];
  const backendCommands = [
    {
      label: 'repo-backend-full',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'verify'), 'backend'],
      cwd: repoRoot,
    },
  ];

  if (target === 'tooling') {
    return toolingCommands;
  }

  if (target === 'frontend') {
    return frontendCommands;
  }

  if (target === 'frontend-pr') {
    return frontendPrCommands;
  }

  if (target === 'backend') {
    return backendCommands;
  }

  return [
    ...toolingCommands,
    ...frontendCommands,
    ...backendCommands,
  ];
}

function usageRepo(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/verify-repo.js [tooling|frontend|frontend-pr|backend|all]\n'
      + 'Runs repository gates, optionally restricted to a CI-friendly slice.\n'
  );
}

async function runRepo(argv = [], deps = {}) {
  const options = parseRepoCliArgs(argv);

  if (options.help) {
    usageRepo(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig = deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;

  return managedRunner({
    repoRoot,
    env,
    scope: options.target === 'all' ? 'verify-repo' : `verify-repo-${options.target}`,
    lockMode: 'heavy',
    commandDisplay: options.target === 'all'
      ? 'node scripts/node/verify-repo.js'
      : `node scripts/node/verify-repo.js ${options.target}`,
    runtimeConfig,
    commands: buildRepoCommands({ repoRoot, env, target: options.target }),
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

module.exports = { parseRepoCliArgs, buildRepoCommands, runRepo };
