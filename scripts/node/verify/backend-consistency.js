const fs = require('node:fs');
const path = require('node:path');
const { buildCargoCommandEnv, getRepoRoot, resolveOutputDir, runCommandSequence,
  runManagedCommandSequence } = require('../testing/warning-capture.js');
const { loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');
const { BACKEND_CONSISTENCY_GROUPS, BACKEND_CONSISTENCY_TARGETS } = require('./backend-targets.js');
const { parseCargoTestCounts } = require('./cargo-test-results.js');
const BACKEND_CONSISTENCY_TARGET_REPORT_FILE = 'backend-consistency-targets.json';

function buildBackendConsistencyTargetResult(command) {
  const target = BACKEND_CONSISTENCY_TARGETS.find((candidate) => candidate.label === command.label);

  return {
    label: command.label,
    packageName: target?.packageName || command.args?.[2] || '',
    filter: target?.filter || command.args?.[5] || '',
    testTarget: target?.testTarget || '',
    status: 'skipped',
    exitCode: null,
    durationMs: null,
    passedCount: null,
    failedCount: null,
  };
}

function writeBackendConsistencyTargetReport({ repoRoot, env, targets }) {
  const outputDir = resolveOutputDir(repoRoot, env);
  fs.mkdirSync(outputDir, { recursive: true });
  fs.writeFileSync(
    path.join(outputDir, BACKEND_CONSISTENCY_TARGET_REPORT_FILE),
    `${JSON.stringify({ targets }, null, 2)}\n`,
    'utf8'
  );
}

function buildBackendConsistencyCommands({
  cargoJobs,
  cargoTestThreads,
  incremental = false,
  group = null,
}) {
  return BACKEND_CONSISTENCY_TARGETS
    .filter((target) => group === null || target.group === group)
    .map((target) => ({
      label: target.label,
      command: 'cargo',
      args: [
        'test',
        '-p',
        target.packageName,
        '--jobs',
        String(cargoJobs),
        target.filter,
        ...(target.testTarget ? ['--test', target.testTarget] : []),
        '--',
        `--test-threads=${cargoTestThreads}`,
      ],
      cwd: 'api',
      env: buildCargoCommandEnv({ cargoParallelism: cargoJobs, incremental }),
    }));
}

function runBackendConsistencyCommandSequence(sequenceOptions) {
  const targets = sequenceOptions.commands.map(buildBackendConsistencyTargetResult);
  const targetByLabel = new Map(targets.map((target) => [target.label, target]));

  const status = runCommandSequence({
    ...sequenceOptions,
    onCommandComplete({ command, result, startedAtMs, finishedAtMs }) {
      const target = targetByLabel.get(command.label);

      if (!target) {
        return;
      }

      const counts = parseCargoTestCounts(`${result.stdout || ''}\n${result.stderr || ''}`);
      const executed = counts.passedCount > 0 && counts.failedCount === 0;
      target.status = result.status === 0 && executed ? 'passed' : 'failed';
      target.exitCode = result.status === 0 && !executed ? 1 : (result.status ?? 1);
      if (result.status === 0 && !executed) {
        (sequenceOptions.writeStderr || process.stderr.write.bind(process.stderr))(
          `${command.label}: no executed passing tests; refusing empty consistency gate\n`,
        );
      }
      target.durationMs = Math.max(0, finishedAtMs - startedAtMs);
      target.passedCount = counts.passedCount;
      target.failedCount = counts.failedCount;
    },
  });

  writeBackendConsistencyTargetReport({
    repoRoot: sequenceOptions.repoRoot,
    env: sequenceOptions.env,
    targets,
  });

  return status || (targets.some((target) => target.status === 'failed') ? 1 : 0);
}

async function runBackendConsistency(argv = [], deps = {}) {
  if (argv.includes('-h') || argv.includes('--help')) {
    (deps.writeStdout || ((text) => process.stdout.write(text)))(
      'Usage: node scripts/node/cli/verify-backend-consistency.js [control-runtime|storage|api]\n'
        + 'Runs targeted backend Rust data/state consistency regression suites.\n'
    );
    return 0;
  }

  const [group = null, ...extraArgs] = argv;
  if (extraArgs.length > 0 || (group !== null && !BACKEND_CONSISTENCY_GROUPS.includes(group))) {
    throw new Error(`Unknown backend consistency group: ${argv.join(' ')}`);
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig = deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;

  return managedRunner({
    repoRoot,
    env,
    scope: group ? `verify-backend-consistency-${group}` : 'verify-backend-consistency',
    lockMode: 'heavy',
    commandDisplay: `node scripts/node/cli/verify-backend-consistency.js${group ? ` ${group}` : ''}`,
    runtimeConfig,
    commands: buildBackendConsistencyCommands({
      cargoJobs: runtimeConfig.backend.cargoJobs,
      cargoTestThreads: runtimeConfig.backend.cargoTestThreads,
      incremental: runtimeConfig.backend.incremental,
      group,
    }),
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
    runCommandSequenceImpl: (sequenceOptions) => runBackendConsistencyCommandSequence({
      ...sequenceOptions,
      nowImpl: deps.nowImpl,
    }),
  });
}

module.exports = { buildBackendConsistencyCommands, runBackendConsistencyCommandSequence, runBackendConsistency };
