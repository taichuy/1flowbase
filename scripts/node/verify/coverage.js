const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { buildCargoCommandEnv, getRepoRoot, runCommandSequence, runManagedCommandSequence } = require('../testing/warning-capture.js');
const { loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');
const { COVERAGE_ROOT } = require('../testing/coverage-thresholds.js');
const { parseCargoTestCounts } = require('./cargo-test-results.js');
const { selectBackendCoverageEntries, reportCoverageThresholds, BACKEND_COVERAGE_ENTRY_BY_KEY } = require("./coverage-report.js");

const VALID_COVERAGE_TARGETS = new Set(['frontend', 'backend', 'all']);

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

function buildCoverageBackendCommands({ repoRoot, cargoParallelism, cargoTestThreads, backendKeys }) {
  return selectBackendCoverageEntries(backendKeys).flatMap((entry) => {
    const usesNextest = entry.key === 'api-server';
    const env = {
      ...buildCargoCommandEnv({ cargoParallelism, disableIncremental: true }),
      // Coverage instrumentation does not require Rust debug symbols for source mapping.
      ...(usesNextest ? { CARGO_PROFILE_TEST_DEBUG: '0' } : {}),
    };
    if (entry.key === 'control-plane') {
      const outputPath = path.join(
        repoRoot,
        COVERAGE_ROOT,
        'backend',
        `${entry.key}.json`
      );
      const command = 'cargo';
      const cwd = 'api';
      const integrationCommand = (label, packageName, { filter, testTarget }) => ({
        label,
        command,
        args: [
          'llvm-cov',
          '--package',
          entry.packageName,
          '--package',
          packageName,
          '--exclude-from-report',
          packageName,
          ...(testTarget ? ['--test', testTarget] : []),
          '--no-clean',
          '--json',
          '--summary-only',
          '--output-path',
          outputPath,
          '--',
          ...(filter ? [filter] : []),
          `--test-threads=${cargoTestThreads}`,
        ],
        cwd,
        env: packageName === 'api-server'
          ? { ...env, CARGO_PROFILE_TEST_DEBUG: '0' }
          : env,
      });

      return [
        {
          label: 'backend-coverage-control-plane-tests',
          command,
          args: [
            'llvm-cov',
            '--package',
            entry.packageName,
            '--no-clean',
            '--json',
            '--summary-only',
            '--output-path',
            outputPath,
            '--',
            `--test-threads=${cargoTestThreads}`,
          ],
          cwd,
          env,
        },
        integrationCommand(
          'backend-coverage-control-plane-mcp-management-integration',
          'control-plane-postgres-tests',
          { testTarget: 'mcp_management_integration' }
        ),
        integrationCommand(
          'backend-coverage-control-plane-ui-management-integration',
          'storage-durable-postgres',
          { filter: 'ui_management_repository_tests' }
        ),
        integrationCommand(
          'backend-coverage-control-plane-mcp-routes-integration',
          'api-server',
          { filter: 'mcp_management_routes' }
        ),
        integrationCommand(
          'backend-coverage-control-plane-ui-routes-integration',
          'api-server',
          { filter: 'ui_management_routes' }
        ),
        {
          // llvm-cov does not include doctests by default. Keep the ordinary
          // cargo test owner's documentation checks, including future examples.
          label: 'backend-coverage-control-plane-doc-tests',
          command,
          args: ['test', '--package', entry.packageName, '--doc', '--', `--test-threads=${cargoTestThreads}`],
          cwd,
          env,
        },
      ];
    }
    if (entry.key === 'storage-postgres') {
      const outputPath = path.join(
        repoRoot,
        COVERAGE_ROOT,
        'backend',
        `${entry.key}.json`
      );
      const baseCommand = {
        label: 'backend-coverage-storage-postgres',
        command: 'cargo',
        args: [
          'llvm-cov',
          '--package',
          entry.packageName,
          '--no-clean',
          '--json',
          '--summary-only',
          '--output-path',
          outputPath,
          '--',
          `--test-threads=${cargoTestThreads}`,
        ],
        cwd: 'api',
        env,
      };
      return [
        baseCommand,
        {
          ...baseCommand,
          label: 'backend-coverage-storage-postgres-control-plane-integration',
          args: [
            'llvm-cov',
            '--package',
            entry.packageName,
            '--package',
            'control-plane-postgres-tests',
            '--exclude-from-report',
            'control-plane-postgres-tests',
            '--test',
            'mcp_instance_copy',
            '--test',
            'mcp_management_integration',
            '--test',
            'file_management_backup_integration',
            '--test',
            'i18n_catalog_integration',
            '--test',
            'role_policy_integration',
            '--test',
            'plugin_network_integration',
            '--test',
            'runtime_record_integration',
            '--test',
            'network_egress_integration',
            '--test',
            'orchestration_runtime_integration',
            '--test',
            'role_console_policy_integration',
            '--no-clean',
            '--json',
            '--summary-only',
            '--output-path',
            outputPath,
            '--',
            `--test-threads=${cargoTestThreads}`,
          ],
        },
      ];
    }
    return {
      label: `backend-coverage-${entry.key}`,
      command: 'cargo',
      args: [
        'llvm-cov',
        ...(usesNextest ? ['nextest'] : []),
        '--package',
        entry.packageName,
        '--no-clean',
        '--json',
        '--summary-only',
        '--output-path',
        path.join(repoRoot, COVERAGE_ROOT, 'backend', `${entry.key}.json`),
        ...(usesNextest
          ? ['--test-threads', String(cargoTestThreads), '--no-fail-fast', '--no-tests=fail']
          : ['--', `--test-threads=${cargoTestThreads}`]),
      ],
      cwd: 'api',
      env,
    };
  });
}

function buildCoverageBackendCleanupCommands() {
  return [
    {
      label: 'backend-coverage-clean',
      command: 'cargo',
      // Cached instrumented binaries can duplicate LLVM regions and inflate
      // the denominator even when execution profiles are fresh.
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
          onCommandComplete({ command, result }) {
            // nextest already fails on an empty selection via --no-tests=fail.
            // Every libtest coverage command needs its own executed-test proof;
            // adjacent suites and a valid coverage JSON cannot fill the gap.
            if (!command.label.startsWith('backend-coverage-')
              || command.args[1] === 'clean'
              || command.args.includes('nextest') || result.status !== 0) return;
            const counts = parseCargoTestCounts(`${result.stdout || ''}\n${result.stderr || ''}`);
            const isControlPlaneDoctest = command.label === 'backend-coverage-control-plane-doc-tests'
              && command.args[0] === 'test' && command.args.includes('--doc');
            const executed = isControlPlaneDoctest
              ? counts.passedCount !== null && counts.passedCount >= 0
              : counts.passedCount > 0;
            if (!(executed && counts.failedCount === 0)) {
              status = 1;
              (deps.writeStderr || process.stderr.write.bind(process.stderr))(
                `${command.label}: ${isControlPlaneDoctest ? 'no successful doctest result' : 'no executed passing tests'}; refusing invalid coverage evidence\n`,
              );
            }
          },
        }) || status;

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

module.exports = { parseCoverageCliArgs, buildCoverageFrontendCommand, buildCoverageFrontendPageRuntimeCommand, buildCoverageBackendCommands, buildCoverageBackendCleanupCommands, ensureCargoLlvmCovInstalled, runCoverage };
