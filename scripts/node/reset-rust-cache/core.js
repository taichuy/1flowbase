const { spawn } = require('node:child_process');
const path = require('node:path');

const {
  runBuildCacheCleanup,
} = require('../clean-build-cache/core.js');
const {
  loadVerifyRuntimeConfig,
} = require('../testing/verify-runtime.js');

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function buildCargoEnv({ cargoJobs, incremental, testProfile = false }) {
  const env = {
    CARGO_BUILD_JOBS: String(cargoJobs),
    CARGO_INCREMENTAL: incremental ? '1' : '0',
  };

  if (testProfile) {
    env.CARGO_PROFILE_TEST_DEBUG = '0';
  }

  return env;
}

function buildRustWarmupPlan({ cargoJobs, incremental }) {
  const devEnv = buildCargoEnv({ cargoJobs, incremental });

  return [
    {
      label: 'workspace dev targets',
      args: [
        'build',
        '--manifest-path',
        'api/Cargo.toml',
        '--workspace',
        '--all-targets',
        '--locked',
      ],
      env: devEnv,
    },
    {
      label: 'api-server dev-up target',
      args: [
        'build',
        '--manifest-path',
        'api/Cargo.toml',
        '-p',
        'api-server',
        '--bin',
        'api-server',
        '--locked',
      ],
      env: devEnv,
    },
    {
      label: 'workspace test targets',
      args: [
        'test',
        '--manifest-path',
        'api/Cargo.toml',
        '--workspace',
        '--all-targets',
        '--no-run',
        '--locked',
      ],
      env: buildCargoEnv({ cargoJobs, incremental, testProfile: true }),
    },
  ];
}

async function cleanupBackendCache({ repoRoot, writeStdout }) {
  return runBuildCacheCleanup({
    repoRoot,
    options: {
      dryRun: false,
      help: false,
      scope: 'backend',
    },
    writeStdout,
  });
}

function runCargo({ repoRoot, args, env }) {
  return new Promise((resolve, reject) => {
    const child = spawn('cargo', args, {
      cwd: repoRoot,
      env: { ...process.env, ...env },
      stdio: 'inherit',
    });

    child.once('error', reject);
    child.once('exit', (code, signal) => {
      if (signal) {
        reject(new Error(`cargo 被信号 ${signal} 终止`));
        return;
      }
      resolve(code ?? 1);
    });
  });
}

async function runRustCacheReset({
  repoRoot = getRepoRoot(),
  loadRuntimeConfigImpl = loadVerifyRuntimeConfig,
  cleanupBackendCacheImpl = cleanupBackendCache,
  runCargoImpl = runCargo,
  writeStdout = (text) => process.stdout.write(text),
} = {}) {
  const runtimeConfig = loadRuntimeConfigImpl({ repoRoot });
  const { cargoJobs, incremental } = runtimeConfig.backend;
  const plan = buildRustWarmupPlan({ cargoJobs, incremental });

  writeStdout('[1flowbase-reset-rust-cache] 清理 api/target 并停止 api-server。\n');
  const cleanupStatus = await cleanupBackendCacheImpl({ repoRoot, writeStdout });
  if (cleanupStatus !== undefined && cleanupStatus !== 0) {
    return cleanupStatus;
  }

  for (const stage of plan) {
    writeStdout(
      `[1flowbase-reset-rust-cache] 预热 ${stage.label} `
      + `(jobs=${stage.env.CARGO_BUILD_JOBS}, incremental=${stage.env.CARGO_INCREMENTAL})。\n`,
    );
    const status = await runCargoImpl({ repoRoot, ...stage });
    if (status !== 0) {
      writeStdout(
        `[1flowbase-reset-rust-cache] ${stage.label} 失败（exit=${status}），停止后续预热。\n`,
      );
      return status;
    }
  }

  writeStdout('[1flowbase-reset-rust-cache] Rust 缓存清理与全量预热完成。\n');
  return 0;
}

module.exports = {
  buildRustWarmupPlan,
  cleanupBackendCache,
  getRepoRoot,
  runCargo,
  runRustCacheReset,
};
