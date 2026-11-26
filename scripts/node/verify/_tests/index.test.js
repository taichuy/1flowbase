const test = require('node:test');
const assert = require('node:assert/strict');

const { buildRepoCommands, main } = require('../index.js');

test('verify index dispatches repo subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['repo'], {
    runRepoImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, []);
});

test('verify index dispatches coverage subcommand with remaining args', async () => {
  let capturedArgv = null;

  const status = await main(['coverage', 'backend'], {
    runCoverageImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['backend']);
});

test('verify index dispatches backend consistency subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['backend-consistency'], {
    runBackendConsistencyImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, []);
});

test('verify index dispatches state protocol subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['state-protocols', '--model', '1flowbase'], {
    runStateProtocolsImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--model', '1flowbase']);
});

test('verify index dispatches console operation registry hygiene subcommand', async () => {
  let capturedArgv = null;

  const status = await main([
    'console-operation-registry-hygiene',
    '--compiled-inventory',
    'tmp/compiled.json',
  ], {
    runConsoleOperationRegistryHygieneImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--compiled-inventory', 'tmp/compiled.json']);
});

test('verify index rejects unknown subcommands', async () => {
  await assert.rejects(
    () => main(['unknown']),
    /Unknown verify command: unknown/u
  );
});

test('verify repo tooling target includes frontstage governance hygiene', () => {
  const commands = buildRepoCommands({
    repoRoot: '/repo-root',
    env: { PATH: process.env.PATH },
    target: 'tooling',
  });

  const command = commands.find(
    (item) => item.label === 'repo-frontstage-governance-hygiene'
  );

  assert.ok(command);
  assert.deepEqual(command.args, [
    '/repo-root/scripts/node/tooling.js',
    'frontstage-governance-hygiene',
  ]);
});

test('DV-F05 verify repo frontend includes the static dev experience gate', () => {
  const commands = buildRepoCommands({
    repoRoot: '/repo-root',
    env: { PATH: process.env.PATH },
    target: 'frontend',
  });

  const command = commands.find((item) => item.label === 'repo-dev-experience');
  assert.ok(command);
  assert.deepEqual(command.args, [
    '/repo-root/scripts/node/tooling.js',
    'dev-experience',
  ]);
});


test('three tiers reuse the existing scoped and release commands without layering full verify on PR', async () => {
  const { buildPrVerifyPlan } = require('../../gate-router/core.js');
  const options = { repoRoot: '/repo-root', env: { PATH: process.env.PATH } };
  const fast = buildRepoCommands({ ...options, target: 'level1' });
  assert.deepEqual(fast.map(({ label }) => label), ['repo-gate-router', 'repo-hygiene', 'repo-i18n-hygiene',
    'repo-vite-lazy-deps-gate', 'repo-backend-static', 'repo-backend-fmt', 'repo-fast-script-tests']);
  const pr = buildRepoCommands({ ...options, target: 'level2', plan: buildPrVerifyPlan({ changedFiles: ['web/app/src/shared/layout.tsx'] }) });
  assert.deepEqual(pr.map(({ label }) => label), ['repo-dev-experience', 'repo-vite-lazy-deps-gate', 'repo-frontend-pr']);
  const ci = buildRepoCommands({ ...options, target: 'level3' });
  assert.deepEqual(ci.map(({ label }) => label), ['ci-verify-repo', 'ci-coverage-all']);
  await main(['level2', '--base-ref', 'a'.repeat(40)], { runRepoImpl(args) {
    assert.deepEqual(args, ['level2', '--base-ref', 'a'.repeat(40)]);
  } });
});
