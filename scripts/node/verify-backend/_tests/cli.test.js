const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { buildCommands, main, parseBackendCliArgs } = require('../../verify-backend.js');

function getExpectedParallelism() {
  return typeof os.availableParallelism === 'function'
    ? os.availableParallelism()
    : os.cpus().length;
}

test('verify-backend buildCommands uses independent cargo jobs and cargo test threads', () => {
  assert.deepEqual(buildCommands({ cargoJobs: 4, cargoTestThreads: 2, repoRoot: '/repo-root', env: {} }), [
    {
      label: 'rust-backend-static-gate',
      command: process.execPath,
      args: ['/repo-root/scripts/node/tooling.js', 'check-rust-backend'],
      cwd: '/repo-root',
    },
    {
      label: 'cargo-fmt',
      command: 'cargo',
      args: ['fmt', '--all', '--check'],
      cwd: 'api',
      env: {
        CARGO_BUILD_JOBS: '4',
      },
    },
    {
      label: 'cargo-clippy',
      command: 'cargo',
      args: ['clippy', '--workspace', '--all-targets', '--jobs', '4', '--', '-D', 'warnings'],
      cwd: 'api',
      env: {
        CARGO_BUILD_JOBS: '4',
        CARGO_INCREMENTAL: '0',
      },
    },
    {
      label: 'cargo-test-image-llm-vision-control-plane-user-media-fallback',
      command: 'cargo',
      args: [
        'test',
        '-p',
        'control-plane',
        '--jobs',
        '4',
        'orchestration_runtime_textualizes_user_media_when_selected_model_is_not_multimodal',
        '--',
        '--test-threads=2',
      ],
      cwd: 'api',
      env: {
        CARGO_BUILD_JOBS: '4',
        CARGO_INCREMENTAL: '0',
      },
    },
    {
      label: 'cargo-test-image-llm-vision-control-plane-multimodal-preserves-media',
      command: 'cargo',
      args: [
        'test',
        '-p',
        'control-plane',
        '--jobs',
        '4',
        'orchestration_runtime_keeps_user_media_when_configured_model_supports_multimodal',
        '--',
        '--test-threads=2',
      ],
      cwd: 'api',
      env: {
        CARGO_BUILD_JOBS: '4',
        CARGO_INCREMENTAL: '0',
      },
    },
    {
      label: 'cargo-test-image-llm-vision-control-plane-routed-media-guidance',
      command: 'cargo',
      args: [
        'test',
        '-p',
        'control-plane',
        '--jobs',
        '4',
        'orchestration_runtime_textualizes_routed_media_as_retry_guidance_for_text_models',
        '--',
        '--test-threads=2',
      ],
      cwd: 'api',
      env: {
        CARGO_BUILD_JOBS: '4',
        CARGO_INCREMENTAL: '0',
      },
    },
    {
      label: 'cargo-test-image-llm-vision-runtime-visible-media-tool',
      command: 'cargo',
      args: [
        'test',
        '-p',
        'orchestration-runtime',
        '--jobs',
        '4',
        'visible_internal_llm_tool_media',
        '--',
        '--test-threads=2',
      ],
      cwd: 'api',
      env: {
        CARGO_BUILD_JOBS: '4',
        CARGO_INCREMENTAL: '0',
      },
    },
    {
      label: 'cargo-test',
      command: 'cargo',
      args: ['test', '--workspace', '--jobs', '4', '--', '--test-threads=2'],
      cwd: 'api',
      env: {
        CARGO_BUILD_JOBS: '4',
        CARGO_INCREMENTAL: '0',
      },
    },
    {
      label: 'cargo-check',
      command: 'cargo',
      args: ['check', '--workspace', '--jobs', '4'],
      cwd: 'api',
      env: {
        CARGO_BUILD_JOBS: '4',
        CARGO_INCREMENTAL: '0',
      },
    },
  ]);
});

test('verify-backend can build targeted shard commands for parallel CI', () => {
  assert.deepEqual(parseBackendCliArgs(['clippy', 'core-libs']), {
    help: false,
    target: 'clippy',
    shard: 'core-libs',
  });

  assert.deepEqual(
    buildCommands({
      cargoJobs: 4,
      cargoTestThreads: 2,
      repoRoot: '/repo-root',
      env: {},
      target: 'clippy',
      shard: 'core-libs',
    }).map((command) => ({ label: command.label, args: command.args })),
    [
      {
        label: 'cargo-clippy-core-libs',
        args: [
          'clippy',
          '--package',
          'domain',
          '--package',
          'access-control',
          '--package',
          'observability',
          '--package',
          'runtime-profile',
          '--package',
          'plugin-framework',
          '--all-targets',
          '--jobs',
          '4',
          '--',
          '-D',
          'warnings',
        ],
      },
    ]
  );

  assert.deepEqual(parseBackendCliArgs(['test', 'api-server']), {
    help: false,
    target: 'test',
    shard: 'api-server',
  });

  assert.deepEqual(
    buildCommands({
      cargoJobs: 4,
      cargoTestThreads: 2,
      repoRoot: '/repo-root',
      env: {},
      target: 'test',
      shard: 'api-server',
    }).map((command) => ({ label: command.label, args: command.args })),
    [
      {
        label: 'cargo-test-api-server',
        args: [
          'test',
          '--package',
          'api-server',
          '--jobs',
          '4',
          '--',
          '--test-threads=2',
        ],
      },
    ]
  );

  assert.deepEqual(parseBackendCliArgs(['image-llm-vision']), {
    help: false,
    target: 'image-llm-vision',
    shard: null,
  });

  assert.deepEqual(
    buildCommands({
      cargoJobs: 4,
      cargoTestThreads: 2,
      repoRoot: '/repo-root',
      env: {},
      target: 'image-llm-vision',
    }).map((command) => ({ label: command.label, args: command.args })),
    [
      {
        label: 'cargo-test-image-llm-vision-control-plane-user-media-fallback',
        args: [
          'test',
          '-p',
          'control-plane',
          '--jobs',
          '4',
          'orchestration_runtime_textualizes_user_media_when_selected_model_is_not_multimodal',
          '--',
          '--test-threads=2',
        ],
      },
      {
        label: 'cargo-test-image-llm-vision-control-plane-multimodal-preserves-media',
        args: [
          'test',
          '-p',
          'control-plane',
          '--jobs',
          '4',
          'orchestration_runtime_keeps_user_media_when_configured_model_supports_multimodal',
          '--',
          '--test-threads=2',
        ],
      },
      {
        label: 'cargo-test-image-llm-vision-control-plane-routed-media-guidance',
        args: [
          'test',
          '-p',
          'control-plane',
          '--jobs',
          '4',
          'orchestration_runtime_textualizes_routed_media_as_retry_guidance_for_text_models',
          '--',
          '--test-threads=2',
        ],
      },
      {
        label: 'cargo-test-image-llm-vision-runtime-visible-media-tool',
        args: [
          'test',
          '-p',
          'orchestration-runtime',
          '--jobs',
          '4',
          'visible_internal_llm_tool_media',
          '--',
          '--test-threads=2',
        ],
      },
    ]
  );

  assert.deepEqual(parseBackendCliArgs(['official-i18n-seed']), {
    help: false,
    target: 'official-i18n-seed',
    shard: null,
  });

  assert.deepEqual(
    buildCommands({
      cargoJobs: 4,
      cargoTestThreads: 2,
      repoRoot: '/repo-root',
      env: {},
      target: 'official-i18n-seed',
    }).map((command) => ({ label: command.label, args: command.args })),
    [
      {
        label: 'cargo-test-official-i18n-seed-integrity',
        args: [
          'test',
          '-p',
          'api-server',
          '--jobs',
          '4',
          'ac_002_decodes_digest_verified_build_time_official_seed',
          '--',
          '--test-threads=2',
        ],
      },
      {
        label: 'cargo-test-official-i18n-seed-consumer-provenance',
        args: [
          'test',
          '-p',
          'api-server',
          '--jobs',
          '4',
          'ac_001_ac_002_ac_006_ac_010_ac_012_official_seed_covers_frozen_consumers',
          '--',
          '--test-threads=2',
        ],
      },
    ]
  );
});

test('main routes backend verification through the heavy managed gate', async () => {
  let capturedOptions = null;

  const status = await main([], {
    repoRoot: '/repo-root',
    env: {},
    runtimeConfig: {
      backend: {
        cargoJobs: 3,
        cargoTestThreads: 1,
      },
      locks: {
        waitTimeoutMinutes: 30,
        waitTimeoutMs: 30 * 60 * 1000,
        pollIntervalMs: 5000,
      },
    },
    writeStdout() {},
    writeStderr() {},
    managedRunnerImpl(options) {
      capturedOptions = options;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedOptions.lockMode, 'heavy');
  assert.equal(capturedOptions.scope, 'verify-backend');
  assert.equal(capturedOptions.commandDisplay, 'node scripts/node/verify-backend.js');
  assert.deepEqual(capturedOptions.commands, buildCommands({
    cargoJobs: 3,
    cargoTestThreads: 1,
    repoRoot: '/repo-root',
    env: {},
  }));
});

test('verify-backend uses all available CPU for cargo jobs and tests in CI', async () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-backend-'));
  const fakeBinDir = path.join(tempDir, 'bin');
  const logPath = path.join(tempDir, 'cargo.log');
  const warningOutputDir = path.join(tempDir, 'warnings');
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const scriptPath = path.join(repoRoot, 'scripts', 'node', 'verify-backend.js');
  const fakeCargoPath = path.join(fakeBinDir, 'cargo');
  const expectedParallelism = getExpectedParallelism();

  fs.mkdirSync(fakeBinDir, { recursive: true });
  fs.writeFileSync(
    fakeCargoPath,
    [
      '#!/usr/bin/env bash',
      'printf "%s\\n" "$*" >> "$VERIFY_BACKEND_LOG"',
      'printf "warning: backend advisory\\n" >&2',
      'exit 0',
    ].join('\n')
  );
  fs.chmodSync(fakeCargoPath, 0o755);

  const result = spawnSync('node', [scriptPath], {
    cwd: repoRoot,
    env: {
      ...process.env,
      CI: 'true',
      PATH: `${fakeBinDir}:${process.env.PATH ?? ''}`,
      VERIFY_BACKEND_LOG: logPath,
      ONEFLOWBASE_WARNING_OUTPUT_DIR: warningOutputDir,
    },
    encoding: 'utf8',
  });

  assert.equal(result.status, 0, result.stderr);

  const invocations = fs
    .readFileSync(logPath, 'utf8')
    .trim()
    .split('\n');

  assert.equal(invocations.length, 8);
  assert.match(invocations[1], new RegExp(`clippy --workspace --all-targets --jobs ${expectedParallelism} -- -D warnings`));
  assert.match(invocations[2], new RegExp(`test -p control-plane --jobs ${expectedParallelism} orchestration_runtime_textualizes_user_media_when_selected_model_is_not_multimodal -- --test-threads=${expectedParallelism}`));
  assert.match(invocations[3], new RegExp(`test -p control-plane --jobs ${expectedParallelism} orchestration_runtime_keeps_user_media_when_configured_model_supports_multimodal -- --test-threads=${expectedParallelism}`));
  assert.match(invocations[4], new RegExp(`test -p control-plane --jobs ${expectedParallelism} orchestration_runtime_textualizes_routed_media_as_retry_guidance_for_text_models -- --test-threads=${expectedParallelism}`));
  assert.match(invocations[5], new RegExp(`test -p orchestration-runtime --jobs ${expectedParallelism} visible_internal_llm_tool_media -- --test-threads=${expectedParallelism}`));
  assert.match(invocations[6], new RegExp(`test --workspace --jobs ${expectedParallelism} -- --test-threads=${expectedParallelism}`));
  assert.match(invocations[7], new RegExp(`check --workspace --jobs ${expectedParallelism}`));

  const warningLogPath = path.join(warningOutputDir, 'verify-backend.warnings.log');
  assert.equal(fs.existsSync(warningLogPath), true);
  assert.match(fs.readFileSync(warningLogPath, 'utf8'), /warning: backend advisory/u);
});
