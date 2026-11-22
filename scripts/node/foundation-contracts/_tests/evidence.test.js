const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { buildFoundationPlan, buildContractReceipt, runFastPack } = require('../core.js');

const candidateSha = 'abcdef1234567890';
const passedOutput = 'running 2 tests\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n';

function withRun(t, foundation, output = foundation === 'native-react' ? 'Tests 2 passed (2)\n' : passedOutput, status = 0) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'foundation-execution-evidence-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const plan = buildFoundationPlan({ foundation });
  const calls = [];
  const receipt = runFastPack({
    repoRoot, candidateSha, plan, foundation,
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args });
      fs.writeSync(options.stdio[1], output);
      return { status };
    },
  });
  return { repoRoot, plan, receipt, calls };
}

test('Cargo fast packs require executed tests and preserve command logs and counts', (t) => {
  for (const foundation of ['mcp-gateway', 'application-backend']) {
    const { repoRoot, plan, receipt } = withRun(t, foundation);
    assert.equal(receipt.status, 'passed');
    assert.equal(receipt.commands[0].passedCount, 2);
    assert.equal(receipt.commands[0].failedCount, 0);
    assert.equal(fs.readFileSync(path.join(repoRoot, receipt.commands[0].logPath), 'utf8'), passedOutput);
    assert.equal(buildContractReceipt({ candidateSha, plan, componentResults: [receipt] }).status, 'passed');
  }
});

test('zero-test, missing-summary and failed-test Cargo output cannot produce green evidence', (t) => {
  for (const output of [
    'test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out;\n',
    'Finished compiling successfully\n',
    'test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out;\n',
  ]) {
    const { repoRoot, plan, receipt } = withRun(t, 'mcp-gateway', output);
    assert.equal(receipt.status, 'failed');
    assert.match(receipt.errors[0], /no executed passing tests|missing or invalid executed-test summary|test execution failed/u);
    assert.equal(fs.readFileSync(path.join(repoRoot, receipt.commands[0].logPath), 'utf8'), output);
    assert.equal(buildContractReceipt({ candidateSha, plan, componentResults: [receipt] }).status, 'failed');
  }
});

test('failed execution preserves real passing counts instead of claiming zero execution', (t) => {
  const output = 'test result: FAILED. 313 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out;\n';
  const { plan, receipt } = withRun(t, 'ai-gateway', '# pass 313\n# fail 1\n');
  assert.equal(receipt.status, 'failed');
  assert.equal(receipt.commands[0].passedCount, 313);
  assert.equal(receipt.commands[0].failedCount, 1);
  assert.match(receipt.errors[0], /test execution failed: 313 passed; 1 failed/u);
  assert.doesNotMatch(receipt.errors.join(' '), /no executed|zero execution/u);
  const aggregate = buildContractReceipt({ candidateSha, plan, componentResults: [receipt] });
  assert.equal(aggregate.status, 'failed');
  assert.doesNotMatch(aggregate.errors.join(' '), /no executed|zero execution/u);
  const cargo = withRun(t, 'mcp-gateway', output, 101).receipt;
  assert.equal(cargo.commands[0].exitCode, 101);
  assert.match(cargo.errors[0], /313 passed; 1 failed/u);
});

test('aggregation refuses omitted, incomplete, duplicate and unsuccessful required command evidence', (t) => {
  const { plan, receipt } = withRun(t, 'native-react', 'Tests 2 passed (2)\n');
  assert.equal(receipt.status, 'passed');
  const mutations = [
    (value) => { delete value.executedPack; delete value.commands; },
    (value) => { value.executedPack = []; value.commands = []; },
    (value) => { value.executedPack.pop(); value.commands.pop(); },
    (value) => { value.executedPack[1] = value.executedPack[0]; },
    (value) => { value.commands[1] = value.commands[0]; },
    (value) => { value.commands[0].exitCode = 1; },
    (value) => { value.commands[0].error = 'fixture rejected'; },
    (value) => { delete value.commands[0].logPath; },
    (value) => { value.commands[0].id = 'unplanned-command'; },
    (value) => { value.candidateSha = 'stale'; },
  ];
  for (const mutate of mutations) {
    const invalid = structuredClone(receipt);
    mutate(invalid);
    const result = buildContractReceipt({ candidateSha, plan, componentResults: [invalid] });
    assert.equal(result.status, 'failed');
    assert.ok(result.errors.length > 0);
    if (!invalid.executedPack) assert.deepEqual(result.foundations[0].executedPack, []);
  }
  assert.equal(buildContractReceipt({ candidateSha, plan, componentResults: [receipt, receipt] }).status, 'failed');
});

test('aggregator independently rejects invented Cargo execution and keeps unselected foundations legal', (t) => {
  const { plan, receipt } = withRun(t, 'mcp-gateway');
  for (const counts of [
    { passedCount: 0, failedCount: 0 },
    { passedCount: null, failedCount: null },
    { passedCount: 1, failedCount: 1 },
  ]) {
    const invalid = structuredClone(receipt);
    Object.assign(invalid.commands[0], counts);
    assert.equal(buildContractReceipt({ candidateSha, plan, componentResults: [invalid] }).status, 'failed');
  }
  const missing = buildContractReceipt({ candidateSha, plan, componentResults: [] });
  assert.equal(missing.status, 'failed');
  assert.deepEqual(missing.foundations[0].executedPack, []);
  const unselected = buildFoundationPlan({ changedFiles: ['README.md'] });
  assert.equal(buildContractReceipt({ candidateSha, plan: unselected, componentResults: [] }).status, 'passed');
});

test('fast pack stops at the failed command and closes its log when spawn throws', (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'foundation-spawn-error-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const plan = buildFoundationPlan({ foundation: 'native-react' });
  let logFd;
  const receipt = runFastPack({
    repoRoot, candidateSha, plan, foundation: 'native-react',
    spawnSyncImpl(command, args, options) {
      logFd = options.stdio[1];
      throw new Error('controlled spawn failure');
    },
  });
  assert.equal(receipt.status, 'failed');
  assert.equal(receipt.commands.length, 1);
  assert.match(receipt.errors[0], /controlled spawn failure/u);
  assert.throws(() => fs.fstatSync(logFd), /EBADF/u);
  assert.equal(buildContractReceipt({ candidateSha, plan, componentResults: [receipt] }).status, 'failed');
});

test('runFastPack applies verify runtime Cargo jobs, test threads, and test-profile incremental env', (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'foundation-cargo-budget-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const plan = buildFoundationPlan({ foundation: 'mcp-gateway' });
  const spawned = [];

  const receipt = runFastPack({
    repoRoot,
    candidateSha,
    plan,
    foundation: 'mcp-gateway',
    env: { CARGO_INCREMENTAL: '1', EXISTING_VAR: 'kept' },
    runtimeConfig: {
      backend: { cargoJobs: 3, cargoTestThreads: 2, incremental: true },
    },
    spawnSyncImpl(command, args, options) {
      spawned.push({ command, args, env: options.env });
      fs.writeSync(options.stdio[1], passedOutput);
      return { status: 0 };
    },
  });

  assert.equal(receipt.status, 'passed');
  assert.equal(spawned.length, 1);
  assert.deepEqual(spawned[0].args, [
    'test',
    '-p',
    'api-server',
    '--test',
    'api_private_contracts',
    'mcp_protocol_routes',
    '--jobs',
    '3',
    '--',
    '--test-threads=2',
  ]);
  assert.equal(spawned[0].env.CARGO_BUILD_JOBS, '3');
  assert.equal(spawned[0].env.CARGO_PROFILE_TEST_INCREMENTAL, 'true');
  assert.equal(Object.hasOwn(spawned[0].env, 'CARGO_INCREMENTAL'), false);
  assert.equal(spawned[0].env.EXISTING_VAR, 'kept');
});
