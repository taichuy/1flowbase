const path = require('node:path');
const { buildCargoCommandEnv, getRepoRoot, runCommandSequence, runManagedCommandSequence } = require('../testing/warning-capture.js');
const { loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');
const { resolveNodeBinaryFromPath } = require('../testing/node-runtime.js');
const { BACKEND_SHARDS, BACKEND_TEST_SHARDS, IMAGE_LLM_VISION_GATE_TARGETS, OFFICIAL_I18N_SEED_GATE_TARGETS } = require('./backend-targets.js');
const { parseCargoTestCounts } = require('./cargo-test-results.js');
const { buildHistoryMaintenanceCommands } = require('./history-maintenance.js');
const { resolveScriptsNodeCliEntry } = require("./command-paths.js");

const VALID_BACKEND_TARGETS = new Set([
  'all',
  'static',
  'fmt',
  'clippy',
  'test',
  'check',
  'image-llm-vision',
  'official-i18n-seed',
  'history-maintenance',
]);

const BACKEND_SHARD_BY_KEY = new Map(BACKEND_TEST_SHARDS.map((shard) => [shard.key, shard]));

function buildRustBackendStaticGateCommand({ repoRoot, env = process.env }) {
  return {
    label: 'rust-backend-static-gate',
    command: resolveNodeBinaryFromPath(env),
    args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'check-rust-backend'],
    cwd: repoRoot,
  };
}

function normalizeBackendShard(shard) {
  if (!shard) {
    return null;
  }

  if (typeof shard === 'string') {
    const resolved = BACKEND_SHARD_BY_KEY.get(shard);

    if (!resolved) {
      throw new Error(`Unknown backend shard: ${shard}`);
    }

    return resolved;
  }

  return shard;
}

function buildBackendPackageArgs(shard) {
  const normalizedShard = normalizeBackendShard(shard);

  if (!normalizedShard) {
    return ['--workspace'];
  }

  return normalizedShard.packages.flatMap((packageName) => ['--package', packageName]);
}

function buildBackendCargoCommand({
  target,
  cargoJobs,
  cargoTestThreads,
  incremental = false,
  shard,
  testSelection,
}) {
  const normalizedShard = normalizeBackendShard(shard);
  const labelSuffix = testSelection ? '-selected' : normalizedShard ? `-${normalizedShard.key}` : '';
  const packageArgs = testSelection
    ? testSelection.packages.flatMap((packageName) => ['--package', packageName])
    : buildBackendPackageArgs(normalizedShard);

  if (target === 'clippy') {
    return {
      label: `cargo-clippy${labelSuffix}`,
      command: 'cargo',
      args: ['clippy', ...packageArgs, '--all-targets', '--jobs', String(cargoJobs), '--', '-D', 'warnings'],
      cwd: 'api',
      env: buildCargoCommandEnv({ cargoParallelism: cargoJobs, incremental }),
    };
  }

  if (target === 'test') {
    if (normalizedShard?.nextestPartition) {
      return {
        label: `cargo-nextest${labelSuffix}`,
        command: 'cargo',
        args: [
          'nextest',
          'run',
          ...packageArgs,
          '--partition',
          normalizedShard.nextestPartition,
          '--test-threads',
          String(cargoTestThreads),
          '--no-fail-fast',
          '--no-tests=fail',
        ],
        cwd: 'api',
        env: {
          ...buildCargoCommandEnv({ cargoParallelism: cargoJobs, incremental }),
          CARGO_PROFILE_TEST_DEBUG: '0',
          ...(normalizedShard.key.startsWith('api-server-')
            ? { RUST_MIN_STACK: String(8 * 1024 * 1024) }
            : {}),
        },
      };
    }

    return {
      label: `cargo-test${labelSuffix}`,
      command: 'cargo',
      args: [
        'test', ...packageArgs, ...(testSelection?.targetArgs ?? []),
        '--jobs', String(cargoJobs),
        ...(testSelection?.filter ? [testSelection.filter] : []),
        '--', `--test-threads=${cargoTestThreads}`,
        ...(testSelection?.exact ? ['--exact'] : []),
      ],
      cwd: 'api',
      env: {
        ...buildCargoCommandEnv({ cargoParallelism: cargoJobs, incremental }),
        // Keep the monolithic API test harness inside the CI budget without changing test behavior.
        ...(normalizedShard?.key === 'api-server'
          ? { CARGO_PROFILE_TEST_DEBUG: '0' }
          : {}),
      },
    };
  }

  if (target === 'check') {
    return {
      label: `cargo-check${labelSuffix}`,
      command: 'cargo',
      args: ['check', ...packageArgs, '--jobs', String(cargoJobs)],
      cwd: 'api',
      env: buildCargoCommandEnv({ cargoParallelism: cargoJobs, incremental }),
    };
  }

  throw new Error(`Unsupported backend cargo target: ${target}`);
}

function buildImageLlmVisionGateCommands({ cargoJobs, cargoTestThreads, incremental = false }) {
  return IMAGE_LLM_VISION_GATE_TARGETS.map((target) => ({
    label: target.label,
    command: 'cargo',
    args: [
      'test',
      '-p',
      target.packageName,
      '--jobs',
      String(cargoJobs),
      target.filter,
      '--',
      `--test-threads=${cargoTestThreads}`,
    ],
    cwd: 'api',
    env: buildCargoCommandEnv({ cargoParallelism: cargoJobs, incremental }),
  }));
}

function buildBackendTestCommands({
  cargoJobs,
  cargoTestThreads,
  incremental = false,
  repoRoot = getRepoRoot(),
  env = process.env,
  selection,
}) {
  return [
    buildRustBackendStaticGateCommand({ repoRoot, env }),
    {
      ...buildBackendCargoCommand({ target: 'test', cargoJobs, cargoTestThreads, incremental, testSelection: selection }),
      requireExecutedTests: true,
    },
  ];
}

function buildOfficialI18nSeedGateCommands({ cargoJobs, cargoTestThreads, incremental = false }) {
  return OFFICIAL_I18N_SEED_GATE_TARGETS.map((target) => ({
    label: target.label,
    command: 'cargo',
    args: [
      'test',
      '-p',
      target.packageName,
      '--jobs',
      String(cargoJobs),
      target.filter,
      '--',
      `--test-threads=${cargoTestThreads}`,
    ],
    cwd: 'api',
    env: buildCargoCommandEnv({ cargoParallelism: cargoJobs, incremental }),
  }));
}

function buildBackendFmtCommand({ cargoJobs }) {
  return {
    label: 'cargo-fmt',
    command: 'cargo',
    args: ['fmt', '--all', '--check'],
    cwd: 'api',
    env: buildCargoCommandEnv({ cargoParallelism: cargoJobs }),
  };
}

function buildBackendCommands({
  cargoJobs,
  cargoTestThreads,
  incremental = false,
  repoRoot = getRepoRoot(),
  env = process.env,
  target = 'all',
  shard,
}) {
  if (target === 'static') {
    return [buildRustBackendStaticGateCommand({ repoRoot, env })];
  }

  if (target === 'fmt') {
    return [buildBackendFmtCommand({ cargoJobs })];
  }

  if (target === 'clippy' || target === 'test' || target === 'check') {
    const normalizedShard = normalizeBackendShard(shard);
    // Only the complete quality-gate workflow schedules the matching coverage
    // owners. Standalone/PR shards and workspace commands retain every package.
    const testShard = target === 'test' && env.QUALITY_GATE_SCOPE === 'ci' && normalizedShard?.coverageOwnedPackages
      ? { ...normalizedShard, packages: normalizedShard.packages.filter((name) => !normalizedShard.coverageOwnedPackages.includes(name)) }
      : normalizedShard;
    const commands = [buildBackendCargoCommand({ target, cargoJobs, cargoTestThreads, incremental, shard: testShard })];
    // The SQL report needs PostgreSQL, not a private deployed page. Run once in
    // the database-backed batch, and keep it inside the candidate gate receipt.
    if (target === 'test' && normalizedShard?.key === 'storage-postgres-1-of-4') {
      commands.push({
        label: 'model-usage-report-postgres',
        command: process.execPath,
        args: ['--test', '--test-reporter=tap', path.join(repoRoot, 'scripts/node/model-usage-report/integration/postgres.test.js')],
        cwd: repoRoot,
      });
    }
    return commands;
  }

  if (target === 'image-llm-vision') {
    return buildImageLlmVisionGateCommands({ cargoJobs, cargoTestThreads, incremental });
  }

  if (target === 'official-i18n-seed') {
    return buildOfficialI18nSeedGateCommands({ cargoJobs, cargoTestThreads, incremental });
  }

  if (target === 'history-maintenance') {
    return buildHistoryMaintenanceCommands({ cargoJobs, cargoTestThreads, incremental });
  }

  return [
    buildRustBackendStaticGateCommand({ repoRoot, env }),
    buildBackendFmtCommand({ cargoJobs }),
    buildBackendCargoCommand({ target: 'clippy', cargoJobs, cargoTestThreads, incremental }),
    ...buildOfficialI18nSeedGateCommands({ cargoJobs, cargoTestThreads, incremental }),
    ...buildHistoryMaintenanceCommands({ cargoJobs, cargoTestThreads, incremental }),
    {
      ...buildBackendCargoCommand({ target: 'test', cargoJobs, cargoTestThreads, incremental }),
      requireExecutedTests: true,
    },
  ];
}

function parseBackendCliArgs(argv = []) {
  if (argv.includes('-h') || argv.includes('--help')) {
    return { help: true, target: 'all', shard: null };
  }

  const [target = 'all', shardKey = null] = argv;
  if (!VALID_BACKEND_TARGETS.has(target)) {
    throw new Error(`Unknown backend target: ${target}`);
  }

  const validShards = target === 'test' ? BACKEND_TEST_SHARDS : BACKEND_SHARDS;
  const validShardByKey = new Map(validShards.map((shard) => [shard.key, shard]));
  const shard = shardKey ? validShardByKey.get(shardKey) : null;
  if (shardKey && !shard) {
    throw new Error(`Unknown backend shard: ${shardKey}`);
  }

  if (shard && !['clippy', 'test', 'check'].includes(target)) {
    throw new Error(`Backend shard is only supported for clippy, test, or check targets: ${target}`);
  }

  return { help: false, target, shard: shard?.key ?? null };
}

function usageBackend(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/verify-backend.js [all|static|fmt|clippy|test|check|image-llm-vision|official-i18n-seed] [core-libs|runtime-storage|apps|control-plane|api-server-N-of-4]\n'
      + 'Runs backend Rust gates, optionally restricted to a CI shard. Package-level app shards are supported for test.\n'
  );
}

function runBackendCommandSequence(sequenceOptions) {
  let emptySelection = false;
  const status = runCommandSequence({
    ...sequenceOptions,
    onCommandComplete({ command, result }) {
      if (command.label === 'model-usage-report-postgres') {
        const output = `${result.stdout || ''}\n${result.stderr || ''}`;
        const count = (name) => Number(output.match(new RegExp(`^# ${name} (\\d+)$`, 'mu'))?.[1] ?? NaN);
        if (result.status === 0 && !(count('tests') > 0 && count('pass') === count('tests') && count('fail') === 0 && count('skipped') === 0)) {
          emptySelection = true;
          (sequenceOptions.writeStderr || process.stderr.write.bind(process.stderr))(
            `${command.label}: no complete passing SQL evidence; refusing empty or skipped PostgreSQL gate\n`,
          );
        }
        return;
      }
      if (!command.requireExecutedTests
        && !command.label.startsWith('cargo-test-image-llm-vision-')
        && !command.label.startsWith('cargo-test-official-i18n-seed-')
        && !command.label.startsWith('cargo-test-history-maintenance-')) return;
      const counts = parseCargoTestCounts(`${result.stdout || ''}\n${result.stderr || ''}`);
      if (result.status === 0 && !(counts.passedCount > 0 && counts.failedCount === 0)) {
        emptySelection = true;
        (sequenceOptions.writeStderr || process.stderr.write.bind(process.stderr))(
          `${command.label}: no executed passing tests; refusing empty or failed protected Cargo gate\n`,
        );
      }
    },
  });
  return status || (emptySelection ? 1 : 0);
}

async function runBackend(argv = [], deps = {}) {
  const options = parseBackendCliArgs(argv);

  if (options.help) {
    usageBackend(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig = deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;
  const shard = options.shard ? BACKEND_SHARD_BY_KEY.get(options.shard) : null;
  const scopeSuffix = options.target === 'all'
    ? ''
    : `-${options.target}${options.shard ? `-${options.shard}` : ''}`;
  const commandSuffix = options.target === 'all'
    ? ''
    : ` ${options.target}${options.shard ? ` ${options.shard}` : ''}`;

  return managedRunner({
    repoRoot,
    env,
    scope: `verify-backend${scopeSuffix}`,
    lockMode: 'heavy',
    commandDisplay: `node scripts/node/verify-backend.js${commandSuffix}`,
    runtimeConfig,
    commands: buildBackendCommands({
      cargoJobs: runtimeConfig.backend.cargoJobs,
      cargoTestThreads: runtimeConfig.backend.cargoTestThreads,
      incremental: runtimeConfig.backend.incremental,
      repoRoot,
      env,
      target: options.target,
      shard,
    }),
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
    runCommandSequenceImpl: runBackendCommandSequence,
  });
}

module.exports = { buildImageLlmVisionGateCommands, buildBackendCommands, buildBackendTestCommands, buildRustBackendStaticGateCommand, parseBackendCliArgs, runBackend, runBackendCommandSequence };
