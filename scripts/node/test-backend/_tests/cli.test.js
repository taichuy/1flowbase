const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { buildCommands, main } = require('../../test-backend.js');

test('test-backend buildCommands uses independent cargo jobs and cargo test threads', () => {
  assert.deepEqual(buildCommands({ cargoJobs: 4, cargoTestThreads: 2, repoRoot: '/repo-root', env: {} }), [
    {
      label: 'rust-backend-static-gate',
      command: process.execPath,
      args: ['/repo-root/scripts/node/tooling.js', 'check-rust-backend'],
      cwd: '/repo-root',
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
  ]);
});

test('test-backend buildCommands enables incremental compilation when configured', () => {
  const commands = buildCommands({
    cargoJobs: 2,
    cargoTestThreads: 1,
    incremental: true,
    repoRoot: '/repo-root',
    env: {},
  });

  assert.deepEqual(commands[1].env, {
    CARGO_BUILD_JOBS: '2',
    CARGO_INCREMENTAL: '1',
  });
  assert.deepEqual(commands.at(-1).env, {
    CARGO_BUILD_JOBS: '2',
    CARGO_INCREMENTAL: '1',
  });
});

test('main prints help without running the backend gate', async () => {
  let output = '';
  let ran = false;

  const status = await main(['--help'], {
    writeStdout(text) {
      output += text;
    },
    managedRunnerImpl() {
      ran = true;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(ran, false);
  assert.match(output, /Usage: node scripts\/node\/test-backend\.js/u);
});

test('main routes backend test execution through the heavy managed gate', async () => {
  let capturedOptions = null;

  const status = await main([], {
    repoRoot: '/repo-root',
    env: {},
    runtimeConfig: {
      backend: {
        cargoJobs: 5,
        cargoTestThreads: 2,
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
  assert.equal(capturedOptions.scope, 'test-backend');
  assert.equal(capturedOptions.commandDisplay, 'node scripts/node/test-backend.js');
  assert.deepEqual(capturedOptions.commands, buildCommands({
    cargoJobs: 5,
    cargoTestThreads: 2,
    repoRoot: '/repo-root',
    env: {},
  }));
});

test('test-backend main writes advisory warning output under tmp/test-governance', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-test-backend-'));
  const calls = [];

  const status = await main([], {
    repoRoot,
    env: {},
    runtimeConfig: {
      backend: {
        cargoJobs: 1,
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
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });

      return {
        status: 0,
        stdout: '',
        stderr: 'warning: cargo test advisory\n',
      };
    },
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 6);
  assert.deepEqual(calls[1].args, [
    'test',
    '-p',
    'control-plane',
    '--jobs',
    '1',
    'orchestration_runtime_textualizes_user_media_when_selected_model_is_not_multimodal',
    '--',
    '--test-threads=1',
  ]);
  assert.deepEqual(calls[4].args, [
    'test',
    '-p',
    'orchestration-runtime',
    '--jobs',
    '1',
    'visible_internal_llm_tool_media',
    '--',
    '--test-threads=1',
  ]);
  assert.deepEqual(calls[5].args, ['test', '--workspace', '--jobs', '1', '--', '--test-threads=1']);

  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'test-backend.warnings.log');
  assert.equal(fs.existsSync(warningLogPath), true);
  assert.match(fs.readFileSync(warningLogPath, 'utf8'), /warning: cargo test advisory/u);
});
