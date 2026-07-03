const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { buildCommands, main } = require('../../cli/test-contracts.js');

const CONTRACT_TEST_FILES = [
  'src/features/settings/api/_tests/settings-api/settings-api.test.ts',
  'src/style-boundary/_tests/registry.test.tsx',
  'src/features/agent-flow/_tests/llm/llm-model-provider-field.test.tsx',
];

test('buildCommands targets the shared model provider contract consumers', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(buildCommands({ repoRoot }), [
    {
      label: 'model-provider-contract-tests',
      command: 'pnpm',
      args: ['--dir', 'web/app', 'exec', 'vitest', 'run', ...CONTRACT_TEST_FILES],
      cwd: repoRoot,
    },
  ]);
});

test('main runs the contract gate and captures advisory output', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-test-contracts-'));
  const calls = [];

  const status = await main([], {
    repoRoot,
    env: {},
    writeStdout() {},
    writeStderr() {},
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });

      return {
        status: 0,
        stdout: '',
        stderr: 'warning: model-provider-contract advisory\n',
      };
    },
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 1);
  assert.deepEqual(
    calls[0].args,
    ['--dir', 'web/app', 'exec', 'vitest', 'run', ...CONTRACT_TEST_FILES]
  );

  const warningLogPath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'test-contracts.warnings.log'
  );
  assert.equal(fs.existsSync(warningLogPath), true);
  assert.match(fs.readFileSync(warningLogPath, 'utf8'), /model-provider-contract advisory/u);
});

test('main routes contract gate through the heavy lock', async () => {
  let capturedLockMode = null;

  const status = await main([], {
    repoRoot: '/repo-root',
    env: {},
    managedRunnerImpl(options) {
      capturedLockMode = options.lockMode;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedLockMode, 'heavy');
});
