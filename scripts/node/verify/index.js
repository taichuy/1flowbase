#!/usr/bin/env node

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const {
  buildCargoCommandEnv,
  getRepoRoot,
  resolveOutputDir,
  runCommandSequence,
  runManagedCommandSequence,
} = require('../testing/warning-capture.js');
const { loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');
const { resolveNodeBinaryFromPath } = require('../testing/node-runtime.js');
const {
  COVERAGE_ROOT,
  frontendThresholds,
  backendThresholds,
} = require('../testing/coverage-thresholds.js');
const {
  BACKEND_CONSISTENCY_TARGETS,
  BACKEND_CI_TEST_SHARDS,
  BACKEND_SHARDS,
  BACKEND_TEST_SHARDS,
  IMAGE_LLM_VISION_GATE_TARGETS,
} = require('./backend-targets.js');

const VALID_COVERAGE_TARGETS = new Set(['frontend', 'backend', 'all']);
const VALID_REPO_TARGETS = new Set(['tooling', 'frontend', 'frontend-pr', 'backend', 'all']);
const {
  buildStateProtocolCommands,
  parseStateProtocolsCliArgs,
  runStateProtocols,
} = require('./state-protocols.js');
const {
  main: runConsoleOperationRegistryHygiene,
} = require('../console-operation-registry-hygiene/cli.js');
const VALID_BACKEND_TARGETS = new Set([
  'all',
  'static',
  'fmt',
  'clippy',
  'test',
  'check',
  'image-llm-vision',
]);
const VERIFY_COMMANDS = new Set([
  'backend',
  'backend-consistency',
  'ci',
  'console-operation-registry-hygiene',
  'coverage',
  'repo',
  'state-protocols',
]);
const FRONTEND_METRICS = ['lines', 'functions', 'statements', 'branches'];
const COVERAGE_SCOPE_LABEL = '1flowbase-verify-coverage';
const BACKEND_CONSISTENCY_TARGET_REPORT_FILE = 'backend-consistency-targets.json';
const BACKEND_SHARD_BY_KEY = new Map(BACKEND_TEST_SHARDS.map((shard) => [shard.key, shard]));
const BACKEND_COVERAGE_ENTRY_BY_KEY = new Map(backendThresholds.map((entry) => [entry.key, entry]));

function parseCargoTestCounts(output) {
  const counts = {
    passedCount: null,
    failedCount: null,
  };
  const pattern = /test result:\s+(?:ok|FAILED)\.\s+(\d+) passed;\s+(\d+) failed;/gu;
  let match = pattern.exec(output);

  while (match) {
    counts.passedCount = (counts.passedCount ?? 0) + Number.parseInt(match[1], 10);
    counts.failedCount = (counts.failedCount ?? 0) + Number.parseInt(match[2], 10);
    match = pattern.exec(output);
  }

  return counts;
}

function buildBackendConsistencyTargetResult(command) {
  const target = BACKEND_CONSISTENCY_TARGETS.find((candidate) => candidate.label === command.label);

  return {
    label: command.label,
    packageName: target?.packageName || command.args?.[2] || '',
    filter: target?.filter || command.args?.[5] || '',
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

function resolveScriptsNodeEntry(repoRoot, entryName) {
  return path.join(repoRoot, 'scripts', 'node', entryName);
}

function resolveScriptsNodeCliEntry(repoRoot, entryName) {
  return `${resolveScriptsNodeEntry(repoRoot, entryName)}.js`;
}
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
  shard,
}) {
  const normalizedShard = normalizeBackendShard(shard);
  const labelSuffix = normalizedShard ? `-${normalizedShard.key}` : '';
  const packageArgs = buildBackendPackageArgs(normalizedShard);

  if (target === 'clippy') {
    return {
      label: `cargo-clippy${labelSuffix}`,
      command: 'cargo',
      args: ['clippy', ...packageArgs, '--all-targets', '--jobs', String(cargoJobs), '--', '-D', 'warnings'],
      cwd: 'api',
      env: buildCargoCommandEnv({ cargoParallelism: cargoJobs, disableIncremental: true }),
    };
  }

  if (target === 'test') {
    return {
      label: `cargo-test${labelSuffix}`,
      command: 'cargo',
      args: ['test', ...packageArgs, '--jobs', String(cargoJobs), '--', `--test-threads=${cargoTestThreads}`],
      cwd: 'api',
      env: buildCargoCommandEnv({ cargoParallelism: cargoJobs, disableIncremental: true }),
    };
  }

  if (target === 'check') {
    return {
      label: `cargo-check${labelSuffix}`,
      command: 'cargo',
      args: ['check', ...packageArgs, '--jobs', String(cargoJobs)],
      cwd: 'api',
      env: buildCargoCommandEnv({ cargoParallelism: cargoJobs, disableIncremental: true }),
    };
  }

  throw new Error(`Unsupported backend cargo target: ${target}`);
}

function buildImageLlmVisionGateCommands({ cargoJobs, cargoTestThreads }) {
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
    env: buildCargoCommandEnv({ cargoParallelism: cargoJobs, disableIncremental: true }),
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
    return [buildBackendCargoCommand({ target, cargoJobs, cargoTestThreads, shard })];
  }

  if (target === 'image-llm-vision') {
    return buildImageLlmVisionGateCommands({ cargoJobs, cargoTestThreads });
  }

  return [
    buildRustBackendStaticGateCommand({ repoRoot, env }),
    buildBackendFmtCommand({ cargoJobs }),
    buildBackendCargoCommand({ target: 'clippy', cargoJobs, cargoTestThreads }),
    ...buildImageLlmVisionGateCommands({ cargoJobs, cargoTestThreads }),
    buildBackendCargoCommand({ target: 'test', cargoJobs, cargoTestThreads }),
    buildBackendCargoCommand({ target: 'check', cargoJobs, cargoTestThreads }),
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
    'Usage: node scripts/node/verify-backend.js [all|static|fmt|clippy|test|check|image-llm-vision] [core-libs|runtime-storage|apps|control-plane|api-server|plugin-runner]\n'
      + 'Runs backend Rust gates, optionally restricted to a CI shard. Package-level app shards are supported for test.\n'
  );
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
      repoRoot,
      env,
      target: options.target,
      shard,
    }),
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

function buildBackendConsistencyCommands({ cargoJobs, cargoTestThreads }) {
  return BACKEND_CONSISTENCY_TARGETS.map((target) => ({
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
    env: buildCargoCommandEnv({ cargoParallelism: cargoJobs, disableIncremental: true }),
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
      target.status = result.status === 0 ? 'passed' : 'failed';
      target.exitCode = result.status ?? 1;
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

  return status;
}

async function runBackendConsistency(argv = [], deps = {}) {
  if (argv.includes('-h') || argv.includes('--help')) {
    (deps.writeStdout || ((text) => process.stdout.write(text)))(
      'Usage: node scripts/node/cli/verify-backend-consistency.js\n'
        + 'Runs targeted backend Rust data/state consistency regression suites.\n'
    );
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig = deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;

  return managedRunner({
    repoRoot,
    env,
    scope: 'verify-backend-consistency',
    lockMode: 'heavy',
    commandDisplay: 'node scripts/node/cli/verify-backend-consistency.js',
    runtimeConfig,
    commands: buildBackendConsistencyCommands({
      cargoJobs: runtimeConfig.backend.cargoJobs,
      cargoTestThreads: runtimeConfig.backend.cargoTestThreads,
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

function buildCiCommands({ repoRoot, env = process.env }) {
  const nodeBinary = resolveNodeBinaryFromPath(env);

  return [
    {
      label: 'ci-verify-repo',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'verify'), 'repo'],
      cwd: repoRoot,
    },
    {
      label: 'ci-backend-consistency',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'verify'), 'backend-consistency'],
      cwd: repoRoot,
    },
    {
      label: 'ci-coverage-all',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'verify'), 'coverage', 'all'],
      cwd: repoRoot,
    },
  ];
}

function usageCi(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/verify-ci.js\n'
      + 'Runs: verify-repo + verify-backend-consistency + verify-coverage all\n'
  );
}

async function runCi(argv = [], deps = {}) {
  if (argv.includes('-h') || argv.includes('--help')) {
    usageCi(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig = deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;

  return managedRunner({
    repoRoot,
    env,
    scope: 'verify-ci',
    lockMode: 'heavy',
    commandDisplay: 'node scripts/node/verify-ci.js',
    runtimeConfig,
    commands: buildCiCommands({ repoRoot, env }),
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

function parseCoverageCliArgs(argv) {
  if (argv.includes('-h') || argv.includes('--help')) {
    return { help: true, target: 'all' };
  }

  const [target = 'all', ...backendKeys] = argv;

  if (!VALID_COVERAGE_TARGETS.has(target)) {
    throw new Error(`Unknown coverage target: ${target}`);
  }

  if (backendKeys.length > 0 && target !== 'backend') {
    throw new Error(`Coverage package filters are only supported for backend target: ${target}`);
  }

  if (backendKeys.length === 0) {
    return { help: false, target };
  }

  const unknownKey = backendKeys.find((backendKey) => !BACKEND_COVERAGE_ENTRY_BY_KEY.has(backendKey));

  if (unknownKey) {
    throw new Error(`Unknown backend coverage package: ${unknownKey}`);
  }

  return { help: false, target, backendKeys };
}

function buildCoverageFrontendCommand({ repoRoot }) {
  return {
    label: 'frontend-coverage',
    command: 'pnpm',
    args: ['--dir', 'web/app', 'test:coverage'],
    cwd: repoRoot,
  };
}

function buildCoverageFrontendPageRuntimeCommand({ repoRoot }) {
  return {
    label: 'frontend-page-runtime-coverage',
    command: 'pnpm',
    args: [
      '--dir',
      'web/packages/page-runtime',
      'exec',
      'vitest',
      'run',
      '--coverage',
      '--coverage.reporter=json-summary',
      '--coverage.reportsDirectory=../../../tmp/test-governance/coverage/frontend/page-runtime',
    ],
    cwd: repoRoot,
  };
}

function selectBackendCoverageEntries(backendKeys) {
  if (!backendKeys || backendKeys.length === 0) {
    return backendThresholds;
  }

  return backendKeys.map((backendKey) => {
    const entry = BACKEND_COVERAGE_ENTRY_BY_KEY.get(backendKey);

    if (!entry) {
      throw new Error(`Unknown backend coverage package: ${backendKey}`);
    }

    return entry;
  });
}

function buildCoverageBackendCommands({ repoRoot, cargoParallelism, cargoTestThreads, backendKeys }) {
  return selectBackendCoverageEntries(backendKeys).map((entry) => ({
    label: `backend-coverage-${entry.key}`,
    command: 'cargo',
    args: [
      'llvm-cov',
      '--package',
      entry.packageName,
      '--json',
      '--summary-only',
      '--output-path',
      path.join(repoRoot, COVERAGE_ROOT, 'backend', `${entry.key}.json`),
      '--',
      `--test-threads=${cargoTestThreads}`,
    ],
    cwd: 'api',
    env: buildCargoCommandEnv({ cargoParallelism, disableIncremental: true }),
  }));
}

function buildCoverageBackendCleanupCommands() {
  return [
    {
      label: 'backend-coverage-clean',
      command: 'cargo',
      args: ['llvm-cov', 'clean', '--workspace'],
      cwd: 'api',
    },
  ];
}

function usageCoverage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/verify-coverage.js [frontend|backend|all]\n'
      + 'Runs repository-owned coverage gates for frontend, backend, or both.\n'
  );
}

function normalizeCoveragePath(filePath) {
  return filePath.replace(/\\/gu, '/');
}

function readMetricPct(metricSummary) {
  if (!metricSummary || typeof metricSummary !== 'object') {
    return null;
  }

  if (
    Number.isFinite(metricSummary.total)
    && Number.isFinite(metricSummary.covered)
    && metricSummary.total > 0
  ) {
    return (metricSummary.covered / metricSummary.total) * 100;
  }

  if (Number.isFinite(metricSummary.pct)) {
    return metricSummary.pct;
  }

  return null;
}

function aggregateMetric(matchedEntries, metric) {
  let weightedCovered = 0;
  let weightedTotal = 0;
  let pctSum = 0;
  let pctCount = 0;

  for (const entry of matchedEntries) {
    const metricSummary = entry[metric];

    if (
      metricSummary
      && Number.isFinite(metricSummary.total)
      && Number.isFinite(metricSummary.covered)
      && metricSummary.total > 0
    ) {
      weightedCovered += metricSummary.covered;
      weightedTotal += metricSummary.total;
      continue;
    }

    const pct = readMetricPct(metricSummary);

    if (pct !== null) {
      pctSum += pct;
      pctCount += 1;
    }
  }

  if (weightedTotal > 0) {
    return (weightedCovered / weightedTotal) * 100;
  }

  if (pctCount > 0) {
    return pctSum / pctCount;
  }

  return 0;
}

function matchesFrontendThreshold(filePath, prefix) {
  return normalizeCoveragePath(filePath).includes(`/${prefix}`);
}

function collectFrontendCoverageFailures(summary) {
  const entries = Object.entries(summary).filter(([filePath]) => filePath !== 'total');

  return frontendThresholds.flatMap((threshold) => {
    const matchedEntries = entries
      .filter(([filePath]) => matchesFrontendThreshold(filePath, threshold.prefix))
      .map(([, coverage]) => coverage);

    return FRONTEND_METRICS.flatMap((metric) => {
      const actualPct = aggregateMetric(matchedEntries, metric);
      const expectedPct = threshold.thresholds[metric];

      if (actualPct + Number.EPSILON >= expectedPct) {
        return [];
      }

      return [{
        key: threshold.key,
        prefix: threshold.prefix,
        metric,
        expectedPct,
        actualPct,
      }];
    });
  });
}

function readBackendLinePct(summary) {
  return summary?.data?.[0]?.totals?.lines?.percent ?? 0;
}

function collectBackendCoverageFailures(summaries, backendKeys) {
  return selectBackendCoverageEntries(backendKeys).flatMap((threshold) => {
    const actualPct = readBackendLinePct(summaries[threshold.key]);
    const expectedPct = threshold.line;

    if (actualPct + Number.EPSILON >= expectedPct) {
      return [];
    }

    return [{
      key: threshold.key,
      metric: 'lines',
      expectedPct,
      actualPct,
    }];
  });
}

function ensureCargoLlvmCovInstalled(spawnSyncImpl = spawnSync, deps = {}) {
  const repoRoot = deps.repoRoot || getRepoRoot();
  const result = spawnSyncImpl('cargo', ['llvm-cov', '--help'], {
    cwd: path.join(repoRoot, 'api'),
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(
      'cargo llvm-cov is required for backend coverage. Install it with: cargo install cargo-llvm-cov --locked'
    );
  }
}

function ensureCoverageOutputDirs(repoRoot, target) {
  if (target === 'frontend' || target === 'all') {
    fs.mkdirSync(path.join(repoRoot, COVERAGE_ROOT, 'frontend'), { recursive: true });
  }

  if (target === 'backend' || target === 'all') {
    fs.mkdirSync(path.join(repoRoot, COVERAGE_ROOT, 'backend'), { recursive: true });
  }
}

function cleanJsonFiles(directoryPath) {
  if (!fs.existsSync(directoryPath)) {
    return;
  }

  for (const entry of fs.readdirSync(directoryPath, { withFileTypes: true })) {
    if (entry.isFile() && entry.name.endsWith('.json')) {
      fs.rmSync(path.join(directoryPath, entry.name), { force: true });
    }
  }
}

function cleanCoverageOutputFiles(repoRoot, target, backendKeys) {
  if (target === 'frontend' || target === 'all') {
    fs.rmSync(
      path.join(repoRoot, COVERAGE_ROOT, 'frontend', 'coverage-summary.json'),
      { force: true }
    );
    fs.rmSync(
      path.join(repoRoot, COVERAGE_ROOT, 'frontend', 'page-runtime'),
      { recursive: true, force: true }
    );
  }

  if (target === 'backend' || target === 'all') {
    const backendCoverageDir = path.join(repoRoot, COVERAGE_ROOT, 'backend');

    if (!backendKeys || backendKeys.length === 0) {
      cleanJsonFiles(backendCoverageDir);
      return;
    }

    for (const backendKey of backendKeys) {
      fs.rmSync(path.join(backendCoverageDir, `${backendKey}.json`), { force: true });
    }
  }
}

function readJsonFile(filePath, readFileSyncImpl = fs.readFileSync) {
  return JSON.parse(readFileSyncImpl(filePath, 'utf8'));
}

function loadFrontendCoverageSummary(repoRoot, readFileSyncImpl = fs.readFileSync) {
  const appSummary = readJsonFile(
    path.join(repoRoot, COVERAGE_ROOT, 'frontend', 'coverage-summary.json'),
    readFileSyncImpl
  );
  const pageRuntimeSummaryPath = path.join(
    repoRoot,
    COVERAGE_ROOT,
    'frontend',
    'page-runtime',
    'coverage-summary.json'
  );

  if (!fs.existsSync(pageRuntimeSummaryPath)) {
    return appSummary;
  }

  const pageRuntimeSummary = readJsonFile(pageRuntimeSummaryPath, readFileSyncImpl);

  return {
    ...appSummary,
    ...Object.fromEntries(
      Object.entries(pageRuntimeSummary).filter(([filePath]) => filePath !== 'total')
    ),
  };
}

function loadBackendCoverageSummaries(repoRoot, readFileSyncImpl = fs.readFileSync, backendKeys) {
  return Object.fromEntries(
    selectBackendCoverageEntries(backendKeys).map((entry) => [
      entry.key,
      readJsonFile(
        path.join(repoRoot, COVERAGE_ROOT, 'backend', `${entry.key}.json`),
        readFileSyncImpl
      ),
    ])
  );
}

function formatPct(value) {
  return value.toFixed(2);
}

function reportCoverageThresholds({
  repoRoot,
  target,
  readFileSyncImpl = fs.readFileSync,
  writeStdout = (text) => process.stdout.write(text),
  writeStderr = (text) => process.stderr.write(text),
  backendKeys,
}) {
  const failures = [];

  if (target === 'frontend' || target === 'all') {
    failures.push(...collectFrontendCoverageFailures(loadFrontendCoverageSummary(repoRoot, readFileSyncImpl)));
  }

  if (target === 'backend' || target === 'all') {
    failures.push(...collectBackendCoverageFailures(
      loadBackendCoverageSummaries(repoRoot, readFileSyncImpl, backendKeys),
      backendKeys
    ));
  }

  if (failures.length > 0) {
    writeStderr(`[${COVERAGE_SCOPE_LABEL}] Coverage threshold failures:\n`);

    for (const failure of failures) {
      writeStderr(
        `- ${failure.key} ${failure.metric}: expected >= ${formatPct(failure.expectedPct)}%, `
          + `received ${formatPct(failure.actualPct)}%\n`
      );
    }

    return 1;
  }

  writeStdout(`[${COVERAGE_SCOPE_LABEL}] Coverage thresholds passed for ${target}.\n`);
  return 0;
}

async function runCoverage(argv = [], deps = {}) {
  const options = parseCoverageCliArgs(argv);

  if (options.help) {
    usageCoverage(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig = deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;
  const coverageCommands = [];
  const backendKeys = options.backendKeys;

  if (options.target === 'backend' || options.target === 'all') {
    ensureCargoLlvmCovInstalled(deps.preflightSpawnSyncImpl, { repoRoot });
  }

  ensureCoverageOutputDirs(repoRoot, options.target);
  cleanCoverageOutputFiles(repoRoot, options.target, backendKeys);

  if (options.target === 'frontend' || options.target === 'all') {
    coverageCommands.push(buildCoverageFrontendCommand({ repoRoot }));
    coverageCommands.push(buildCoverageFrontendPageRuntimeCommand({ repoRoot }));
  }

  if (options.target === 'backend' || options.target === 'all') {
    coverageCommands.push(...buildCoverageBackendCommands({
      repoRoot,
      cargoParallelism: runtimeConfig.backend.cargoJobs,
      cargoTestThreads: runtimeConfig.backend.cargoTestThreads,
      backendKeys,
    }));
  }

  const shouldCleanupBackendCoverage = options.target === 'backend' || options.target === 'all';
  const scopeSuffix = backendKeys?.length ? `-${backendKeys.join('-')}` : '';
  const commandSuffix = [
    options.target === 'all' ? '' : options.target,
    ...(backendKeys ?? []),
  ].filter(Boolean).join(' ');
  const commands = shouldCleanupBackendCoverage
    ? [...buildCoverageBackendCleanupCommands(), ...coverageCommands]
    : coverageCommands;

  return managedRunner({
    repoRoot,
    env,
    scope: `verify-coverage-${options.target}${scopeSuffix}`,
    lockMode: 'heavy',
    commandDisplay: `node scripts/node/verify-coverage.js ${commandSuffix}`.trim(),
    runtimeConfig,
    commands,
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
    runCommandSequenceImpl: (sequenceOptions) => {
      let status = 0;

      try {
        status = runCommandSequence({
          ...sequenceOptions,
          commands: sequenceOptions.commands,
        });

        if (status === 0) {
          status = reportCoverageThresholds({
            repoRoot,
            target: options.target,
            readFileSyncImpl: deps.readFileSyncImpl,
            writeStdout: deps.writeStdout,
            writeStderr: deps.writeStderr,
            backendKeys,
          });
        }
      } finally {
        if (shouldCleanupBackendCoverage) {
          const cleanupStatus = runCommandSequence({
            repoRoot,
            env: sequenceOptions.env,
            scope: `verify-coverage-${options.target}-clean-after`,
            commands: buildCoverageBackendCleanupCommands(),
            spawnSyncImpl: deps.spawnSyncImpl,
            writeStdout: deps.writeStdout,
            writeStderr: deps.writeStderr,
          });

          if (status === 0 && cleanupStatus !== 0) {
            status = cleanupStatus;
          }
        }
      }

      return status;
    },
  });
}

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
      label: 'repo-growth-table-report',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'tooling'), 'growth-table-report'],
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
  const frontendCommands = [
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
