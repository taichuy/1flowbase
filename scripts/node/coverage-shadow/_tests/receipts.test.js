const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {
  collectNextestTestIds,
  mergeApiServerShadow,
  parseNextestSummary,
  runApiServerShard,
  validateCoverageSummary,
  validateShardInventories,
  validateShardMetadata,
} = require('../core.js');

const llvmEnvironment = "export RUSTC_WRAPPER='/bin/cov'\nexport CARGO_LLVM_COV='1'\n";
const sha = 'a'.repeat(40);

function inventory(testIds, ignoredIds = []) {
  return {
    'rust-suites': {
      'api-server::lib': {
        testcases: Object.fromEntries([
          ...testIds.map((id) => [id, { ignored: false, 'filter-match': { status: 'matches' } }]),
          // nextest filters ignored before partitions, so this appears in every shard.
          ...ignoredIds.map((id) => [id, { ignored: true, 'filter-match': { status: 'mismatch', reason: 'ignored' } }]),
        ]),
      },
    },
  };
}

function metadata(index = 1) {
  return {
    package: 'api-server', partition: `hash:${index}/4`, sha,
    testCount: 1, ignoredCount: 1, exitCode: 0, passedCount: 1, failedCount: 0, profileCount: 1,
  };
}

function temporaryRepo(t) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coverage-receipts-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  fs.mkdirSync(path.join(repoRoot, 'api'));
  return repoRoot;
}

function summary(covered = 8, count = 10) {
  return { data: [{
    totals: { lines: { count, covered } },
    files: [{ filename: '/repo/api/apps/api-server/src/lib.rs', summary: { lines: { count, covered } } }],
  }] };
}

function prepareMerge(t) {
  const repoRoot = temporaryRepo(t);
  const root = path.join(repoRoot, 'tmp/test-governance/coverage-shadow/api-server');
  const directory = path.join(root, 'downloaded');
  fs.mkdirSync(directory, { recursive: true });
  for (let index = 1; index <= 4; index += 1) {
    fs.writeFileSync(path.join(directory, `inventory-${index}.json`), JSON.stringify(inventory([String(index)], ['later'])));
    fs.writeFileSync(path.join(directory, `metadata-${index}.json`), JSON.stringify(metadata(index)));
    fs.writeFileSync(path.join(directory, `shard-${index}-1.profraw`), `profile-${index}`);
  }
  const finalPath = path.join(repoRoot, 'tmp/test-governance/coverage/backend/api-server.json');
  return { repoRoot, root, directory, finalPath };
}

test('nextest Summary requires one complete nonzero passing execution, preserving slow/flaky annotations', () => {
  // Shape observed in the saved API coverage job: 429 passed (4 slow), 1454 skipped.
  assert.deepEqual(parseNextestSummary(
    '\u001b[32;1m     Summary\u001b[0m [1501.465s] 429 tests run: 429 passed (4 slow), 1454 skipped\n'
  ), { testCount: 429, passedCount: 429, failedCount: 0 });
  assert.deepEqual(parseNextestSummary('Summary [1.0s] 2 tests run: 2 passed (1 flaky, 1 slow), 9 skipped\n'),
    { testCount: 2, passedCount: 2, failedCount: 0 });
  for (const output of [
    '',
    'test result: ok. 1 passed; 0 failed;',
    'Summary [1.0s] 0 tests run: 0 passed, 15 skipped',
    'Summary [1.0s] 2 tests run: 1 passed, 1 failed',
    'Summary [1.0s] 2 tests run: 1 passed, 1 timed out',
    'Summary [1.0s] 1 test run: 1 passed\nSummary [2.0s] 1 test run: 1 passed',
  ]) assert.throws(() => parseNextestSummary(output), /nextest/u, output);
});

test('ignored identity is separate from runnable partitions and never inferred from skipped', () => {
  const full = inventory(['a', 'b'], ['later']);
  const first = inventory(['a'], ['later']);
  first['rust-suites']['api-server::lib'].testcases.b = {
    ignored: false, 'filter-match': { status: 'mismatch', reason: 'partition' },
  };
  assert.deepEqual(collectNextestTestIds(first), ['api-server::lib::a']);
  assert.deepEqual(collectNextestTestIds(first, { ignored: true }), ['api-server::lib::later']);
  assert.deepEqual(validateShardInventories({ fullInventory: full, shardInventories: [first, inventory(['b'], ['later'])] }),
    { fullCount: 2, ignoredCount: 1, shardCount: 2 });
  assert.throws(() => validateShardInventories({ fullInventory: full, shardInventories: [first, inventory(['b'])] }), /ignored inventory/u);
  for (const [ids, pattern] of [[['a', 'a'], /duplicate=1, missing=1/u], [['a'], /missing=1/u], [['a', 'b', 'extra'], /extra=1/u]]) {
    assert.throws(() => validateShardInventories({
      fullInventory: full, shardInventories: ids.map((id) => inventory([id], ['later'])),
    }), pattern);
  }
  assert.throws(() => validateShardInventories({ fullInventory: inventory([], ['later']), shardInventories: [] }), /no runnable tests/u);
});

test('metadata rejects missing or wrong package, partition, SHA, execution, inventory and profile counts', (t) => {
  const { directory } = prepareMerge(t);
  const valid = metadata();
  const options = { inventory: inventory(['1'], ['later']), shardIndex: 1, shardCount: 4, expectedSha: sha, directory };
  assert.deepEqual(validateShardMetadata({ ...options, metadata: valid }), ['shard-1-1.profraw']);
  for (const field of Object.keys(valid)) {
    const incomplete = { ...valid };
    delete incomplete[field];
    assert.throws(() => validateShardMetadata({ ...options, metadata: incomplete }), /coverage shard/u, field);
  }
  for (const patch of [
    { package: 'control-plane' }, { partition: 'hash:2/4' }, { sha: 'old-sha' },
    { testCount: 0 }, { testCount: 2 }, { ignoredCount: 0 },
    { exitCode: 1 }, { passedCount: 0 }, { passedCount: 2 }, { failedCount: 1 },
    { profileCount: 0 }, { profileCount: 2 },
  ]) assert.throws(() => validateShardMetadata({ ...options, metadata: { ...valid, ...patch } }), /coverage shard/u, JSON.stringify(patch));
  assert.throws(() => validateShardMetadata({ ...options, expectedSha: '', metadata: { ...valid, sha: '' } }), /frozen SHA/u);
  assert.throws(() => validateShardMetadata({ ...options, inventory: inventory([], ['later']), metadata: { ...valid, testCount: 0, passedCount: 0 } }), /nonzero/u);
  fs.writeFileSync(path.join(directory, 'shard-1-1.profraw'), '');
  assert.throws(() => validateShardMetadata({ ...options, metadata: valid }), /empty or invalid profile/u);
  fs.rmSync(path.join(directory, 'shard-1-1.profraw'));
  assert.throws(() => validateShardMetadata({ ...options, metadata: valid }), /no profraw/u);
});

test('successful shard persists raw output and metadata only after exact runnable execution', (t) => {
  const repoRoot = temporaryRepo(t);
  const rawStdout = 'test output\n';
  const rawStderr = '\u001b[32mSummary\u001b[0m [1.0s] 1 test run: 1 passed, 1454 skipped\n';
  let echoedStdout = '';
  let echoedStderr = '';
  const calls = [];
  runApiServerShard({
    repoRoot, shardIndex: 1, shardCount: 4, cargoTestThreads: 2,
    env: { QUALITY_GATE_TARGET_SHA: sha, CARGO_TARGET_DIR: '../tmp/existing-coverage-target', ONEFLOWBASE_VERIFY_LOCK_TOKEN: 'owned-lock' },
    runtimeConfig: { backend: { cargoJobs: 3, cargoTestThreads: 2 } },
    writeStdout(text) { echoedStdout += text; }, writeStderr(text) { echoedStderr += text; },
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      if (args[1] === 'show-env') return { status: 0, stdout: llvmEnvironment };
      if (args[1] === 'list') return { status: 0, stdout: JSON.stringify(inventory(['1'], ['later'])) };
      if (args[0] === 'build') return { status: 0, stdout: null, stderr: null };
      fs.writeFileSync(options.env.LLVM_PROFILE_FILE.replace('%p', '1').replace('%m', 'binary'), 'profile');
      fs.writeSync(options.stdio[1], rawStdout);
      fs.writeSync(options.stdio[2], rawStderr);
      return { status: 0, stdout: null, stderr: null };
    },
  });
  const directory = path.join(repoRoot, 'tmp/test-governance/coverage-shadow/api-server/shard-1');
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(directory, 'metadata-1.json'), 'utf8')), metadata());
  assert.equal(fs.readFileSync(path.join(directory, 'nextest-1.stdout.log'), 'utf8'), rawStdout);
  assert.equal(fs.readFileSync(path.join(directory, 'nextest-1.stderr.log'), 'utf8'), rawStderr);
  assert.equal(echoedStdout, rawStdout);
  assert.equal(echoedStderr, rawStderr);
  assert.equal(calls.length, 4);
  assert.deepEqual(calls[1].args.slice(0, 2), ['nextest', 'list']);
  assert.equal(calls[1].command, 'cargo');
  assert.equal(calls[2].args[0], 'build');
  assert.equal(calls[3].command, path.join(repoRoot, 'tmp/existing-coverage-target/debug/with-postgres-template'));
  assert.deepEqual(calls[3].args.slice(0, 4), ['--', 'cargo', 'nextest', 'run']);
  for (const call of calls.slice(1)) {
    assert.equal(call.options.env.CARGO_BUILD_JOBS, '3');
    assert.equal(call.options.env.CARGO_TARGET_DIR, '../tmp/existing-coverage-target');
    assert.equal(call.options.env.RUSTC_WRAPPER, '/bin/cov');
    assert.equal(call.options.env.CARGO_LLVM_COV, '1');
    assert.equal(call.options.env.ONEFLOWBASE_VERIFY_LOCK_TOKEN, 'owned-lock');
    assert.equal(call.options.env.POSTGRES_TEST_METRICS_DIR,
      path.join(repoRoot, 'tmp/test-governance/postgres-fixtures/coverage-shadow-api-server-1-of-4'));
    assert.equal(call.options.env.LLVM_PROFILE_FILE, path.join(directory, 'shard-1-%p-%m.profraw'));
  }
});

test('failed, empty or incomplete shard execution cannot leave a successful receipt', (t) => {
  for (const scenario of [
    { output: 'Summary [1s] 1 test run: 1 passed', exitCode: 7, pattern: /exit code 7/u },
    { output: '', pattern: /execution summary/u },
    { output: 'Summary [1s] 0 tests run: 0 passed', pattern: /nonzero/u },
    { output: 'Summary [1s] 2 tests run: 2 passed', pattern: /does not match its inventory/u },
    { output: 'Summary [1s] 1 test run: 0 passed, 1 failed', pattern: /nonzero/u },
    { output: 'Summary [1s] 1 test run: 1 passed', noProfile: true, pattern: /no profraw/u },
    { emptyInventory: true, pattern: /no runnable tests/u },
    { noSha: true, pattern: /frozen SHA/u },
    { buildStatus: 7, pattern: /exit code 7/u },
  ]) {
    const repoRoot = temporaryRepo(t);
    const directory = path.join(repoRoot, 'tmp/test-governance/coverage-shadow/api-server/shard-1');
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(path.join(directory, 'metadata-1.json'), JSON.stringify(metadata()));
    let executions = 0;
    assert.throws(() => runApiServerShard({
      repoRoot, shardIndex: 1, shardCount: 4, cargoTestThreads: 2,
      env: { QUALITY_GATE_TARGET_SHA: scenario.noSha ? '' : sha },
      writeStdout() {}, writeStderr() {},
      spawnSyncImpl(_command, args, options) {
        if (args[1] === 'show-env') return { status: 0, stdout: llvmEnvironment };
        if (args[1] === 'list') return { status: 0, stdout: JSON.stringify(inventory(scenario.emptyInventory ? [] : ['1'], ['later'])) };
        if (args[0] === 'build') return { status: scenario.buildStatus || 0, stdout: null, stderr: null };
        executions += 1;
        if (!scenario.noProfile) fs.writeFileSync(options.env.LLVM_PROFILE_FILE.replace('%p', '1').replace('%m', 'binary'), 'profile');
        fs.writeSync(options.stdio[2], scenario.output || '');
        return { status: scenario.exitCode || 0, stdout: null, stderr: null };
      },
    }), scenario.pattern);
    assert.equal(fs.existsSync(path.join(directory, 'metadata-1.json')), false);
    if (scenario.exitCode) assert.equal(fs.readFileSync(path.join(directory, 'nextest-1.stderr.log'), 'utf8'), scenario.output);
    if (scenario.buildStatus) assert.equal(executions, 0);
  }
});

test('merge refuses old or missing receipts and unexpected profiles before any Cargo command', (t) => {
  for (const mutate of [
    (directory) => fs.writeFileSync(path.join(directory, 'metadata-1.json'), JSON.stringify({ sha })),
    (directory) => fs.rmSync(path.join(directory, 'metadata-1.json')),
    (directory) => fs.writeFileSync(path.join(directory, 'shard-5-1.profraw'), 'unexpected'),
  ]) {
    const { repoRoot, directory } = prepareMerge(t);
    mutate(directory);
    let calls = 0;
    assert.throws(() => mergeApiServerShadow({
      repoRoot, shardCount: 4, env: { QUALITY_GATE_TARGET_SHA: sha },
      spawnSyncImpl() { calls += 1; throw new Error('unexpected Cargo call'); },
    }), /metadata|unexpected shard profiles/u);
    assert.equal(calls, 0);
  }
});

test('merge rejects duplicate inventories, stale report reuse and below-threshold coverage', (t) => {
  for (const mode of ['duplicate', 'stale', 'threshold']) {
    const { repoRoot, root, directory, finalPath } = prepareMerge(t);
    fs.mkdirSync(path.dirname(finalPath), { recursive: true });
    for (const file of [finalPath, path.join(root, 'api-server-merged.json'), path.join(root, 'equivalence.json')]) {
      fs.writeFileSync(file, JSON.stringify(summary()));
    }
    if (mode === 'duplicate') fs.writeFileSync(path.join(directory, 'inventory-2.json'), JSON.stringify(inventory(['1'], ['later'])));
    let reports = 0;
    assert.throws(() => mergeApiServerShadow({
      repoRoot, shardCount: 4, env: { QUALITY_GATE_TARGET_SHA: sha },
      spawnSyncImpl(_command, args) {
        if (args[1] === 'show-env') return { status: 0, stdout: llvmEnvironment };
        if (args[1] === 'list') return { status: 0, stdout: JSON.stringify(inventory(['1', '2', '3', '4'], ['later'])) };
        reports += 1;
        if (mode !== 'stale') fs.writeFileSync(args.at(-1), JSON.stringify(summary(5)));
        return { status: 0, stdout: '' };
      },
    }), mode === 'duplicate' ? /duplicate=1, missing=1/u : mode === 'stale' ? /api-server-merged.json/u : /below 60/u);
    assert.equal(reports, mode === 'duplicate' ? 0 : 1);
    assert.equal(fs.existsSync(finalPath), false);
    assert.equal(fs.existsSync(path.join(root, 'equivalence.json')), false);
  }
});

test('API coverage cannot pass with an empty or malformed owned line denominator', () => {
  for (const value of [summary(0, 0), summary(0, NaN), summary(11, 10)]) {
    assert.throws(() => validateCoverageSummary(value), /API-owned/u);
  }
});
