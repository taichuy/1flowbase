const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  buildReactDoctorCommand,
  resolveReactDoctorDiffBase,
  runReactDoctorGate,
} = require('../core.js');

test('buildReactDoctorCommand checks only the current commit by default', () => {
  assert.deepEqual(buildReactDoctorCommand({ repoRoot: '/repo', diffBase: 'abc123' }), {
    command: 'npm',
    args: [
      'exec',
      '--yes',
      '--package',
      'react-doctor@0.2.16',
      '--',
      'react-doctor',
      'web/app',
      '--diff',
      'abc123',
      '--no-score',
      '--fail-on',
      'warning',
      '--verbose',
      '--no-color',
    ],
    cwd: '/repo',
  });
});

test('buildReactDoctorCommand accepts an explicit debt audit baseline', () => {
  const command = buildReactDoctorCommand({
    repoRoot: '/repo',
    diffBase: 'origin/main',
  });

  assert.deepEqual(command.args.slice(7, 10), ['--diff', 'origin/main', '--no-score']);
});

test('resolveReactDoctorDiffBase resolves the current commit parent to a SHA', () => {
  const calls = [];
  const diffBase = resolveReactDoctorDiffBase({
    repoRoot: '/repo',
    env: {},
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      return { status: 0, stdout: 'abc123\n', stderr: '' };
    },
  });

  assert.equal(diffBase, 'abc123');
  assert.equal(calls[0].command, 'git');
  assert.deepEqual(calls[0].args, ['rev-parse', '--verify', 'HEAD^']);
  assert.equal(calls[0].options.cwd, '/repo');
});

test('runReactDoctorGate writes stable test-governance evidence', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-react-doctor-'));
  const calls = [];
  let stdout = '';

  const status = runReactDoctorGate({
    repoRoot,
    env: {
      PATH: process.env.PATH,
      REACT_DOCTOR_DIFF_BASE: 'abc123',
    },
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      return {
        status: 0,
        stdout: '\u001b[32mreact doctor passed\u001b[0m\n',
        stderr: '',
      };
    },
    writeStdout(text) {
      stdout += text;
    },
    writeStderr() {},
  });

  assert.equal(status, 0);
  assert.equal(stdout, '\u001b[32mreact doctor passed\u001b[0m\n');
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, 'npm');
  assert.deepEqual(calls[0].args, [
    'exec',
    '--yes',
    '--package',
    'react-doctor@0.2.16',
    '--',
    'react-doctor',
    'web/app',
    '--diff',
    'abc123',
    '--no-score',
    '--fail-on',
    'warning',
    '--verbose',
    '--no-color',
  ]);
  assert.equal(calls[0].options.cwd, repoRoot);

  const outputDir = path.join(repoRoot, 'tmp', 'test-governance');
  assert.equal(fs.readFileSync(path.join(outputDir, 'react-doctor.log'), 'utf8'), 'react doctor passed\n');
  assert.match(
    fs.readFileSync(path.join(outputDir, 'react-doctor.md'), 'utf8'),
    /Status: passed/u,
  );

  const report = JSON.parse(fs.readFileSync(path.join(outputDir, 'react-doctor.json'), 'utf8'));
  assert.equal(report.status, 'passed');
  assert.equal(report.exitCode, 0);
  assert.equal(report.command, 'npm exec --yes --package react-doctor@0.2.16 -- react-doctor web/app --diff abc123 --no-score --fail-on warning --verbose --no-color');
  assert.equal(report.logPath, 'tmp/test-governance/react-doctor.log');
});
