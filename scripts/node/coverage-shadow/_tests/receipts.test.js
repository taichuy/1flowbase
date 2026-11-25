const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const {
  buildApiServerCoverageArchive,
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
    package: 'api-server', packages: ['api-server'], partition: `hash:${index}/4`, sha,
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
    { package: 'control-plane' }, { packages: ['api-server', 'api-host-kernel'] }, { packages: ['api-server', 'api-applications'] },
    { packages: ['api-server', 'api-console-settings'] }, { packages: ['api-server', 'api-mcp'] },
    { packages: ['api-server', 'api-plugins-models'] }, { packages: ['api-server', 'api-console-interaction'] }, { partition: 'hash:2/4' }, { sha: 'old-sha' },
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
  runApiServerShard({
    repoRoot, shardIndex: 1, shardCount: 4, cargoTestThreads: 2,
    env: { QUALITY_GATE_TARGET_SHA: sha },
    writeStdout(text) { echoedStdout += text; }, writeStderr(text) { echoedStderr += text; },
    spawnSyncImpl(_command, args, options) {
      if (args[1] === 'show-env') return { status: 0, stdout: llvmEnvironment };
      if (args[0] === 'clean') return { status: 0, stdout: '' };
      if (args[1] === 'list') return { status: 0, stdout: JSON.stringify(inventory(['1'], ['later'])) };
      fs.writeFileSync(options.env.LLVM_PROFILE_FILE.replace(/%\d*m/gu, 'binary_0'), 'profile');
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
});

test('buffered nextest-list failures expose bounded child diagnostics before rejecting the shard', (t) => {
  const repoRoot = temporaryRepo(t);
  const directory = path.join(repoRoot, 'tmp/test-governance/coverage-shadow/api-server/shard-1');
  const stdoutTail = '\nlist stdout evidence\n';
  const stderrTail = '\nerror[E0425]: compiler diagnostic evidence\n';
  const childScript = `process.stdout.write('界'.repeat(30000) + ${JSON.stringify(stdoutTail)});`
    + `process.stderr.write('界'.repeat(30000) + ${JSON.stringify(stderrTail)}); process.exitCode = 101;`;
  let echoedStdout = '';
  let echoedStderr = '';
  let executionStarted = false;
  assert.throws(() => runApiServerShard({
    repoRoot, shardIndex: 1, shardCount: 4, cargoTestThreads: 2,
    env: { QUALITY_GATE_TARGET_SHA: sha },
    writeStdout(text) { echoedStdout += text; }, writeStderr(text) { echoedStderr += text; },
    spawnSyncImpl(_command, args, options) {
      if (args[1] === 'show-env') return { status: 0, stdout: llvmEnvironment };
      if (args[0] === 'clean') return { status: 0, stdout: '' };
      if (args[1] === 'list') {
        // Exercise actual buffered child IO, without invoking Cargo or compiling anything.
        return spawnSync(process.execPath, ['-e', childScript], options);
      }
      executionStarted = true;
      throw new Error('listing failure must prevent test execution');
    },
  }), /nextest list .*failed with exit code 101/u);
  assert.ok(echoedStdout.endsWith(stdoutTail), 'failed buffered stdout must remain visible');
  assert.ok(echoedStderr.endsWith(stderrTail), 'failed buffered stderr must remain visible');
  for (const output of [echoedStdout, echoedStderr]) {
    assert.ok(Buffer.byteLength(output) <= 64 * 1024 + 128, 'failure diagnostics must be bounded');
  }
  assert.equal(executionStarted, false);
  assert.equal(fs.readFileSync(path.join(directory, 'nextest-list-1.stdout.log'), 'utf8'), '界'.repeat(30000) + stdoutTail);
  assert.equal(fs.readFileSync(path.join(directory, 'nextest-list-1.stderr.log'), 'utf8'), '界'.repeat(30000) + stderrTail);
  assert.equal(fs.existsSync(path.join(directory, 'metadata-1.json')), false);
  assert.equal(fs.existsSync(path.join(directory, 'inventory-1.json')), false);
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
  ]) {
    const repoRoot = temporaryRepo(t);
    const directory = path.join(repoRoot, 'tmp/test-governance/coverage-shadow/api-server/shard-1');
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(path.join(directory, 'metadata-1.json'), JSON.stringify(metadata()));
    assert.throws(() => runApiServerShard({
      repoRoot, shardIndex: 1, shardCount: 4, cargoTestThreads: 2,
      env: { QUALITY_GATE_TARGET_SHA: scenario.noSha ? '' : sha },
      writeStdout() {}, writeStderr() {},
      spawnSyncImpl(_command, args, options) {
        if (args[1] === 'show-env') return { status: 0, stdout: llvmEnvironment };
        if (args[0] === 'clean') return { status: 0, stdout: '' };
        if (args[1] === 'list') return { status: 0, stdout: JSON.stringify(inventory(scenario.emptyInventory ? [] : ['1'], ['later'])) };
        if (!scenario.noProfile) fs.writeFileSync(options.env.LLVM_PROFILE_FILE.replace(/%\d*m/gu, 'binary_0'), 'profile');
        fs.writeSync(options.stdio[2], scenario.output || '');
        return { status: scenario.exitCode || 0, stdout: null, stderr: null };
      },
    }), scenario.pattern);
    assert.equal(fs.existsSync(path.join(directory, 'metadata-1.json')), false);
    if (scenario.exitCode) assert.equal(fs.readFileSync(path.join(directory, 'nextest-1.stderr.log'), 'utf8'), scenario.output);
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

test('merge rejects duplicate inventories, stale reports, low coverage and retired source objects', (t) => {
  const errors = { duplicate: /duplicate=1, missing=1/u, stale: /api-server-merged.json/u,
    threshold: /below 60/u, retired: /sources absent from the candidate/u };
  for (const mode of Object.keys(errors)) {
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
        if (args[0] === 'clean') return { status: 0, stdout: '' };
        if (args[1] === 'list') return { status: 0, stdout: JSON.stringify(inventory(['1', '2', '3', '4'], ['later'])) };
        reports += 1;
        if (mode !== 'stale') fs.writeFileSync(args.at(-1), JSON.stringify(summary(mode === 'retired' ? 8 : 5)));
        return { status: 0, stdout: '' };
      },
    }), errors[mode]);
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

test('merge applies the same owner selection and stale-source rejection to every current API owner', (t) => {
  for (const owner of ['kernel', 'public-gateway', 'applications', 'console-settings', 'mcp', 'plugins-models', 'console-interaction']) {
    const { repoRoot, directory } = prepareMerge(t);
    const packageName = owner === 'kernel' ? 'api-host-kernel' : `api-${owner}`;
    const manifest = path.join(repoRoot, 'api/crates/api', owner, 'Cargo.toml');
    fs.mkdirSync(path.dirname(manifest), { recursive: true });
    fs.writeFileSync(manifest, '[package]\n');
    for (let index = 1; index <= 4; index += 1) {
      fs.writeFileSync(path.join(directory, `metadata-${index}.json`), JSON.stringify({ ...metadata(index), packages: ['api-server', packageName] }));
    }
    const selected = ['--package', 'api-server', '--package', packageName];
    const stale = `/repo/api/crates/api/${owner}/src/deleted.rs`;
    assert.throws(() => mergeApiServerShadow({
      repoRoot, shardCount: 4, env: { QUALITY_GATE_TARGET_SHA: sha },
      spawnSyncImpl(_command, args) {
        if (args[1] === 'show-env') return { status: 0, stdout: llvmEnvironment };
        assert.deepEqual(args.filter((_arg, i) => args[i - 1] === '--package' || _arg === '--package'), selected);
        if (args[0] === 'clean') return { status: 0, stdout: '' };
        if (args[1] === 'list') return { status: 0, stdout: JSON.stringify(inventory(['1', '2', '3', '4'], ['later'])) };
        const value = summary();
        value.data[0].files[0].filename = stale;
        fs.writeFileSync(args.at(-1), JSON.stringify(value));
        return { status: 0 };
      },
    }), /sources absent from the candidate/u);
  }
});


for (const owner of ['applications', 'console-settings', 'mcp', 'plugins-models', 'console-interaction']) {
  test(`${owner} shard provenance merges only with its exact current package identity and live source`, (t) => {
    const { repoRoot, root, directory, finalPath } = prepareMerge(t);
    const sources = ['api/apps/api-server/src/lib.rs', `api/crates/api/${owner}/src/lib.rs`];
    for (const file of sources) {
      fs.mkdirSync(path.dirname(path.join(repoRoot, file)), { recursive: true });
      fs.writeFileSync(path.join(repoRoot, file), 'pub fn live() {}\n');
    }
    fs.writeFileSync(path.join(repoRoot, `api/crates/api/${owner}/Cargo.toml`), '[package]\n');
    for (let index = 1; index <= 4; index += 1) {
      fs.writeFileSync(path.join(directory, `metadata-${index}.json`), JSON.stringify({ ...metadata(index), packages: ['api-server', `api-${owner}`] }));
    }
    const value = summary();
    value.data[0].files.push({ filename: `/repo/api/crates/api/${owner}/src/lib.rs`, summary: { lines: { count: 10, covered: 8 } } });
    mergeApiServerShadow({
      repoRoot, shardCount: 4, env: { QUALITY_GATE_TARGET_SHA: sha },
      spawnSyncImpl(_command, args) {
        if (args[1] === 'show-env') return { status: 0, stdout: llvmEnvironment };
        assert.deepEqual(args.filter((_arg, index) => args[index - 1] === '--package' || _arg === '--package'), ['--package', 'api-server', '--package', `api-${owner}`]);
        if (args[0] === 'clean') return { status: 0, stdout: '' };
        if (args[1] === 'list') return { status: 0, stdout: JSON.stringify(inventory(['1', '2', '3', '4'], ['later'])) };
        fs.writeFileSync(args.at(-1), JSON.stringify(value));
        return { status: 0 };
      },
    });
    const evidence = JSON.parse(fs.readFileSync(path.join(root, 'equivalence.json'), 'utf8'));
    assert.equal(evidence.sha, sha);
    assert.equal(evidence.coverage.lineCoveragePercent, 80);
    assert.equal(evidence.shards.every((shard) => JSON.stringify(shard.packages) === JSON.stringify(['api-server', `api-${owner}`])), true);
    assert.deepEqual(JSON.parse(fs.readFileSync(finalPath, 'utf8')), value);
  });
}


function archiveFixture(t) {
  const { repoRoot, root, directory, finalPath } = prepareMerge(t);
  const archiveDir = path.join(root, 'build');
  const archivePath = path.join(archiveDir, 'api-server-nextest.tar.zst');
  const env = { QUALITY_GATE_TARGET_SHA: sha };
  const calls = [];
  let currentSha = sha;
  let result = summary();
  const source = path.join(repoRoot, 'api/apps/api-server/src/lib.rs');
  fs.mkdirSync(path.dirname(source), { recursive: true });
  fs.writeFileSync(source, 'pub fn example() {}');
  const spawnSyncImpl = (command, args, opts) => {
    calls.push([command, ...args]);
    const ok = (stdout = '') => ({ status: 0, stdout });
    if (command === 'git') return ok(args[0] === 'rev-parse' ? currentSha : '');
    if (args.includes('show-env')) return ok(llvmEnvironment);
    if (args[0] === 'clean') return ok();
    if (args[1] === 'archive') {
      assert.equal(opts.env.CARGO_LLVM_COV, '1');
      assert.equal(opts.env.CARGO_PROFILE_TEST_DEBUG, '0');
      fs.writeFileSync(args.at(-1), 'the producer instrumented executable archive');
      return ok();
    }
    if (args[0] === 'nextest') {
      assert.equal(args[args.indexOf('--archive-file') + 1], archivePath);
      assert.equal(args[args.indexOf('--workspace-remap') + 1], path.join(repoRoot, 'api'));
      const partition = args.find((item) => item.startsWith('hash:'));
      if (args[1] === 'list') {
        if (args.includes('binaries-only')) {
          fs.mkdirSync(path.join(args[args.indexOf('--extract-to') + 1], 'target'), { recursive: true });
          return ok('{}');
        }
        return ok(JSON.stringify(inventory(partition ? [partition[5]] : ['1', '2', '3', '4'], ['later'])));
      }
      assert.equal(args[1], 'run');
      fs.writeFileSync(opts.env.LLVM_PROFILE_FILE.replace(/%\d*m/u, 'binary_0'), 'profile');
      fs.writeSync(opts.stdio[2], 'Summary [1.0s] 1 test run: 1 passed, 3 skipped\n');
      return ok();
    }
    assert.deepEqual(args.slice(0, 2), ['llvm-cov', 'report']);
    assert.equal(opts.env.CARGO_TARGET_DIR, opts.env.CARGO_LLVM_COV_TARGET_DIR);
    assert.deepEqual(fs.readdirSync(opts.env.CARGO_LLVM_COV_TARGET_DIR).filter((name) => name.endsWith('.profraw')).sort(),
      [1, 2, 3, 4].map((index) => `shard-${index}-binary_0.profraw`));
    fs.writeFileSync(args.at(-1), JSON.stringify(result));
    return ok();
  };
  const options = { repoRoot, archiveDir, env, spawnSyncImpl };
  buildApiServerCoverageArchive(options);
  fs.rmSync(directory, { recursive: true, force: true });
  fs.mkdirSync(directory);
  const runShard = (shardIndex) => runApiServerShard({ ...options, shardIndex, shardCount: 4,
    cargoTestThreads: 2, writeStdout() {}, writeStderr() {} });
  for (let i = 1; i <= 4; i += 1) {
    runShard(i);
    fs.cpSync(path.join(root, `shard-${i}`), directory, { recursive: true });
  }
  return { calls, archiveDir, archivePath, root, directory, finalPath, runShard,
    changeSha: (value) => { currentSha = value; }, setSummary: (value) => { result = value; },
    merge: () => mergeApiServerShadow({ ...options, shardCount: 4 }) };
}

test('one instrumented build supplies the exact disjoint four-shard inventory and merge binaries', (t) => {
  const f = archiveFixture(t);
  f.merge();
  const evidence = JSON.parse(fs.readFileSync(path.join(f.root, 'equivalence.json')));
  assert.equal(evidence.sha, sha);
  assert.match(evidence.archiveSha256, /^[0-9a-f]{64}$/u);
  assert.equal(evidence.shards.length, 4);
  assert.ok(evidence.shards.every((shard) => shard.passedCount === 1 && shard.profileCount === 1));
  assert.equal(f.calls.filter(([, target, verb]) => target === 'nextest' && verb === 'archive').length, 1);
  const calls = f.calls.slice(f.calls.findIndex(([, target, verb]) => target === 'nextest' && verb === 'archive') + 1);
  assert.ok(calls.filter(([command]) => command === 'cargo').every(([, target, verb, ...args]) =>
    target === 'llvm-cov' ? verb === 'report' : ['list', 'run'].includes(verb) && args.includes('--archive-file')));
  assert.ok(fs.existsSync(f.finalPath));
});

for (const [label, invalidate, expected] of [
  ['stale checkout', (f) => f.changeSha('b'.repeat(40)), /exact frozen SHA checkout/u],
  ['archive digest', (f) => fs.appendFileSync(f.archivePath, 'changed'), /integrity mismatch/u],
  ['missing profile', (f) => fs.unlinkSync(path.join(f.directory, 'shard-2-binary_0.profraw')), /no profraw/u],
  ['overlapping inventory', (f) => fs.copyFileSync(path.join(f.directory, 'inventory-1.json'), path.join(f.directory, 'inventory-2.json')), /duplicat|overlap/u],
  ['stale source', (f) => { const value = summary(); value.data[0].files[0].filename = '/repo/api/apps/api-server/src/removed.rs'; f.setSummary(value); }, /sources absent from the candidate/u],
]) {
  test(`archive mode refuses ${label} without publishing a passing coverage summary`, (t) => {
    const f = archiveFixture(t);
    invalidate(f);
    assert.throws(f.merge, expected);
    assert.equal(fs.existsSync(f.finalPath), false);
  });
}
