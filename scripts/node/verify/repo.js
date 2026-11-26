const { getRepoRoot, runManagedCommandSequence } = require('../testing/warning-capture.js');
const { loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');
const { resolveNodeBinaryFromPath } = require('../testing/node-runtime.js');
const { buildCiCommands } = require('./ci.js');
const { buildPrVerifyPlan, readPrVerifyPlan } = require('../gate-router/core.js');
const { resolveScriptsNodeCliEntry } = require("./command-paths.js");

const VALID_REPO_TARGETS = new Set(['tooling', 'frontend', 'frontend-pr', 'backend', 'all', 'level1', 'level2', 'level3']);

function parseRepoCliArgs(argv) {
  if (argv.includes('-h') || argv.includes('--help')) {
    return { help: true, target: 'all' };
  }

  const [target = 'all', ...args] = argv;

  if (!VALID_REPO_TARGETS.has(target)) {
    throw new Error(`Unknown repo target: ${target}`);
  }

  if (target === 'level2' && args.length === 2 && args[0] === '--base-ref' && args[1].trim()) {
    return { help: false, target, baseRef: args[1] };
  }
  if (args.length) throw new Error(`Unexpected verify-repo arguments: ${args.join(' ')}`);
  return { help: false, target };
}

function buildRepoCommands({ repoRoot, env = process.env, target = 'all', plan, baseRef }) {
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
      label: 'repo-data-contracts',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'data-contracts'],
      cwd: repoRoot,
    },
    {
      label: 'repo-frontstage-governance-hygiene',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'frontstage-governance-hygiene'],
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

  const nodeCommand = (label, entry, ...args) => ({ label, command: nodeBinary,
    args: [resolveScriptsNodeCliEntry(repoRoot, entry), ...args], cwd: repoRoot });

  if (target === 'level1') {
    return [
      ...toolingCommands.slice(0, 3),
      viteLazyDepsCommand,
      nodeCommand('repo-backend-static', 'tooling', 'check-rust-backend'),
      nodeCommand('repo-backend-fmt', 'verify', 'backend', 'fmt'),
      nodeCommand('repo-fast-script-tests', 'test', 'scripts', 'gate-router', 'testing/resources'),
    ];
  }
  if (target === 'level2') {
    const impact = plan ?? buildPrVerifyPlan();
    const foundation = impact.allFoundations ? 'all' : 'auto';
    const packs = impact.selectedFoundations ?? [];
    const foundationArgs = ['--candidate-sha', 'HEAD'];
    return [
      ...(impact.tooling ? toolingCommands : []),
      ...(impact.frontend ? frontendPrCommands : []),
      ...impact.backendScopes.map((scope) => {
        const targetName = scope.slice('repo-backend-'.length);
        const args = targetName.startsWith('check-') ? ['check', targetName.slice(6)] : [targetName];
        return nodeCommand(scope, 'verify', 'backend', ...args);
      }),
      ...(packs.length ? [nodeCommand('foundation-contract-plan', 'tooling', 'foundation-contracts', 'plan',
        '--base-ref', /^(?!0+$)[0-9a-f]{40}$/u.test(baseRef ?? '') ? baseRef : 'HEAD',
        '--event', 'pull_request', '--foundation', foundation, ...foundationArgs)] : []),
      ...packs.map((pack) => nodeCommand(`foundation-contract-${pack}-fast`, 'tooling', 'foundation-contracts',
        'run-fast', '--plan', 'tmp/test-governance/foundation-contracts/foundation-contract-plan.json',
        '--foundation', pack, ...foundationArgs)),
    ];
  }
  if (target === 'level3') return buildCiCommands({ repoRoot, env });

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
    'Usage: node scripts/node/verify-repo.js [tooling|frontend|frontend-pr|backend|all|level1|level2 [--base-ref <sha>]|level3]\n'
      + 'level1: fast development checks; level2: impact-routed PR gates; level3: full local CI and coverage.\n'
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

  const plan = options.target === 'level2'
    ? readPrVerifyPlan({ repoRoot, env, baseRef: options.baseRef, spawnSyncImpl: deps.spawnSyncImpl })
    : undefined;
  const commands = buildRepoCommands({ repoRoot, env, target: options.target, plan, baseRef: options.baseRef });
  if (!commands.length) return 0;
  return managedRunner({
    repoRoot,
    env,
    scope: options.target === 'all' ? 'verify-repo' : `verify-repo-${options.target}`,
    lockMode: 'heavy',
    commandDisplay: options.target === 'all'
      ? 'node scripts/node/verify-repo.js'
      : ['node scripts/node/verify-repo.js', ...argv].join(' '),
    runtimeConfig,
    commands,
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

module.exports = { parseRepoCliArgs, buildRepoCommands, runRepo };
