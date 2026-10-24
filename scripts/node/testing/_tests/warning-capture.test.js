const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const {
  runCommandSequence,
  runManagedCommandSequence,
} = require('../warning-capture.js');

test('runManagedCommandSequence injects the heavy lock token into child env', async () => {
  const calls = [];

  const status = await runManagedCommandSequence({
    repoRoot: '/repo-root',
    env: {},
    scope: 'verify-backend',
    lockMode: 'heavy',
    commandDisplay: 'node scripts/node/verify-backend.js',
    commands: [{ label: 'cargo-test', command: 'cargo', args: ['test'] }],
    withHeavyVerifyLockImpl: async (_options, run) => run({
      ONEFLOWBASE_VERIFY_LOCK_TOKEN: 'chain-token',
    }),
    runCommandSequenceImpl: (options) => {
      calls.push(options);
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].env.ONEFLOWBASE_VERIFY_LOCK_TOKEN, 'chain-token');
});

test('runManagedCommandSequence skips the heavy lock wrapper in none mode', async () => {
  let heavyLockCalls = 0;

  const status = await runManagedCommandSequence({
    repoRoot: '/repo-root',
    env: {},
    scope: 'test-backend',
    lockMode: 'none',
    commandDisplay: 'node scripts/node/test-backend.js',
    commands: [{ label: 'cargo-test', command: 'cargo', args: ['test'] }],
    withHeavyVerifyLockImpl: async () => {
      heavyLockCalls += 1;
      return 0;
    },
    runCommandSequenceImpl: () => 0,
  });

  assert.equal(status, 0);
  assert.equal(heavyLockCalls, 0);
});

test('runCommandSequence handles large stdout without hitting the default spawnSync buffer limit', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-warning-capture-'));
  const scriptPath = path.join(repoRoot, 'emit-large-output.js');

  fs.writeFileSync(
    scriptPath,
    'process.stdout.write("x".repeat(2 * 1024 * 1024));\n'
  );

  const status = runCommandSequence({
    repoRoot,
    scope: 'warning-capture-large-output',
    commands: [
      {
        label: 'large-output',
        command: process.execPath,
        args: [scriptPath],
      },
    ],
    spawnSyncImpl: spawnSync,
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status, 0);
});

test('runCommandSequence does not create a warning log for stdout-only success output', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-warning-capture-'));
  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'stdout-only.warnings.log');

  const status = runCommandSequence({
    repoRoot,
    scope: 'stdout-only',
    commands: [
      {
        label: 'stdout-only',
        command: process.execPath,
        args: ['-e', 'process.stdout.write("all good\\n")'],
      },
    ],
    spawnSyncImpl: spawnSync,
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status, 0);
  assert.equal(fs.existsSync(warningLogPath), false);
});

test('runCommandSequence injects the pnpm-adjacent real node env for pnpm commands', () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-pnpm-node-env-'));
  const binDir = path.join(tempDir, 'bin');
  const calls = [];

  fs.mkdirSync(binDir, { recursive: true });
  fs.writeFileSync(path.join(binDir, 'pnpm'), '', 'utf8');
  fs.writeFileSync(path.join(binDir, 'node'), '', 'utf8');
  fs.chmodSync(path.join(binDir, 'pnpm'), 0o755);
  fs.chmodSync(path.join(binDir, 'node'), 0o755);

  const status = runCommandSequence({
    repoRoot: '/repo-root',
    scope: 'pnpm-node-env',
    env: {
      PATH: ['/tmp/bun-node-wrapper', binDir].join(path.delimiter),
    },
    commands: [
      {
        label: 'pnpm-test',
        command: 'pnpm',
        args: ['--version'],
      },
    ],
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      return { status: 0, stdout: '', stderr: '' };
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, 'pnpm');
  assert.equal(calls[0].options.env.NODE, path.join(binDir, 'node'));
  assert.equal(
    calls[0].options.env.npm_node_execpath,
    path.join(binDir, 'node')
  );
  assert.match(
    calls[0].options.env.PATH,
    new RegExp(`^${binDir.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`)
  );
});

test('runCommandSequence resets the warning log before a new run', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-warning-capture-'));
  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'reset.warnings.log');

  const firstStatus = runCommandSequence({
    repoRoot,
    scope: 'reset',
    commands: [
      {
        label: 'first-run',
        command: process.execPath,
        args: ['-e', 'require("node:fs").writeSync(2, "warning: first run\\n")'],
      },
    ],
    spawnSyncImpl: spawnSync,
    writeStdout() {},
    writeStderr() {},
  });

  const firstLog = fs.readFileSync(warningLogPath, 'utf8');
  assert.equal(firstStatus, 0);
  assert.match(firstLog, /warning: first run/u);

  const secondStatus = runCommandSequence({
    repoRoot,
    scope: 'reset',
    commands: [
      {
        label: 'second-run',
        command: process.execPath,
        args: ['-e', 'require("node:fs").writeSync(2, "warning: second run\\n")'],
      },
    ],
    spawnSyncImpl: spawnSync,
    writeStdout() {},
    writeStderr() {},
  });

  const secondLog = fs.readFileSync(warningLogPath, 'utf8');
  assert.equal(secondStatus, 0);
  assert.doesNotMatch(secondLog, /warning: first run/u);
  assert.match(secondLog, /warning: second run/u);
});

test('runCommandSequence removes stale warning logs before a clean run', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-warning-capture-'));
  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'warning-clean.warnings.log');

  const firstStatus = runCommandSequence({
    repoRoot,
    scope: 'warning-clean',
    commands: [
      {
        label: 'warning-run',
        command: process.execPath,
        args: ['-e', 'require("node:fs").writeSync(2, "warning: stale artifact\\n")'],
      },
    ],
    spawnSyncImpl: spawnSync,
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(firstStatus, 0);
  assert.equal(fs.existsSync(warningLogPath), true);

  const secondStatus = runCommandSequence({
    repoRoot,
    scope: 'warning-clean',
    commands: [
      {
        label: 'clean-run',
        command: process.execPath,
        args: ['-e', 'process.stdout.write("clean run\\n")'],
      },
    ],
    spawnSyncImpl: spawnSync,
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(secondStatus, 0);
  assert.equal(fs.existsSync(warningLogPath), false);
});

test('completion status stops later child processes and preserves process and act failures', () => {
  for (const [childStatus, completionStatus, actWarning, expected] of [
    [0, undefined, false, 0],
    [0, 0, false, 0],
    [0, 7, false, 7],
    [23, 7, false, 23],
    [0, 7, true, 1],
  ]) {
    const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'command-completion-'));
    try {
      const marker = path.join(repoRoot, 'later-child-started');
      const output = actWarning
        ? 'An update to Table inside a test was not wrapped in act(...).'
        : 'test result: ok. 0 passed; 0 failed;';
      let stdout = '';
      let stderr = '';
      let completions = 0;
      const status = runCommandSequence({
        repoRoot, env: {}, scope: 'completion-status',
        commands: [
          { label: 'first', command: process.execPath, args: ['-e',
            `process.stdout.write(${JSON.stringify(output)}); process.exitCode = ${childStatus};`] },
          { label: 'later', command: process.execPath, args: ['-e',
            `require('node:fs').writeFileSync(${JSON.stringify(marker)}, 'started');`] },
        ],
        writeStdout(text) { stdout += text; },
        writeStderr(text) { stderr += text; },
        onCommandComplete({ command, result }) {
          completions += 1;
          if (command.label === 'first') {
            assert.equal(result.status, childStatus);
            return completionStatus;
          }
        },
      });
      assert.equal(status, expected);
      assert.equal(stdout, output);
      assert.equal(completions, expected === 0 ? 2 : 1);
      assert.equal(fs.existsSync(marker), expected === 0);
      if (actWarning) assert.match(stderr, /test-fidelity.*first/u);
    } finally {
      fs.rmSync(repoRoot, { recursive: true, force: true });
    }
  }
});

test('spawn errors retain available output and failure diagnostics before being rethrown', () => {
  for (const code of ['ENOBUFS', 'ENOENT']) {
    const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'spawn-error-evidence-'));
    try {
      const logPath = path.join(repoRoot, 'tmp/test-governance/spawn-error.warnings.log');
      const error = Object.assign(new Error('spawnSync cargo buffer exhausted'), { code });
      let stdout = '';
      let stderr = '';
      let calls = 0;
      let completionCalls = 0;
      assert.throws(() => runCommandSequence({
        repoRoot, env: {}, scope: 'spawn-error',
        commands: [
          { label: 'failed-spawn', command: path.join(repoRoot, 'missing-executable'), args: [] },
          { label: 'must-not-start', command: process.execPath, args: [] },
        ],
        spawnSyncImpl(...args) {
          calls += 1;
          return code === 'ENOENT' ? spawnSync(...args) : {
            error, status: null, stdout: 'partial test output\n',
            stderr: '   Compiling interrupted-crate\nwarning: partial diagnostic\n',
          };
        },
        writeStdout(text) { stdout += text; },
        writeStderr(text) { stderr += text; },
        onCommandComplete() { completionCalls += 1; },
      }), (caught) => {
        assert.equal(caught.code, code);
        if (code === 'ENOBUFS') assert.equal(caught, error);
        assert.ok(fs.existsSync(logPath), 'failure evidence must exist before the throw');
        assert.ok(stderr.includes(code), 'the diagnostic must be emitted before the throw');
        return true;
      });
      const log = fs.readFileSync(logPath, 'utf8');
      assert.equal(calls, 1);
      assert.equal(completionCalls, 0);
      assert.match(log, /step=failed-spawn/u);
      assert.ok(log.includes(code));
      if (code === 'ENOBUFS') {
        assert.equal(stdout, 'partial test output\n');
        assert.match(stderr, /Compiling interrupted-crate\nwarning: partial diagnostic/u);
        assert.match(log, /\[stdout\]\npartial test output/u);
        assert.match(log, /Compiling interrupted-crate\nwarning: partial diagnostic/u);
      }
    } finally {
      fs.rmSync(repoRoot, { recursive: true, force: true });
    }
  }
});
