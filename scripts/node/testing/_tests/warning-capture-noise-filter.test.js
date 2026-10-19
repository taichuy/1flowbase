const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { runCommandSequence } = require('../warning-capture.js');

test('runCommandSequence ignores known tool progress stderr on successful commands', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-warning-capture-'));
  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'tool-progress.warnings.log');

  const status = runCommandSequence({
    repoRoot,
    scope: 'tool-progress',
    commands: [
      { label: 'cargo-clippy', command: 'cargo', args: ['clippy'] },
      { label: 'frontend-lint', command: 'pnpm', args: ['lint'] },
    ],
    spawnSyncImpl(command) {
      if (command === 'cargo') {
        return {
          status: 0,
          stdout: '',
          stderr: [
            '    Updating crates.io index',
            ' Downloading crates ...',
            '  Downloaded serde v1.0.228',
            '   Compiling api-server v0.1.0 (/repo/api/apps/api-server)',
            '    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.23s',
          ].join('\n'),
        };
      }

      return {
        status: 0,
        stdout: '',
        stderr: [
          'Attention:',
          '• turbo 2.9.6',
          'Turborepo now collects completely anonymous telemetry regarding usage.',
          'This information is used to shape the Turborepo roadmap and prioritize features.',
          "You can learn more, including how to opt-out if you'd not like to participate in this anonymous program, by visiting the following URL:",
          'https://turborepo.dev/docs/telemetry',
          '$ turbo run lint',
          '$ ../scripts/node/cli/exec-with-real-node.sh ../scripts/node/cli/run-frontend-workspace-test.js',
          '$ tsc -p tsconfig.json --noEmit && vite build',
          '$ ../../scripts/node/cli/exec-with-real-node.sh ../../scripts/node/cli/run-frontend-vitest.js run src/features/me/_tests/me-page.test.tsx',
          '$ eslint src --ext .ts',
        ].join('\n'),
      };
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status, 0);
  assert.equal(fs.existsSync(warningLogPath), false);
});

test('runCommandSequence fails successful child commands with React act warnings and preserves the blocks', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-warning-capture-'));
  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'react-act.warnings.log');

  const status = runCommandSequence({
    repoRoot,
    scope: 'react-act',
    commands: [
      { label: 'frontend-test', command: 'pnpm', args: ['test'] },
    ],
    spawnSyncImpl() {
      return {
        status: 0,
        stdout: '',
        stderr: [
          'stderr | src/style-boundary/_tests/registry.test.tsx > renders page',
          'An update to Transitioner inside a test was not wrapped in act(...).',
          '',
          'When testing, code that causes React state updates should be wrapped into act(...):',
          '',
          'act(() => {',
          '  /* fire events that update state */',
          '});',
          '/* assert on the output */',
          '',
          "This ensures that you're testing the behavior the user would see in the browser. Learn more at https://react.dev/link/wrap-tests-with-act",
          '',
          'stderr | src/style-boundary/_tests/registry.test.tsx > renders another page',
          'An update to Navigation inside a test was not wrapped in act(...).',
          "This ensures that you're testing the behavior the user would see in the browser. Learn more at https://react.dev/link/wrap-tests-with-act",
        ].join('\n'),
      };
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status, 1);
  const warningLog = fs.readFileSync(warningLogPath, 'utf8');
  assert.match(warningLog, /Transitioner inside a test was not wrapped in act/u);
  assert.match(warningLog, /Navigation inside a test was not wrapped in act/u);
  assert.match(warningLog, /src\/style-boundary\/_tests\/registry.test.tsx/u);
});

test('runCommandSequence preserves other warnings alongside an act failure', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-warning-capture-'));
  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'react-act-real-warning.warnings.log');

  const status = runCommandSequence({
    repoRoot,
    scope: 'react-act-real-warning',
    commands: [
      { label: 'frontend-test', command: 'pnpm', args: ['test'] },
    ],
    spawnSyncImpl() {
      return {
        status: 0,
        stdout: '',
        stderr: [
          'stderr | src/example.test.tsx > renders page',
          'An update to Transitioner inside a test was not wrapped in act(...).',
          "This ensures that you're testing the behavior the user would see in the browser. Learn more at https://react.dev/link/wrap-tests-with-act",
          'real warning',
        ].join('\n'),
      };
    },
    writeStdout() {},
    writeStderr() {},
  });

  const warningLog = fs.readFileSync(warningLogPath, 'utf8');
  assert.equal(status, 1);
  assert.match(warningLog, /stderr \| src\/example\.test\.tsx/u);
  assert.match(warningLog, /real warning/u);
  assert.match(warningLog, /wrapped in act/u);
});

test('runCommandSequence keeps full stderr for failed commands', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-warning-capture-'));
  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'failed-progress.warnings.log');

  const status = runCommandSequence({
    repoRoot,
    scope: 'failed-progress',
    commands: [
      { label: 'cargo-test', command: 'cargo', args: ['test'] },
    ],
    spawnSyncImpl() {
      return {
        status: 1,
        stdout: '',
        stderr: [
          '   Compiling api-server v0.1.0 (/repo/api/apps/api-server)',
          'error: test failed, to rerun pass `-p api-server --lib`',
        ].join('\n'),
      };
    },
    writeStdout() {},
    writeStderr() {},
  });

  const warningLog = fs.readFileSync(warningLogPath, 'utf8');
  assert.equal(status, 1);
  assert.match(warningLog, /Compiling api-server/u);
  assert.match(warningLog, /error: test failed/u);
});


test('runCommandSequence detects ANSI and workspace-prefixed act output on stdout', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-warning-capture-'));
  let diagnostic = '';
  const status = runCommandSequence({
    repoRoot,
    scope: 'frontend',
    commands: [{ label: 'workspace-test', command: 'pnpm', args: ['test'] }],
    spawnSyncImpl: () => ({
      status: 0,
      stdout: '\u001b[31m@web:test: An update to Table inside a test was not wrapped in act(...).\u001b[0m',
      stderr: '',
    }),
    writeStdout() {},
    writeStderr(text) { diagnostic += text; },
  });
  assert.equal(status, 1);
  assert.match(diagnostic, /test-fidelity.*workspace-test/u);
});

test('runCommandSequence records upstream warnings without treating them as act failures', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-warning-capture-'));
  const status = runCommandSequence({
    repoRoot,
    scope: 'upstream',
    commands: [{ label: 'build', command: 'pnpm', args: ['build'] }],
    spawnSyncImpl: () => ({ status: 0, stdout: '', stderr: 'upstream dependency is deprecated' }),
    writeStdout() {},
    writeStderr() {},
  });
  assert.equal(status, 0);
  assert.match(fs.readFileSync(path.join(repoRoot, 'tmp/test-governance/upstream.warnings.log'), 'utf8'), /upstream dependency/u);
});
