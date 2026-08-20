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

test('buildReactDoctorCommand records an explicit base while scanning JSON evidence', () => {
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
      '--json',
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

  assert.deepEqual(command.args.slice(7, 11), ['--diff', 'origin/main', '--json', '--no-score']);
});

test('resolveReactDoctorDiffBase requires an explicit base or pull request base SHA', () => {
  assert.throws(
    () => resolveReactDoctorDiffBase({ env: {} }),
    /set REACT_DOCTOR_DIFF_BASE or GITHUB_BASE_SHA/u,
  );
  assert.equal(
    resolveReactDoctorDiffBase({ env: { GITHUB_BASE_SHA: 'base-sha' } }),
    'base-sha',
  );
});

test('runReactDoctorGate binds candidate, changed files, and diagnostics to the artifact', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-react-doctor-'));
  fs.mkdirSync(path.join(repoRoot, 'web', 'app'), { recursive: true });
  fs.writeFileSync(
    path.join(repoRoot, 'web', 'app', 'doctor.config.json'),
    JSON.stringify({ ignore: { overrides: [] } }),
  );
  assert.equal(resolveReactDoctorDiffBase({
    env: { REACT_DOCTOR_DIFF_BASE: 'base-sha' },
  }), 'base-sha');
  const calls = [];
  let stdout = '';

  const status = runReactDoctorGate({
    repoRoot,
    env: {
      PATH: process.env.PATH,
      REACT_DOCTOR_DIFF_BASE: 'base-sha',
      REACT_DOCTOR_CANDIDATE_SHA: 'candidate-sha',
    },
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      if (command === 'git' && args[0] === 'rev-parse') {
        return {
          status: 0,
          stdout: args.at(-1) === 'base-sha^{commit}' ? 'base-sha\n' : 'candidate-sha\n',
          stderr: '',
        };
      }
      if (command === 'git' && args[0] === 'diff') {
        return {
          status: 0,
          stdout: 'web/app/src/features/a.tsx\0web/app/src/features/b.tsx\0',
          stderr: '',
        };
      }
      return {
        status: 0,
        stdout: JSON.stringify({ summary: { totalDiagnosticCount: 2 } }),
        stderr: '',
      };
    },
    writeStdout(text) {
      stdout += text;
    },
    writeStderr() {},
  });

  assert.equal(status, 0);
  assert.equal(stdout, JSON.stringify({ summary: { totalDiagnosticCount: 2 } }));
  assert.equal(calls.length, 5);
  assert.equal(calls.at(-1).command, 'npm');
  assert.deepEqual(calls.at(-1).args, [
    'exec',
    '--yes',
    '--package',
    'react-doctor@0.2.16',
    '--',
    'react-doctor',
    'web/app',
    '--diff',
    'base-sha',
    '--json',
    '--no-score',
    '--fail-on',
    'warning',
    '--verbose',
    '--no-color',
  ]);
  assert.equal(calls.at(-1).options.cwd, repoRoot);

  const outputDir = path.join(repoRoot, 'tmp', 'test-governance');
  assert.equal(
    fs.readFileSync(path.join(outputDir, 'react-doctor.log'), 'utf8'),
    JSON.stringify({ summary: { totalDiagnosticCount: 2 } }),
  );
  assert.match(
    fs.readFileSync(path.join(outputDir, 'react-doctor.md'), 'utf8'),
    /Status: passed/u,
  );

  const report = JSON.parse(fs.readFileSync(path.join(outputDir, 'react-doctor.json'), 'utf8'));
  assert.equal(report.status, 'passed');
  assert.equal(report.exitCode, 0);
  assert.equal(report.baseSha, 'base-sha');
  assert.equal(report.candidateSha, 'candidate-sha');
  assert.deepEqual(report.changedFiles, [
    'web/app/src/features/a.tsx',
    'web/app/src/features/b.tsx',
  ]);
  assert.equal(report.changedFileCount, 2);
  assert.equal(report.unsuppressed, 2);
  assert.equal(report.suppressed, 0);
  assert.equal(report.command, 'npm exec --yes --package react-doctor@0.2.16 -- react-doctor web/app --diff base-sha --json --no-score --fail-on warning --verbose --no-color');
  assert.equal(report.logPath, 'tmp/test-governance/react-doctor.log');
});
