const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { main } = require('../../test-backend.js');
const { buildBackendCommands } = require('../../verify/backend.js');

const runtimeConfig = {
  backend: { cargoJobs: 5, cargoTestThreads: 2, incremental: true },
};
const passingResult = {
  status: 0,
  stdout: 'test result: ok. 1 passed; 0 failed; 0 ignored;\n',
  stderr: '',
};

async function runFixture(t, argv, cargoResult = passingResult) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'test-backend-cli-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const calls = [];
  let managedOptions;
  let stderr = '';
  const status = await main(argv, {
    repoRoot,
    env: {},
    runtimeConfig,
    writeStdout() {},
    writeStderr(text) { stderr += text; },
    managedRunnerImpl(options) {
      managedOptions = options;
      return options.runCommandSequenceImpl({
        ...options,
        spawnSyncImpl(command, args, spawnOptions) {
          calls.push({ command, args, ...spawnOptions });
          return command === 'cargo'
            ? cargoResult
            : { status: 0, stdout: '', stderr: '' };
        },
      });
    },
  });
  return { status, calls, managedOptions, stderr, repoRoot };
}

function cargoCall(result) {
  const calls = result.calls.filter((call) => call.command === 'cargo');
  assert.equal(calls.length, 1, 'each invocation runs one Cargo test selection');
  return calls[0];
}

test('no-argument backend entry retains static and workspace coverage without duplicate image runs', async (t) => {
  const result = await runFixture(t, []);
  assert.equal(result.status, 0);
  assert.equal(result.calls.length, 2);
  assert.equal(result.calls[0].args.at(-1), 'check-rust-backend');
  assert.deepEqual(cargoCall(result).args,
    ['test', '--workspace', '--jobs', '5', '--', '--test-threads=2']);
  assert.equal(result.managedOptions.scope, 'test-backend');
  assert.equal(result.managedOptions.lockMode, 'heavy');
  assert.equal(result.managedOptions.commandDisplay, 'node scripts/node/test-backend.js');

  const focused = buildBackendCommands({
    repoRoot: result.repoRoot, env: {}, cargoJobs: 5, cargoTestThreads: 2,
    target: 'image-llm-vision',
  });
  assert.equal(focused.length, 4, 'the independent image gate remains available');
  assert.ok(focused.every((command) => command.label.startsWith('cargo-test-image-llm-vision-')));
  assert.ok(focused.every((command) => !command.args.includes('--workspace')));
});

test('backend package, target and filter selections reach Cargo with configured resource limits', async (t) => {
  const cases = [
    {
      argv: ['--package', 'domain', '--lib', '--filter', 'scope::matches', '--exact'],
      packages: ['domain'], target: ['--lib'], filter: 'scope::matches', exact: true,
    },
    {
      argv: ['-p', 'control-plane-postgres-tests', '--test', 'responses_history_migration', '--filter', 'migration'],
      packages: ['control-plane-postgres-tests'], target: ['--test', 'responses_history_migration'], filter: 'migration',
    },
    {
      argv: ['-p', 'api-server', '--bin', 'api-server'],
      packages: ['api-server'], target: ['--bin', 'api-server'],
    },
    {
      argv: ['-p', 'domain', '--package', 'access-control'],
      packages: ['domain', 'access-control'], target: [],
    },
  ];
  for (const fixture of cases) {
    const result = await runFixture(t, fixture.argv);
    assert.equal(result.status, 0);
    const call = cargoCall(result);
    assert.equal(call.args.includes('--workspace'), false);
    assert.deepEqual(call.args.flatMap((arg, index) => arg === '--package' ? [call.args[index + 1]] : []), fixture.packages);
    const targetIndex = 1 + fixture.packages.length * 2;
    assert.deepEqual(call.args.slice(targetIndex, targetIndex + fixture.target.length), fixture.target);
    assert.equal(call.args[targetIndex + fixture.target.length], '--jobs');
    if (fixture.filter) {
      assert.ok(call.args.includes(fixture.filter));
      assert.ok(call.args.indexOf(fixture.filter) < call.args.indexOf('--'));
    }
    assert.equal(call.args[call.args.indexOf('--jobs') + 1], '5');
    assert.deepEqual(call.args.slice(call.args.indexOf('--') + 1), [
      '--test-threads=2', ...(fixture.exact ? ['--exact'] : []),
    ]);
    assert.equal(call.env.CARGO_BUILD_JOBS, '5');
    assert.equal(call.env.CARGO_INCREMENTAL, '1');
    assert.equal(call.cwd, path.join(result.repoRoot, 'api'));
    assert.equal(result.managedOptions.lockMode, 'heavy');
    assert.match(result.managedOptions.commandDisplay, /domain|api-server|control-plane-postgres-tests/u);
  }
});

test('backend rejects unsupported or incomplete selections before starting any runner', async () => {
  const invalid = [
    [['domain'], /Unsupported backend test argument/u],
    [['--workspace'], /Unsupported backend test argument/u],
    [['--package'], /Missing value/u],
    [['--package', '--lib'], /Missing value/u],
    [['--package', ''], /Missing value/u],
    [['--filter', 'one_test'], /requires --package/u],
    [['--lib'], /requires --package/u],
    [['-p', 'domain', '--filter'], /Missing value/u],
    [['-p', 'domain', '--test'], /Missing value/u],
    [['-p', 'domain', '--lib', '--test', 'integration'], /Only one backend test target/u],
    [['-p', 'domain', '--filter', 'one', '--filter', 'two'], /Only one backend test filter/u],
    [['-p', 'domain', '--exact'], /--exact requires --filter/u],
    [['-p', 'domain', '--jobs', '99'], /Unsupported backend test argument/u],
    [['-p', 'domain', '--', '--test-threads=99'], /Unsupported backend test argument/u],
  ];
  for (const [argv, error] of invalid) {
    let started = false;
    await assert.rejects(() => main(argv, {
      managedRunnerImpl() { started = true; return 0; },
      spawnSyncImpl() { started = true; return passingResult; },
    }), error);
    assert.equal(started, false, argv.join(' '));
  }
});

test('backend selection requires executed passing tests and preserves process failure', async (t) => {
  const cases = [
    { stdout: 'test result: ok. 0 passed; 0 failed; 0 ignored;', expected: 1 },
    { stdout: 'test result: ok. 0 passed; 0 failed; 7 ignored;', expected: 1 },
    { stdout: 'Compiling domain\n', expected: 1 },
    { stdout: 'test result: FAILED. 1 passed; 1 failed; 0 ignored;', expected: 1 },
    { stdout: 'test result: ok. 0 passed; 0 failed;\ntest result: ok. 1 passed; 0 failed;', expected: 0 },
    { stdout: passingResult.stdout, status: 23, expected: 23 },
  ];
  for (const fixture of cases) {
    const result = await runFixture(t, ['-p', 'domain', '--lib', '--filter', 'selected'], {
      status: fixture.status ?? 0, stdout: fixture.stdout, stderr: '',
    });
    assert.equal(result.status, fixture.expected, fixture.stdout);
    if (fixture.expected === 1) assert.match(result.stderr, /no executed passing tests/u);
  }
  const emptyWorkspace = await runFixture(t, [], { ...passingResult, stdout: '' });
  assert.equal(emptyWorkspace.status, 1);
});

test('selected backend execution retains advisory warning capture', async (t) => {
  const result = await runFixture(t, ['-p', 'domain', '--lib'], {
    ...passingResult, stderr: 'warning: cargo test advisory\n',
  });
  assert.equal(result.status, 0);
  const warningLog = path.join(result.repoRoot, 'tmp', 'test-governance', 'test-backend.warnings.log');
  assert.match(fs.readFileSync(warningLog, 'utf8'), /warning: cargo test advisory/u);
});

test('backend help documents selection without running the gate', async () => {
  let output = '';
  let ran = false;
  const status = await main(['--help'], {
    writeStdout(text) { output += text; },
    managedRunnerImpl() { ran = true; return 0; },
  });
  assert.equal(status, 0);
  assert.equal(ran, false);
  assert.match(output, /Usage: node scripts\/node\/test-backend\.js/u);
  assert.match(output, /--package/u);
  assert.match(output, /--filter/u);
});
